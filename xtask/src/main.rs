//! Development tasks.
//!
//! ```sh
//! cargo xtask stdlib --game skyrim --from ".../Data/Source/Scripts"
//! ```
//!
//! Only runs on the machine of a developer who owns the game: the `papyrus`
//! binary never reads an installation. The result, signature tables, is
//! committed.

#![forbid(unsafe_code)]

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use papyrus_dialect::{Dialect, GameId};
use papyrus_project::{DiscoverOptions, decode, discover};
use papyrus_syntax::{SyntaxKind, SyntaxNode};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Debug, Subcommand)]
enum Task {
    /// Generate the `papyrus-stdlib` tables from a game's sources.
    Stdlib {
        /// skyrim, fallout4 or starfield.
        #[arg(long)]
        game: GameId,
        /// The folder of the game's `.psc` sources.
        #[arg(long)]
        from: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::parse().task {
        Task::Stdlib { game, from } => match stdlib(game, &from) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
    }
}

// ── Extraction ────────────────────────────────────────────────────────────

struct Param {
    name: String,
    ty: String,
    default: Option<String>,
}

struct Function {
    name: String,
    returns: Option<String>,
    params: Vec<Param>,
    global: bool,
    native: bool,
}

struct Property {
    name: String,
    ty: String,
    readonly: bool,
}

struct Script {
    name: String,
    parent: Option<String>,
    functions: Vec<Function>,
    events: Vec<Function>,
    properties: Vec<Property>,
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// The text of a node, trivia included, trimmed.
fn text(node: &SyntaxNode) -> String {
    node.text().to_string().trim().to_owned()
}

/// The significant text of a node: non-trivia tokens, joined.
fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_owned())
        .collect()
}

fn has_flag(owner: &SyntaxNode, flag: SyntaxKind) -> bool {
    child(owner, SyntaxKind::FLAG_LIST)
        .is_some_and(|list| list.descendants_with_tokens().any(|e| e.kind() == flag))
}

/// True for a property that cannot be assigned: `AutoReadOnly`, `Const`,
/// or a full property without `Set`.
fn readonly(property: &SyntaxNode) -> bool {
    if has_flag(property, SyntaxKind::AUTOREADONLY_KW) || has_flag(property, SyntaxKind::CONST_KW) {
        return true;
    }
    !has_flag(property, SyntaxKind::AUTO_KW)
        && !property
            .children()
            .filter(|c| c.kind() == SyntaxKind::FUNCTION_DECL)
            .filter_map(|f| child(&f, SyntaxKind::NAME))
            .any(|n| text(&n).eq_ignore_ascii_case("Set"))
}

fn function(decl: &SyntaxNode) -> Option<Function> {
    let name = text(&child(decl, SyntaxKind::NAME)?);
    // The return type precedes `Function`; `Event` has none.
    let returns = if decl.kind() == SyntaxKind::FUNCTION_DECL {
        child(decl, SyntaxKind::TYPE_REF).map(|t| compact(&t))
    } else {
        None
    };
    let params = child(decl, SyntaxKind::PARAM_LIST)
        .into_iter()
        .flat_map(|list| list.children().filter(|c| c.kind() == SyntaxKind::PARAM))
        .filter_map(|param| {
            let ty = compact(&child(&param, SyntaxKind::TYPE_REF)?);
            let name = text(&child(&param, SyntaxKind::NAME)?);
            // The default value: whatever follows `=`.
            let default = param
                .children()
                .find(|c| !matches!(c.kind(), SyntaxKind::TYPE_REF | SyntaxKind::NAME))
                .map(|e| compact(&e));
            Some(Param { name, ty, default })
        })
        .collect();
    Some(Function {
        name,
        returns,
        params,
        global: has_flag(decl, SyntaxKind::GLOBAL_KW),
        native: has_flag(decl, SyntaxKind::NATIVE_KW),
    })
}

/// True if the script only declares: no variable, no property, no function
/// body.
///
/// That is the shape of engine types without native functions — in Skyrim,
/// `Activator.psc` is a single line: `Scriptname Activator extends Form
/// Hidden`, and `Static.psc` does not even have `Hidden`. Without this
/// criterion, these types would be missing from the tables.
fn declares_only(root: &SyntaxNode) -> bool {
    root.children().all(|item| match item.kind() {
        SyntaxKind::SCRIPT_HEADER | SyntaxKind::IMPORT => true,
        SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
            child(&item, SyntaxKind::STMT_LIST).is_none_or(|body| body.children().next().is_none())
        }
        _ => false,
    })
}

fn script(root: &SyntaxNode) -> Option<Script> {
    let header = child(root, SyntaxKind::SCRIPT_HEADER)?;
    let name = text(&child(&header, SyntaxKind::NAME)?);
    let parent = child(&header, SyntaxKind::EXTENDS_CLAUSE)
        .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
        .map(|t| compact(&t));

    // Only the declarations of the empty state: states redefine, they do not
    // declare.
    let mut functions: Vec<Function> = root
        .children()
        .filter(|c| c.kind() == SyntaxKind::FUNCTION_DECL)
        .filter_map(|f| function(&f))
        .collect();
    let mut events: Vec<Function> = root
        .children()
        .filter(|c| c.kind() == SyntaxKind::EVENT_DECL)
        // `Event Actor.OnDeath` listens to another script: it is not one of
        // this script's events.
        .filter(|e| child(e, SyntaxKind::TYPE_REF).is_none())
        .filter_map(|e| function(&e))
        .collect();

    // Properties, in a group or not: `Actor.CritStage_None`,
    // `GlobalVariable.Value`.
    let groups = root
        .children()
        .filter(|c| c.kind() == SyntaxKind::GROUP_DECL);
    let mut properties: Vec<Property> = root
        .children()
        .chain(groups.flat_map(|g| g.children()))
        .filter(|c| c.kind() == SyntaxKind::PROPERTY_DECL)
        .filter_map(|p| {
            Some(Property {
                name: text(&child(&p, SyntaxKind::NAME)?),
                ty: compact(&child(&p, SyntaxKind::TYPE_REF)?),
                readonly: readonly(&p),
            })
        })
        .collect();

    let native = has_flag(&header, SyntaxKind::NATIVE_KW) || functions.iter().any(|f| f.native);
    if !native && !declares_only(root) {
        return None;
    }

    let by_name = |a: &Function, b: &Function| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    };
    functions.sort_by(by_name);
    functions.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    events.sort_by(by_name);
    events.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    properties.sort_by_key(|p| p.name.to_ascii_lowercase());
    properties.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    Some(Script {
        name,
        parent,
        functions,
        events,
        properties,
    })
}

// ── Script extender ───────────────────────────────────────────────────────

/// The scripts SKSE adds to Skyrim's: not in the vanilla sources.
const SKSE_ONLY: &[&str] = &[
    "ActorValueInfo",
    "ArmorAddon",
    "Art",
    "Camera",
    "ColorComponent",
    "ColorForm",
    "CombatStyle",
    "DefaultObjectManager",
    "EquipSlot",
    "FormType",
    "GameData",
    "HeadPart",
    "Input",
    "ModEvent",
    "NetImmerse",
    "SKSE",
    "SoundDescriptor",
    "SpawnerTask",
    "StringUtil",
    "TreeObject",
    "UI",
    "UICallback",
    "WornObject",
];

/// The vanilla part of a Skyrim source SKSE may have extended: what precedes
/// its `; SKSE64 additions built ...` line. `None` for a script of SKSE's
/// own.
fn vanilla_part(text: &str) -> Option<&str> {
    let header = text.lines().find(|l| {
        l.trim_start()
            .to_ascii_lowercase()
            .starts_with("scriptname")
    })?;
    let name = header.split_whitespace().nth(1)?;
    if SKSE_ONLY.iter().any(|s| s.eq_ignore_ascii_case(name)) {
        return None;
    }
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let lower = line.trim_start().to_ascii_lowercase();
        if lower.starts_with("; skse") && lower.contains("additions built") {
            return Some(&text[..start]);
        }
        start += line.len();
    }
    Some(text)
}

// ── Generation ────────────────────────────────────────────────────────────

fn emit_functions(out: &mut String, functions: &[Function]) {
    out.push_str("&[");
    for f in functions {
        write!(
            out,
            "\n        Function {{ name: {:?}, returns: {:?}, global: {}, native: {}, params: &[",
            f.name, f.returns, f.global, f.native
        )
        .unwrap();
        for p in &f.params {
            write!(
                out,
                "Param {{ name: {:?}, ty: {:?}, default: {:?} }}, ",
                p.name, p.ty, p.default
            )
            .unwrap();
        }
        out.push_str("] },");
    }
    out.push_str("\n    ]");
}

fn generate(game: GameId, scripts: &[Script], files: usize, what: &str) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "// @generated by `cargo xtask stdlib --game {}` — do not edit.\n\
         //\n\
         // {} native scripts, extracted from {files} {what}. Signatures\n\
         // only: no function bodies, no docstrings.\n\n\
         use crate::{{Function, Param, Property, Script}};\n\n\
         #[rustfmt::skip]\n\
         pub(crate) static SCRIPTS: &[Script] = &[",
        game.as_str(),
        scripts.len()
    )
    .unwrap();
    for s in scripts {
        write!(
            out,
            "Script {{\n    name: {:?},\n    parent: {:?},\n    functions: ",
            s.name, s.parent
        )
        .unwrap();
        emit_functions(&mut out, &s.functions);
        out.push_str(",\n    events: ");
        emit_functions(&mut out, &s.events);
        out.push_str(",\n    properties: &[");
        for p in &s.properties {
            write!(
                out,
                "\n        Property {{ name: {:?}, ty: {:?}, readonly: {} }},",
                p.name, p.ty, p.readonly
            )
            .unwrap();
        }
        out.push_str("\n    ],\n},\n");
    }
    out.push_str("];\n");
    out
}

/// The native scripts of `texts`, sorted by name.
fn natives(texts: &[(PathBuf, String)], dialect: Dialect) -> Result<Vec<Script>, String> {
    let mut scripts = Vec::new();
    for (path, text) in texts {
        let parse = papyrus_syntax::parse(text, dialect);
        if !parse.ok() {
            return Err(format!("{} does not parse", path.display()));
        }
        scripts.extend(script(&parse.syntax()));
    }
    scripts.sort_by_key(|s| s.name.to_ascii_lowercase());
    scripts.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    Ok(scripts)
}

/// Writes the table `module` of `papyrus-stdlib`.
fn write_table(
    game: GameId,
    module: &str,
    scripts: &[Script],
    files: usize,
    what: &str,
) -> Result<(), String> {
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/papyrus-stdlib/src/generated")
        .join(format!("{module}.rs"));
    let code = generate(game, scripts, files, what);
    std::fs::write(&target, &code).map_err(|e| format!("{}: {e}", target.display()))?;

    let functions: usize = scripts.iter().map(|s| s.functions.len()).sum();
    let events: usize = scripts.iter().map(|s| s.events.len()).sum();
    let properties: usize = scripts.iter().map(|s| s.properties.len()).sum();
    eprintln!(
        "{module}: {} native scripts, {functions} functions, {events} events, {properties} \
         properties, {} KiB → {}",
        scripts.len(),
        code.len() / 1024,
        target.display()
    );
    Ok(())
}

fn stdlib(game: GameId, from: &Path) -> Result<(), String> {
    let files =
        discover(&[from.to_owned()], DiscoverOptions::default()).map_err(|e| e.to_string())?;
    let dialect = Dialect::new(game);
    let mut texts = Vec::new();
    for path in files {
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = decode(&bytes).text;
        texts.push((path, text));
    }

    match game {
        // Skyrim's sources usually come with SKSE's: one table leaves them
        // out, the other keeps them.
        GameId::Skyrim => {
            let vanilla: Vec<(PathBuf, String)> = texts
                .iter()
                .filter_map(|(path, text)| Some((path.clone(), vanilla_part(text)?.to_owned())))
                .collect();
            let only = natives(&vanilla, dialect)?;
            write_table(game, "skyrim", &only, vanilla.len(), "vanilla sources")?;
            let extended = natives(&texts, dialect)?;
            write_table(
                game,
                "skyrim_skse",
                &extended,
                texts.len(),
                "sources, SKSE's included",
            )
        }
        GameId::Fallout4 | GameId::Fallout76 => {
            let scripts = natives(&texts, dialect)?;
            write_table(game, "fallout4", &scripts, texts.len(), "vanilla sources")
        }
        GameId::Starfield => {
            let scripts = natives(&texts, dialect)?;
            write_table(game, "starfield", &scripts, texts.len(), "vanilla sources")
        }
    }
}
