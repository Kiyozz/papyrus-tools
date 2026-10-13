//! Papyrus lints.
//!
//! A lint only reports what the compiler lets through, or only says at
//! compile time — never what missing information prevents from deciding.
//! Each lint has a stable code (`PAP101`) and a readable name
//! (`naming/locals`), usable interchangeably in `papyrus.toml` and in
//! suppression comments:
//!
//! ```papyrus
//! int HP = 0 ; papyrus: allow(naming/locals)
//!
//! ; papyrus: allow(PAP001)
//! string s = "café"
//! ```

#![forbid(unsafe_code)]

mod case;
mod rules;
mod suppress;
mod workspace;

use std::collections::BTreeMap;

use papyrus_dialect::GameId;
use papyrus_project::flags::FlagDef;
use papyrus_project::{Decoded, Level};
use papyrus_syntax::{SyntaxNode, TextRange, lex};

pub use case::{to_lower_camel, to_pascal};
pub use rules::vanilla_flags;
pub use workspace::{ScriptSummary, Workspace};

/// What we know about a lint.
#[derive(Debug)]
pub struct Lint {
    /// Stable code, `PAP101`.
    pub code: &'static str,
    /// Readable name, `naming/locals`.
    pub name: &'static str,
    /// Level without configuration.
    pub default: Level,
    /// What the lint checks, in one sentence.
    pub description: &'static str,
}

/// `PAP001`: text the target's compiler will corrupt.
pub static ENCODING_LOSSY: Lint = Lint {
    code: "PAP001",
    name: "encoding/lossy",
    default: Level::Warn,
    description: "string or docstring the target's compiler will silently corrupt",
};

/// `PAP002`: flag set on an object the `.flg` does not allow.
pub static FLAGS_PLACEMENT: Lint = Lint {
    code: "PAP002",
    name: "flags/placement",
    default: Level::Error,
    description: "flag the game's .flg file does not allow on this object",
};

/// `PAP003`: unknown name in `; papyrus: allow(...)`.
pub static ALLOW_UNKNOWN: Lint = Lint {
    code: "PAP003",
    name: "allow/unknown",
    default: Level::Warn,
    description: "unknown lint name in `; papyrus: allow(...)`, which therefore suppresses nothing",
};

/// `PAP004`: event the engine will never call.
pub static EVENTS_UNKNOWN: Lint = Lint {
    code: "PAP004",
    name: "events/unknown",
    default: Level::Warn,
    description: "event the script never receives, most often a typo",
};

/// `PAP005`: call with too few or too many arguments.
pub static CALLS_ARITY: Lint = Lint {
    code: "PAP005",
    name: "calls/arity",
    default: Level::Error,
    description: "call with too few or too many arguments",
};

/// `PAP006`: named argument the callee does not declare.
pub static CALLS_NAMED_ARG: Lint = Lint {
    code: "PAP006",
    name: "calls/named-arg",
    default: Level::Warn,
    description: "named argument the callee does not declare, or already given",
};

/// `PAP007`: value of a type the compiler does not cast implicitly.
pub static TYPES_MISMATCH: Lint = Lint {
    code: "PAP007",
    name: "types/mismatch",
    default: Level::Error,
    description: "argument, assignment or return value of a type the compiler does not cast implicitly",
};

/// `PAP008`: member that does not exist.
pub static MEMBERS_UNKNOWN: Lint = Lint {
    code: "PAP008",
    name: "members/unknown",
    default: Level::Error,
    description: "function, variable, property or field that does not exist, most often a typo",
};

/// `PAP009`: typed function that can end without `Return`.
pub static RETURNS_MISSING: Lint = Lint {
    code: "PAP009",
    name: "returns/missing",
    default: Level::Warn,
    description: "typed function that can reach its end without `Return`, and return the default value",
};

/// `PAP010`: `GotoState` to a state that does not exist.
pub static STATES_UNKNOWN: Lint = Lint {
    code: "PAP010",
    name: "states/unknown",
    default: Level::Error,
    description: "`GotoState` to a state the script does not declare, which behaves as the empty state",
};

/// `PAP011`: event whose parameters are not the engine's.
pub static EVENTS_SIGNATURE: Lint = Lint {
    code: "PAP011",
    name: "events/signature",
    default: Level::Error,
    description: "event whose parameters differ from the event it implements",
};

/// `PAP012`: read-only property assigned.
pub static PROPERTIES_READONLY: Lint = Lint {
    code: "PAP012",
    name: "properties/readonly",
    default: Level::Error,
    description: "assignment to an `AutoReadOnly` or `Const` property, or one without `Set`",
};

/// `PAP013`: instance member reached without an instance.
pub static CALLS_NOT_GLOBAL: Lint = Lint {
    code: "PAP013",
    name: "calls/not-global",
    default: Level::Error,
    description: "non-`Global` function called on its script, or `self` and members used in a `Global` function",
};

/// `PAP014`: code after `Return`.
pub static CODE_UNREACHABLE: Lint = Lint {
    code: "PAP014",
    name: "code/unreachable",
    default: Level::Warn,
    description: "statements after one that always returns, which never run",
};

/// `PAP015`: local variable never read.
pub static LOCALS_UNUSED: Lint = Lint {
    code: "PAP015",
    name: "locals/unused",
    default: Level::Warn,
    description: "local variable never read",
};

/// `PAP016`: parameter never read.
pub static PARAMS_UNUSED: Lint = Lint {
    code: "PAP016",
    name: "params/unused",
    default: Level::Warn,
    description: "parameter never read, after the last one read; events and overrides excepted",
};

/// `PAP017`: SKSE member in a project without SKSE.
pub static MEMBERS_SKSE: Lint = Lint {
    code: "PAP017",
    name: "members/skse",
    default: Level::Warn,
    description: "member only SKSE declares, in a Skyrim project that does not use SKSE",
};

/// `PAP101`: locals and parameters in lowerCamelCase.
pub static NAMING_LOCALS: Lint = Lint {
    code: "PAP101",
    name: "naming/locals",
    default: Level::Warn,
    description: "locals and parameters in lowerCamelCase",
};

/// `PAP102`: script variables, properties and fields in lowerCamelCase.
pub static NAMING_VARIABLES: Lint = Lint {
    code: "PAP102",
    name: "naming/variables",
    default: Level::Warn,
    description: "script variables, properties and struct fields in lowerCamelCase",
};

/// `PAP103`: functions, events and types in PascalCase.
pub static NAMING_FUNCTIONS: Lint = Lint {
    code: "PAP103",
    name: "naming/functions",
    default: Level::Warn,
    description: "functions, events, states, structs and groups in PascalCase",
};

/// All lints, in code order.
pub static LINTS: &[&Lint] = &[
    &ENCODING_LOSSY,
    &FLAGS_PLACEMENT,
    &ALLOW_UNKNOWN,
    &EVENTS_UNKNOWN,
    &CALLS_ARITY,
    &CALLS_NAMED_ARG,
    &TYPES_MISMATCH,
    &MEMBERS_UNKNOWN,
    &RETURNS_MISSING,
    &STATES_UNKNOWN,
    &EVENTS_SIGNATURE,
    &PROPERTIES_READONLY,
    &CALLS_NOT_GLOBAL,
    &CODE_UNREACHABLE,
    &LOCALS_UNUSED,
    &PARAMS_UNUSED,
    &MEMBERS_SKSE,
    &NAMING_LOCALS,
    &NAMING_VARIABLES,
    &NAMING_FUNCTIONS,
];

/// The lint designated by `key`, code or name, case-insensitively.
pub fn find(key: &str) -> Option<&'static Lint> {
    LINTS
        .iter()
        .copied()
        .find(|l| l.code.eq_ignore_ascii_case(key) || l.name.eq_ignore_ascii_case(key))
}

/// Edit distance, case-insensitive.
pub(crate) fn distance(a: &str, b: &str) -> usize {
    let a: Vec<u8> = a.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let b: Vec<u8> = b.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

/// The lint name closest to `key`, if close enough to be a typo.
pub fn closest(key: &str) -> Option<&'static str> {
    LINTS
        .iter()
        .flat_map(|l| [l.name, l.code])
        .map(|candidate| (distance(key, candidate), candidate))
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate)
}

/// "unknown lint `x`", with a suggestion when possible.
fn unknown_message(key: &str) -> String {
    match closest(key) {
        Some(name) => format!("unknown lint `{key}` — did you mean `{name}`?"),
        None => format!("unknown lint `{key}`; `papyrus lints` lists them"),
    }
}

/// The level of each lint.
#[derive(Debug, Clone)]
pub struct Levels(BTreeMap<&'static str, Level>);

impl Default for Levels {
    fn default() -> Levels {
        Levels(LINTS.iter().map(|l| (l.code, l.default)).collect())
    }
}

impl Levels {
    /// The default levels, modified by the `[lints]` section.
    pub fn from_config(lints: &BTreeMap<String, Level>) -> Result<Levels, String> {
        let mut levels = Levels::default();
        for (key, level) in lints {
            let lint = find(key).ok_or_else(|| unknown_message(key))?;
            levels.0.insert(lint.code, *level);
        }
        Ok(levels)
    }

    /// The level of `lint`.
    pub fn get(&self, lint: &Lint) -> Level {
        self.0.get(lint.code).copied().unwrap_or(lint.default)
    }
}

/// A text replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// What is replaced.
    pub range: TextRange,
    /// The new text.
    pub text: String,
}

/// A suggested fix, never applied automatically: it changes the meaning of
/// the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// What it does: « Renommer en `OnActivate` ».
    pub title: String,
    /// The replacements.
    pub edits: Vec<Edit>,
}

/// A problem found by a lint.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The lint that found it.
    pub lint: &'static Lint,
    /// Warning or error, depending on the configuration.
    pub level: Level,
    /// Where.
    pub range: TextRange,
    /// What, and how to fix it.
    pub message: String,
    /// The fix, when it is safe.
    pub fix: Option<Vec<Edit>>,
    /// A fix that compiles but may break what lies outside the scripts:
    /// applied only when asked explicitly, `papyrus check --fix --unsafe`.
    pub unsafe_fix: Option<Vec<Edit>>,
    /// Fixes to suggest, when none is safe.
    pub suggestions: Vec<Suggestion>,
}

/// What the lints need.
#[derive(Debug)]
pub struct Context<'a> {
    /// The file as decoded.
    pub decoded: &'a Decoded,
    /// The tree of `decoded.text`.
    pub root: &'a SyntaxNode,
    /// The target, if known. The lints that depend on it stay quiet without
    /// it.
    pub game: Option<GameId>,
    /// The flags of the `.flg` in effect.
    pub flags: &'a [FlagDef],
    /// The other scripts of the project.
    pub workspace: &'a Workspace,
    /// The semantic model of the project, and the file in it: the lints
    /// that follow calls and types across files stay quiet without it.
    pub model: Option<Model<'a>>,
}

impl Context<'_> {
    /// The native scripts of the target, SKSE's included when the project
    /// has them.
    pub fn library(&self) -> Option<papyrus_stdlib::Library> {
        let game = self.game?;
        let skse = self.model.is_some_and(|m| m.semantics.library().skse);
        Some(papyrus_stdlib::Library::new(game).with_skse(skse))
    }
}

/// A file within the semantic model of its project.
#[derive(Debug, Clone, Copy)]
pub struct Model<'a> {
    /// The project's model.
    pub semantics: papyrus_hir::Semantics<'a>,
    /// The file, with the text being linted.
    pub file: papyrus_hir::SourceFile,
}

/// What the rules produce, before levels and suppressions.
pub(crate) struct Finding {
    pub(crate) lint: &'static Lint,
    pub(crate) range: TextRange,
    pub(crate) message: String,
    pub(crate) fix: Option<Vec<Edit>>,
    pub(crate) unsafe_fix: Option<Vec<Edit>>,
    pub(crate) suggestions: Vec<Suggestion>,
}

/// Runs the active lints and drops what the code explicitly allowed.
pub fn run(context: &Context<'_>, levels: &Levels) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    rules::run(context, &mut findings);

    let allowed = suppress::Suppressions::collect(&context.decoded.text);
    // A misspelled `allow(...)` suppresses nothing: better say so.
    for (range, key) in &allowed.unknown {
        findings.push(Finding {
            lint: &ALLOW_UNKNOWN,
            range: *range,
            message: format!("{}; this suppression has no effect", unknown_message(key)),
            fix: None,
            unsafe_fix: None,
            suggestions: Vec::new(),
        });
    }
    let mut diagnostics: Vec<Diagnostic> = findings
        .into_iter()
        .filter_map(|f| {
            let level = levels.get(f.lint);
            if level == Level::Allow || allowed.covers(f.lint, f.range.start()) {
                return None;
            }
            Some(Diagnostic {
                lint: f.lint,
                level,
                range: f.range,
                message: f.message,
                fix: f.fix,
                unsafe_fix: f.unsafe_fix,
                suggestions: f.suggestions,
            })
        })
        .collect();
    diagnostics.sort_by_key(|d| (d.range.start(), d.lint.code));
    diagnostics
}

/// Applies the safe fixes of `diagnostics` to `source`.
///
/// Fixes only change case: the result is checked token by token, ignoring
/// case, and `None` signals that it differs. That would be a bug; the file
/// must then stay untouched.
pub fn apply_fixes(source: &str, diagnostics: &[Diagnostic]) -> Option<String> {
    apply_fixes_with(source, diagnostics, false)
}

/// Applies the safe fixes of `diagnostics` to `source`, and the unsafe ones
/// with `unsafe_fixes`. Checked like [`apply_fixes`].
pub fn apply_fixes_with(
    source: &str,
    diagnostics: &[Diagnostic],
    unsafe_fixes: bool,
) -> Option<String> {
    let mut edits: Vec<&Edit> = diagnostics
        .iter()
        .flat_map(|d| {
            let risky = d.unsafe_fix.as_ref().filter(|_| unsafe_fixes);
            d.fix.iter().chain(risky)
        })
        .flatten()
        .collect();
    edits.sort_by(|a, b| {
        (a.range.start(), a.range.end(), &a.text).cmp(&(b.range.start(), b.range.end(), &b.text))
    });
    edits.dedup();

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in edits {
        let start = usize::from(edit.range.start());
        // Two fixes on the same range: the first one wins.
        if start < cursor {
            continue;
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&edit.text);
        cursor = usize::from(edit.range.end());
    }
    out.push_str(&source[cursor..]);

    let before = lex(source);
    let after = lex(&out);
    let same = before.len() == after.len()
        && before
            .iter()
            .zip(&after)
            .all(|(a, b)| a.kind == b.kind && a.text(source).eq_ignore_ascii_case(b.text(&out)));
    same.then_some(out)
}
