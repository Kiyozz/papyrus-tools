//! `papyrus build`: compiles each project with the Creation Kit's compiler
//! or Caprica, and reports their errors like the lints'.

use std::path::{Path, PathBuf};

use papyrus_build::{Compiler, CompilerKind, Job, Message, Severity as BuildSeverity};
use papyrus_lint::ScriptSummary;
use papyrus_project::decode;

use crate::report::{Diagnostic, Reporter, Severity};
use crate::{BuildArgs, Settings, read};

/// The compiler of a project: the command line's, papyrus.toml's, else the
/// Creation Kit's of a default Steam installation.
fn compiler(
    settings: &Settings,
    args: &BuildArgs,
    game: papyrus_dialect::GameId,
) -> Result<Compiler, String> {
    let configured = args
        .compiler
        .clone()
        .or_else(|| settings.config.build.compiler.clone());
    match configured {
        Some(path) if path.is_file() => Ok(Compiler::new(path)),
        Some(path) => Err(format!("{}: no such compiler", path.display())),
        None => Compiler::find(game).ok_or_else(|| {
            format!(
                "{}: no compiler found — set `compiler` under `[build]` in papyrus.toml, or \
                 `--compiler`, to the Creation Kit's PapyrusCompiler.exe or to Caprica",
                settings.root.display()
            )
        }),
    }
}

/// The script names of a project's files, to find the folders above their
/// namespaces.
fn named(settings: &Settings) -> Vec<(PathBuf, String)> {
    settings
        .scripts
        .iter()
        .filter_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            let parse = papyrus_syntax::parse(&decode(&bytes).text, settings.dialect());
            let summary = ScriptSummary::of(&parse.syntax())?;
            Some((path.clone(), summary.name))
        })
        .collect()
}

pub(crate) fn build(
    projects: &[Settings],
    args: &BuildArgs,
    reporter: &mut Reporter,
) -> Result<bool, String> {
    let mut success = true;
    let mut files = 0;
    for settings in projects {
        let game = settings.game.ok_or_else(|| {
            format!(
                "{}: unknown target — set `game` in papyrus.toml, or `--game`",
                settings.root.display()
            )
        })?;
        let compiler = compiler(settings, args, game)?;
        let scripts = named(settings);
        let roots = papyrus_build::outermost(
            game,
            papyrus_build::roots(scripts.iter().map(|(p, n)| (p.as_path(), n.as_str()))),
        );
        let build = &settings.config.build;
        let mut imports = settings.config.imports.clone();
        imports.extend(compiler.game_sources(game));
        let job = Job {
            game,
            roots: roots.clone(),
            imports,
            flags: settings
                .config
                .flags
                .clone()
                .unwrap_or_else(|| papyrus_build::vanilla_flags(game).into()),
            output: args
                .output
                .clone()
                .or_else(|| build.output.clone())
                .unwrap_or_else(|| settings.root.join("Scripts")),
            optimize: args.optimize || build.optimize,
            release: args.release || build.release,
            final_: args.final_ || build.final_,
        };
        let name = match compiler.kind {
            CompilerKind::PapyrusCompiler => "PapyrusCompiler",
            CompilerKind::Caprica => "Caprica",
        };
        if job.flags_missing() {
            let hint = match compiler.kind {
                CompilerKind::Caprica => {
                    "Caprica ships without the game: add the game's sources to `imports` in papyrus.toml"
                }
                CompilerKind::PapyrusCompiler => {
                    "the game's sources are not next to the compiler: add them to `imports` in papyrus.toml, or set `flags`"
                }
            };
            return Err(format!(
                "{}: flags file {} found in no source folder — {hint}",
                settings.root.display(),
                job.flags.display()
            ));
        }
        eprintln!(
            "compiling {} script(s) for {} with {name} into {}",
            scripts.len(),
            game.display_name(),
            job.output.display()
        );
        files += scripts.len();
        for root in &roots {
            let outcome = papyrus_build::compile(&compiler, &job, root)
                .map_err(|e| format!("{}: {e}", compiler.path.display()))?;
            success &= outcome.success;
            for message in &outcome.messages {
                report(message, name, reporter);
            }
            for line in &outcome.other {
                reporter.general(line);
            }
        }
    }
    reporter.summary(files, "compiled");
    Ok(success && reporter.errors() == 0)
}

/// Shows a compiler's message on the line it names.
fn report(message: &Message, compiler: &str, reporter: &mut Reporter) {
    let path: &Path = &message.path;
    let Some(bytes) = read(path, reporter) else {
        return;
    };
    let source = decode(&bytes).text;
    let diagnostic = Diagnostic {
        severity: match message.severity {
            BuildSeverity::Error => Severity::Error,
            BuildSeverity::Warning => Severity::Warning,
        },
        code: message.code.clone().unwrap_or_else(|| compiler.to_owned()),
        message: message.text.clone(),
        range: papyrus_build::range_of(&source, message.line, message.position),
    };
    reporter.emit(path, &source, &[diagnostic]);
}
