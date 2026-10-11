//! Several projects in one command: each script follows its own papyrus.toml,
//! and only sees the scripts of its own project.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tree(files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("papyrus-cli-projects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

fn check(root: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .args(["check"])
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
    let root = tree(&[
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
    ]);
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
