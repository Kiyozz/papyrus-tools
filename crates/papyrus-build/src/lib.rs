//! Building Papyrus scripts: drives the Creation Kit's `PapyrusCompiler.exe`
//! or [Caprica](https://github.com/Orvid/Caprica), and reads back what they
//! report.
//!
//! The compiler is given the project's source folders, its imports, then the
//! game's own sources, found next to the official compiler. Each source
//! folder is compiled whole, with `-all`: fragments included, and for
//! Fallout 4 and Starfield the namespaces of its subfolders.

#![forbid(unsafe_code)]

mod output;

use std::path::{Path, PathBuf};
use std::process::Command;

use papyrus_dialect::GameId;

pub use output::{Message, Position, Severity, parse, range_of};

/// Which compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerKind {
    /// The Creation Kit's `PapyrusCompiler.exe`.
    PapyrusCompiler,
    /// Caprica, without its language extensions.
    Caprica,
}

/// A compiler on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiler {
    /// The executable.
    pub path: PathBuf,
    /// What it is, by its name.
    pub kind: CompilerKind,
}

/// The game folders of a default Steam installation.
const STEAM: &str = "C:/Program Files (x86)/Steam/steamapps/common";

impl Compiler {
    /// The compiler at `path`: Caprica if its name says so, else the
    /// Creation Kit's.
    pub fn new(path: PathBuf) -> Compiler {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let kind = if name.contains("caprica") {
            CompilerKind::Caprica
        } else {
            CompilerKind::PapyrusCompiler
        };
        Compiler { path, kind }
    }

    /// The Creation Kit's compiler of a default Steam installation of
    /// `game`, if there is one.
    pub fn find(game: GameId) -> Option<Compiler> {
        let relative = match game {
            GameId::Skyrim => &[
                "Skyrim Special Edition/Papyrus Compiler",
                "Skyrim/Papyrus Compiler",
            ][..],
            GameId::Fallout4 => &["Fallout 4/Papyrus Compiler"],
            GameId::Fallout76 => &[],
            GameId::Starfield => &["Starfield/Tools/Papyrus Compiler"],
        };
        relative
            .iter()
            .map(|dir| Path::new(STEAM).join(dir).join("PapyrusCompiler.exe"))
            .find(|path| path.is_file())
            .map(Compiler::new)
    }

    /// The game's own sources, next to the Creation Kit's compiler; `None`
    /// for Caprica, which ships without a game.
    pub fn game_sources(&self, game: GameId) -> Option<PathBuf> {
        if self.kind != CompilerKind::PapyrusCompiler {
            return None;
        }
        // `<game>/Papyrus Compiler`, or `<game>/Tools/Papyrus Compiler`.
        let dir = self.path.parent()?;
        let root = match dir.parent()? {
            tools
                if tools
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("Tools")) =>
            {
                tools.parent()?
            }
            root => root,
        };
        let candidates: &[&str] = match game {
            // Special Edition, then Legendary Edition.
            GameId::Skyrim => &["Data/Source/Scripts", "Data/Scripts/Source"],
            GameId::Fallout4 | GameId::Fallout76 => &["Data/Scripts/Source/Base"],
            GameId::Starfield => &["Data/Scripts/Source"],
        };
        candidates
            .iter()
            .map(|c| root.join(c))
            .find(|path| path.is_dir())
    }
}

/// The flags file a game's scripts are compiled with, looked up in the
/// import folders.
pub fn vanilla_flags(game: GameId) -> &'static str {
    match game {
        GameId::Skyrim => "TESV_Papyrus_Flags.flg",
        GameId::Fallout4 | GameId::Fallout76 => "Institute_Papyrus_Flags.flg",
        GameId::Starfield => "Starfield_Papyrus_Flags.flg",
    }
}

/// What to build, and how.
#[derive(Debug, Clone)]
pub struct Job {
    /// The target.
    pub game: GameId,
    /// The folders to compile whole.
    pub roots: Vec<PathBuf>,
    /// Where the compiler looks for other scripts, in order: the roots,
    /// then these.
    pub imports: Vec<PathBuf>,
    /// The flags file: a path, or a name looked up in the imports.
    pub flags: PathBuf,
    /// Where the `.pex` files go.
    pub output: PathBuf,
    /// Optimize the bytecode.
    pub optimize: bool,
    /// Strip `DebugOnly` code (Fallout 4 onwards).
    pub release: bool,
    /// Strip `BetaOnly` code (Fallout 4 onwards).
    pub final_: bool,
}

impl Job {
    /// The import folders as the compiler takes them: the roots first,
    /// without duplicates.
    fn import_list(&self) -> String {
        let mut seen: Vec<String> = Vec::new();
        for dir in self.roots.iter().chain(&self.imports) {
            let text = dir.display().to_string();
            if !seen.iter().any(|s| s.eq_ignore_ascii_case(&text)) {
                seen.push(text);
            }
        }
        seen.join(";")
    }

    /// The flags file, found in the imports when only named.
    fn flags_path(&self) -> PathBuf {
        if self.flags.components().count() > 1 || self.flags.is_absolute() {
            return self.flags.clone();
        }
        self.roots
            .iter()
            .chain(&self.imports)
            .map(|dir| dir.join(&self.flags))
            .find(|path| path.is_file())
            .unwrap_or_else(|| self.flags.clone())
    }
}

/// The command compiling `root`.
pub fn command(compiler: &Compiler, job: &Job, root: &Path) -> Command {
    let mut command = Command::new(&compiler.path);
    let flags = job.flags_path();
    let newer = job.game.rank() >= 1;
    match compiler.kind {
        CompilerKind::PapyrusCompiler => {
            command
                .arg(root)
                .arg("-all")
                .arg("-quiet")
                .arg(format!("-flags={}", flags.display()))
                .arg(format!("-import={}", job.import_list()))
                .arg(format!("-output={}", job.output.display()));
            if job.optimize {
                command.arg("-optimize");
            }
            if newer && job.release {
                command.arg("-release");
            }
            if newer && job.final_ {
                command.arg("-final");
            }
        }
        CompilerKind::Caprica => {
            command
                .arg("--game")
                .arg(job.game.as_str())
                .arg("--import")
                .arg(job.import_list())
                .arg("--flags")
                .arg(&flags)
                .arg("--output")
                .arg(&job.output)
                .arg("--quiet");
            // Skyrim has no namespaces: its folders are compiled one by one.
            if newer {
                command.arg("--recurse");
            }
            if job.optimize {
                command.arg("--optimize");
            }
            if newer && job.release {
                command.arg("--release");
            }
            if newer && job.final_ {
                command.arg("--final");
            }
            command.arg(root);
        }
    }
    command
}

/// The folders holding the scripts, `(path, script name)`: for a namespaced
/// script, `MyMod:Quests:Main` in `…/MyMod/Quests/Main.psc`, the folder
/// above its namespace.
pub fn roots<'a>(scripts: impl IntoIterator<Item = (&'a Path, &'a str)>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for (path, name) in scripts {
        let mut dir = path.parent().map(Path::to_owned).unwrap_or_default();
        for _ in 0..name.matches(':').count() {
            dir.pop();
        }
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out.sort();
    out
}

/// What a compilation reported.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    /// The compiler's own verdict.
    pub success: bool,
    /// The located problems.
    pub messages: Vec<Message>,
    /// The other lines, which do not say where.
    pub other: Vec<String>,
}

/// Compiles `root`.
pub fn compile(compiler: &Compiler, job: &Job, root: &Path) -> std::io::Result<Outcome> {
    std::fs::create_dir_all(&job.output)?;
    let output = command(compiler, job, root).output()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let (messages, other) = parse(compiler.kind, &text);
    Ok(Outcome {
        success: output.status.success(),
        messages,
        other,
    })
}

/// Drops the roots inside another one: from Fallout 4 on, the compilers
/// recurse into them. Skyrim keeps them apart: it has no namespaces, and its
/// compiler does not recurse.
pub fn outermost(game: GameId, roots: Vec<PathBuf>) -> Vec<PathBuf> {
    if game.rank() == 0 {
        return roots;
    }
    roots
        .iter()
        .filter(|root| {
            !roots
                .iter()
                .any(|other| other != *root && root.starts_with(other))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_comes_from_the_name() {
        assert_eq!(
            Compiler::new("C:/Tools/Caprica.exe".into()).kind,
            CompilerKind::Caprica
        );
        assert_eq!(
            Compiler::new("D:/Skyrim/Papyrus Compiler/PapyrusCompiler.exe".into()).kind,
            CompilerKind::PapyrusCompiler
        );
    }

    #[test]
    fn namespaced_scripts_root_above_their_namespace() {
        let scripts = [
            (
                Path::new("mod/Source/MyMod/Quests/Main.psc"),
                "MyMod:Quests:Main",
            ),
            (Path::new("mod/Source/Flat.psc"), "Flat"),
            (Path::new("mod/Source/Fragments/QF_A.psc"), "Fragments:QF_A"),
            (Path::new("mod/Other/Thing.psc"), "Thing"),
        ];
        assert_eq!(
            roots(scripts),
            [PathBuf::from("mod/Other"), PathBuf::from("mod/Source")]
        );
    }

    #[test]
    fn recursing_compilers_keep_the_outermost_roots() {
        let all = vec![PathBuf::from("mod/Source"), PathBuf::from("mod/Source/Sub")];
        assert_eq!(
            outermost(GameId::Fallout4, all.clone()),
            [PathBuf::from("mod/Source")]
        );
        assert_eq!(outermost(GameId::Skyrim, all.clone()), all);
    }

    #[test]
    fn papyrus_compiler_arguments() {
        let job = Job {
            game: GameId::Fallout4,
            roots: vec!["src".into()],
            imports: vec!["lib".into(), "SRC".into()],
            flags: "Institute_Papyrus_Flags.flg".into(),
            output: "out".into(),
            optimize: true,
            release: true,
            final_: false,
        };
        let compiler = Compiler::new("PapyrusCompiler.exe".into());
        let command = command(&compiler, &job, Path::new("src"));
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "src",
                "-all",
                "-quiet",
                "-flags=Institute_Papyrus_Flags.flg",
                "-import=src;lib",
                "-output=out",
                "-optimize",
                "-release",
            ]
        );
    }

    #[test]
    fn caprica_arguments() {
        let job = Job {
            game: GameId::Skyrim,
            roots: vec!["src".into()],
            imports: vec!["vanilla".into()],
            flags: "TESV_Papyrus_Flags.flg".into(),
            output: "out".into(),
            optimize: false,
            release: true,
            final_: true,
        };
        let compiler = Compiler::new("caprica.exe".into());
        let command = command(&compiler, &job, Path::new("src"));
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        // Skyrim knows neither `-release` nor `-final`.
        assert_eq!(
            args,
            [
                "--game",
                "skyrim",
                "--import",
                "src;vanilla",
                "--flags",
                "TESV_Papyrus_Flags.flg",
                "--output",
                "out",
                "--quiet",
                "src",
            ]
        );
    }
}
