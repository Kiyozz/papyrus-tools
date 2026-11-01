//! `papyrus build`: compiles each project with the Creation Kit's compiler
//! or Caprica, and reports their errors like the lints'.

use std::path::{Path, PathBuf};

use papyrus_build::{Compiler, CompilerKind, Job, Message, Severity as BuildSeverity};
use papyrus_dialect::GameId;
use papyrus_lint::ScriptSummary;
use papyrus_project::{DiscoverOptions, decode};

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

/// The script names of files, to find the folders above their namespaces.
fn named(settings: &Settings, scripts: &[PathBuf]) -> Vec<(PathBuf, String)> {
    scripts
        .iter()
        .filter_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            let parse = papyrus_syntax::parse(&decode(&bytes).text, settings.dialect());
            let summary = ScriptSummary::of(&parse.syntax())?;
            Some((path.clone(), summary.name))
        })
        .collect()
}

/// The scripts of a project in which only some were named, one by one, on the
/// command line: they still need the folders of the others.
fn whole_project(settings: &Settings) -> Vec<PathBuf> {
    let projects = papyrus_project::projects(std::slice::from_ref(&settings.root), true);
    let project = projects
        .into_iter()
        .flatten()
        .find(|project| project.root == settings.root);
    let mut scripts = project.map_or_else(Vec::new, |project| project.scripts);
    scripts.extend(settings.scripts.iter().cloned());
    scripts.sort();
    scripts.dedup();
    scripts
}

/// What to compile in `root`: the whole folder, or `scripts` one by one,
/// relative to `root`, if `partial` and they leave some of it out.
fn units(partial: bool, scripts: &[PathBuf], root: &Path, game: GameId) -> Vec<Option<PathBuf>> {
    if !partial {
        return vec![None];
    }
    // Only the compilers of the games with namespaces recurse.
    let compiled = |path: &&PathBuf| {
        path.starts_with(root) && (game.rank() >= 1 || path.parent() == Some(root))
    };
    let options = DiscoverOptions {
        include_fragments: true,
    };
    let all = papyrus_project::discover(&[root.to_owned()], options).unwrap_or_default();
    if all.iter().filter(compiled).all(|p| scripts.contains(p)) {
        return vec![None];
    }
    scripts
        .iter()
        .filter(compiled)
        .filter_map(|path| path.strip_prefix(root).ok())
        .map(|path| Some(path.to_owned()))
        .collect()
}

pub(crate) fn build(
    projects: &[Settings],
    args: &BuildArgs,
    reporter: &mut Reporter,
) -> Result<bool, String> {
    let mut success = true;
    let mut files = 0;
    let given: Vec<PathBuf> = args
        .common
        .paths
        .iter()
        .filter(|path| path.is_file())
        .filter_map(|path| std::path::absolute(path).ok())
        .collect();
    for settings in projects {
        let game = settings.game.ok_or_else(|| {
            format!(
                "{}: unknown target — set `game` in papyrus.toml, or `--game`",
                settings.root.display()
            )
        })?;
        let compiler = compiler(settings, args, game)?;
        let alone = settings.scripts.iter().all(|path| given.contains(path));
        let scripts = if alone {
            named(settings, &whole_project(settings))
        } else {
            named(settings, &settings.scripts)
        };
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
            settings.scripts.len(),
            game.display_name(),
            job.output.display()
        );
        files += settings.scripts.len();
        let partial = alone || settings.config.sources.is_some();
        for root in &roots {
            for script in units(partial, &settings.scripts, root, game) {
                let outcome = papyrus_build::compile(&compiler, &job, root, script.as_deref())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_alone_leaves_its_folder_out() {
        let root = std::env::temp_dir().join(format!("papyrus-units-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("MyNs")).unwrap();
        let (one, other) = (root.join("MyNs/One.psc"), root.join("Other.psc"));
        std::fs::write(&one, "ScriptName MyNs:One\n").unwrap();
        std::fs::write(&other, "ScriptName Other\n").unwrap();
        let game = GameId::Fallout4;

        let single = std::slice::from_ref(&one);
        let script = one.strip_prefix(&root).unwrap().to_owned();
        assert_eq!(units(true, single, &root, game), [Some(script)]);
        // Nothing left out, or nothing chosen: the compiler takes the folder.
        let both = [one.clone(), other];
        assert_eq!(units(true, &both, &root, game), [None]);
        assert_eq!(units(false, single, &root, game), [None]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
