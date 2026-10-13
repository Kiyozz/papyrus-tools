//! Several projects in one command: each script follows its own papyrus.toml,
//! and only sees the scripts of its own project.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("papyrus-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

fn check(root: &Path) -> String {
    run(root, &[])
}

fn run(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .arg("check")
        .args(args)
        .arg(root)
        .env("NO_COLOR", "1")
        .output()
        .expect("papyrus runs");
    String::from_utf8(output.stderr).unwrap()
}

#[test]
fn each_project_keeps_its_settings_and_its_scripts() {
    // Two projects declare a `MyBase` script, with different events. Each
    // child must be checked against its own `MyBase`, for its own game.
    let root = tree(
        "projects",
        &[
            ("skyrim/papyrus.toml", "game = \"skyrim\"\n"),
            (
                "skyrim/MyBase.psc",
                "ScriptName MyBase extends ObjectReference\nEvent OnRitualDone()\nEndEvent\n",
            ),
            (
                "skyrim/Child.psc",
                "ScriptName Child extends MyBase\nEvent OnRitualDone()\nEndEvent\n",
            ),
            (
                "fallout/papyrus.toml",
                "game = \"fallout4\"\n[lints]\n\"naming/locals\" = \"allow\"\n",
            ),
            (
                "fallout/MyBase.psc",
                "ScriptName MyBase extends ObjectReference\nEvent OnRitualDune()\nEndEvent\n",
            ),
            (
                "fallout/Child.psc",
                "ScriptName Child extends MyBase\nEvent OnRitualDone()\nEndEvent\nFunction F()\n  int HP = 1\nEndFunction\n",
            ),
        ],
    );
    let stderr = check(&root);

    // The Skyrim child matches its own base; the Fallout 4 one is one letter
    // off its own, and its project allows badly named locals.
    let events = stderr.matches("PAP004").count();
    assert_eq!(events, 1, "{stderr}");
    assert!(stderr.contains("`OnRitualDune`"), "{stderr}");
    assert!(!stderr.contains("PAP101"), "{stderr}");
    let fallout_child = root.join("fallout").join("Child.psc");
    assert!(
        stderr.contains(&fallout_child.display().to_string()),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn imported_scripts_relay_extends_but_are_not_checked() {
    let root = tree(
        "imports",
        &[
            (
                "mod/papyrus.toml",
                "game = \"skyrim\"
imports = [\"../lib\"]
",
            ),
            (
                "mod/Child.psc",
                "ScriptName Child extends LibBase
Event OnRitualDome()
EndEvent
",
            ),
            (
                "lib/LibBase.psc",
                "ScriptName LibBase extends ObjectReference
Event OnRitualDone()
EndEvent
int Function f()
	int BAD = 1
EndFunction
",
            ),
        ],
    );
    let stderr = check(&root.join("mod"));
    // The typo is found through the imported parent, which is not checked.
    assert!(stderr.contains("`OnRitualDone`"), "{stderr}");
    assert!(!stderr.contains("LibBase.psc"), "{stderr}");
    assert!(stderr.contains("1 file(s) checked"), "{stderr}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unsafe_fixes_need_unsafe() {
    let root = tree(
        "unsafe",
        &[(
            "A.psc",
            "ScriptName A
Actor Property MyActor Auto
Function F()
	MyActor.Kill()
EndFunction
",
        )],
    );
    let file = root.join("A.psc");
    let stderr = run(&root, &["--fix"]);
    assert!(stderr.contains("--fix --unsafe"), "{stderr}");
    assert!(std::fs::read_to_string(&file).unwrap().contains("MyActor"));

    run(&root, &["--fix", "--unsafe"]);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        !text.contains("MyActor") && text.contains("myActor.Kill()"),
        "{text}"
    );

    // `--unsafe` alone is refused.
    let refused = run(&root, &["--unsafe"]);
    assert!(refused.contains("--fix"), "{refused}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn calls_are_checked_across_files_and_imports() {
    let root = tree(
        "calls",
        &[
            (
                "mod/papyrus.toml",
                "game = \"skyrim\"\nimports = [\"../lib\"]\n",
            ),
            (
                "mod/Quest1.psc",
                "ScriptName Quest1 extends Quest\nFunction Start(int stage)\nEndFunction\n",
            ),
            (
                "mod/Quest2.psc",
                "ScriptName Quest2 extends Quest\nimport Tools\nQuest1 Property first Auto\nEvent OnInit()\n  first.Start()\n  Help(1.5)\nEndEvent\n",
            ),
            (
                "lib/Tools.psc",
                "ScriptName Tools\nFunction Help(int count) Global\nEndFunction\n",
            ),
        ],
    );
    let stderr = check(&root.join("mod"));
    assert!(stderr.contains("PAP005"), "{stderr}");
    assert!(
        stderr.contains("`Start` is missing argument `stage`"),
        "{stderr}"
    );
    assert!(stderr.contains("PAP007"), "{stderr}");
    assert!(
        stderr.contains("`Help` expects `int` for `count`, found `float`"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
