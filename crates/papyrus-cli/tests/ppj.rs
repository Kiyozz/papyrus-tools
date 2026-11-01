//! Homestead, a fake mod described by `.ppj` files, from
//! `tests/fixtures`: the Creation Kit's format for Fallout 4 and Starfield,
//! Pyro's for Skyrim. Each test copies the mod and one file of its `projects`
//! folder, as `mod.ppj`, into a temporary folder.
//!
//! The compiler tests only run where the games are installed: set
//! `PAPYRUS_CORPUS_<GAME>`, as for the corpus tests, and
//! `PAPYRUS_COMPILER_<GAME>` to its `PapyrusCompiler.exe`, for `SKYRIM`,
//! `FALLOUT4` and `STARFIELD`. Fallout 4 and Starfield then compare
//! `papyrus build` with the official compiler given the same `.ppj`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// A temporary copy of the mod `fixture`, its project being `project`.
fn prepare(test: &str, fixture: &str, project: &str, sources: &str, flags: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("papyrus-ppj-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let fixture = Path::new(FIXTURES).join(fixture);
    copy(&fixture.join("Source"), &root.join("Source"));
    copy(&fixture.join("Lib"), &root.join("Lib"));
    let text = std::fs::read_to_string(fixture.join("projects").join(project)).unwrap();
    let text = text
        .replace("GAME_SOURCES", sources)
        .replace("GAME_FLAGS", flags);
    std::fs::write(root.join("mod.ppj"), text).unwrap();
    root
}

fn papyrus(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .args(args)
        .current_dir(root)
        .env("NO_COLOR", "1")
        .output()
        .expect("papyrus runs")
}

/// The scripts `check` went through, by the problems it found there, and
/// what it said on standard error.
fn check(root: &Path) -> (BTreeSet<String>, String) {
    let found = papyrus(root, &["check", "--message-format", "json"]);
    let problems: serde_json::Value = serde_json::from_slice(&found.stdout).unwrap();
    let scripts = problems
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| Path::new(p["path"].as_str()?).file_stem())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    (scripts, String::from_utf8_lossy(&found.stderr).into_owned())
}

/// The `.pex` files under `dir`, relative to it.
fn compiled(dir: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "pex") {
                let relative = path.strip_prefix(root).unwrap();
                out.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn a_folder_of_the_ppj_makes_the_project() {
    let root = prepare(
        "whole",
        "homestead",
        "whole.ppj",
        "Z:/Fallout 4/Base",
        "Institute_Papyrus_Flags.flg",
    );
    let (scripts, stderr) = check(&root);
    assert!(stderr.contains("5 file(s) checked"), "{stderr}");
    // The script that does not compile; the fragment stays out.
    assert!(scripts.contains("Draft"), "{scripts:?}");
    assert!(
        stderr.contains("import `Z:/Fallout 4/Base`: not found, skipped"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_list_of_scripts_makes_the_project() {
    let root = prepare(
        "subset",
        "homestead",
        "subset.ppj",
        "Z:/Fallout 4/Base",
        "Institute_Papyrus_Flags.flg",
    );
    let (scripts, stderr) = check(&root);
    assert!(stderr.contains("3 file(s) checked"), "{stderr}");
    assert!(!scripts.contains("Draft"), "{scripts:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn no_recurse_stops_at_the_folder() {
    let root = prepare(
        "norecurse",
        "homestead",
        "norecurse.ppj",
        "Z:/Fallout 4/Base",
        "Institute_Papyrus_Flags.flg",
    );
    let (_, stderr) = check(&root);
    assert!(stderr.contains("1 file(s) checked"), "{stderr}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pyro_s_project_is_read_and_its_events_never_run() {
    let root = prepare("pyro", "homestead-skyrim", "pyro.ppj", "Z:/Skyrim", "");
    let (scripts, stderr) = check(&root);
    assert!(stderr.contains("1 file(s) checked"), "{stderr}");
    assert!(!scripts.contains("HomesteadDraft"), "{scripts:?}");
    assert!(stderr.contains("https://github.com/example/framework/tree/main/Source"));

    let made = papyrus(&root, &["init"]);
    assert!(made.status.success());
    let stderr = String::from_utf8_lossy(&made.stderr);
    for left in ["Packages", "ZipFiles", "PostBuildEvent", "Anonymize", "Zip"] {
        assert!(stderr.contains(left), "{left}: {stderr}");
    }
    let toml = std::fs::read_to_string(root.join("papyrus.toml")).unwrap();
    assert!(toml.contains("ppj = \"mod.ppj\"") && toml.contains("game = \"skyrim\""));

    let (_, stderr) = check(&root);
    assert!(stderr.contains("1 file(s) checked"), "{stderr}");
    assert!(!root.join("ran.txt").exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// The game's sources and compiler, where it is installed.
fn installed(game: &str, base: &str) -> Option<(String, PathBuf)> {
    let sources = std::env::var(format!("PAPYRUS_CORPUS_{game}")).ok();
    let compiler = std::env::var_os(format!("PAPYRUS_COMPILER_{game}"));
    let (Some(sources), Some(compiler)) = (sources, compiler) else {
        eprintln!("· {game} skipped");
        return None;
    };
    Some((format!("{sources}{base}"), compiler.into()))
}

/// `papyrus build` and the official compiler on each project of Homestead:
/// the same verdict, the same files.
fn same_as_the_official_compiler(game: &str, base: &str, flags: &str) {
    let Some((sources, compiler)) = installed(game, base) else {
        return;
    };
    for (project, expected) in [
        (
            "whole.ppj",
            &[
                "Fragments/Quests/QF_Homestead_01000F99.pex",
                "Homestead/Legacy/Old.pex",
                "Homestead/Main.pex",
                "Homestead/Util.pex",
                "Plain.pex",
            ][..],
        ),
        (
            "subset.ppj",
            &["Homestead/Main.pex", "Homestead/Util.pex", "Plain.pex"][..],
        ),
        ("norecurse.ppj", &["Plain.pex"][..]),
    ] {
        let test = format!("{game}-{project}");
        let root = prepare(&test, "homestead", project, &sources, flags);
        let official = Command::new(&compiler)
            .arg("mod.ppj")
            .current_dir(&root)
            .output()
            .expect("the compiler runs");
        let theirs = compiled(&root.join("Build"));
        std::fs::remove_dir_all(root.join("Build")).unwrap();

        let compiler = compiler.to_string_lossy();
        let ours = papyrus(&root, &["build", "--compiler", &compiler]);
        let stderr = String::from_utf8_lossy(&ours.stderr);
        assert_eq!(compiled(&root.join("Build")), theirs, "{project}: {stderr}");
        assert_eq!(
            theirs.iter().map(String::as_str).collect::<Vec<_>>(),
            expected,
            "{project}"
        );
        assert_eq!(
            ours.status.success(),
            official.status.success(),
            "{project}: {stderr}"
        );
        // Only `Draft`, which does not compile, can make them fail.
        assert_eq!(ours.status.success(), project != "whole.ppj", "{stderr}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[test]
fn fallout4_agrees_with_the_official_compiler() {
    same_as_the_official_compiler("FALLOUT4", "/Base", "Institute_Papyrus_Flags.flg");
}

#[test]
fn starfield_agrees_with_the_official_compiler() {
    same_as_the_official_compiler("STARFIELD", "", "Starfield_Papyrus_Flags.flg");
}

/// A project of `papyrus.toml`, in which `script` alone is compiled: the
/// script next to it that does not compile stays out of it, and the folders
/// of the others are still there for what it uses.
fn alone(game: &str, config: &str, fixture: &str, base: &str, script: &str, expected: &str) {
    let Some((sources, compiler)) = installed(game, base) else {
        return;
    };
    let project = if fixture == "homestead" {
        "whole.ppj"
    } else {
        "pyro.ppj"
    };
    let root = prepare(&format!("alone-{game}"), fixture, project, "", "");
    std::fs::remove_file(root.join("mod.ppj")).unwrap();
    let config = format!("{config}\nimports = [\"{sources}\"]\n[build]\noutput = \"Build\"\n");
    std::fs::write(root.join("papyrus.toml"), config).unwrap();
    let compiler = compiler.to_string_lossy();
    let built = papyrus(&root, &["build", "--compiler", &compiler, script]);
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(built.status.success(), "{stderr}");
    assert!(stderr.contains("compiling 1 script(s)"), "{stderr}");
    assert_eq!(
        compiled(&root.join("Build")),
        BTreeSet::from([expected.to_owned()])
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fallout4_compiles_a_script_alone() {
    alone(
        "FALLOUT4",
        "game = \"fallout4\"",
        "homestead",
        "/Base",
        "Source/Homestead/Main.psc",
        "Homestead/Main.pex",
    );
}

#[test]
fn starfield_compiles_a_script_alone() {
    alone(
        "STARFIELD",
        "game = \"starfield\"",
        "homestead",
        "",
        "Source/Homestead/Main.psc",
        "Homestead/Main.pex",
    );
}

#[test]
fn skyrim_compiles_a_script_alone() {
    alone(
        "SKYRIM",
        "game = \"skyrim\"",
        "homestead-skyrim",
        "",
        "Source/HomesteadMain.psc",
        "HomesteadMain.pex",
    );
}

/// Skyrim's compiler cannot read a `.ppj`: only `papyrus build` can.
#[test]
fn skyrim_builds_what_pyro_s_project_lists() {
    let Some((sources, compiler)) = installed("SKYRIM", "") else {
        return;
    };
    let root = prepare("skyrim", "homestead-skyrim", "pyro.ppj", &sources, "");
    let compiler = compiler.to_string_lossy();
    let built = papyrus(&root, &["build", "--compiler", &compiler]);
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(built.status.success(), "{stderr}");
    assert_eq!(
        compiled(&root.join("Build/Scripts")),
        BTreeSet::from(["HomesteadMain.pex".to_owned()])
    );
    assert!(!root.join("ran.txt").exists());
    let _ = std::fs::remove_dir_all(&root);
}
