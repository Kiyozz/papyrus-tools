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

#[test]
fn build_says_which_compiler_is_missing() {
    let root = tree(
        "build",
        &[
            (
                "mod/papyrus.toml",
                "game = \"skyrim\"\n[build]\ncompiler = \"tools/PapyrusCompiler.exe\"\n",
            ),
            ("mod/Quest1.psc", "ScriptName Quest1 extends Quest\n"),
        ],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .arg("build")
        .arg(root.join("mod"))
        .env("NO_COLOR", "1")
        .output()
        .expect("papyrus runs");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("PapyrusCompiler.exe: no such compiler"),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

const UNUSED: &str =
    "ScriptName MyQuest extends Quest\nFunction F()\n\tint unused = 1\nEndFunction\n";

fn papyrus(root: &Path, args: &[&str], stdin: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .args(args)
        .current_dir(root)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("papyrus runs");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_baseline_only_lets_new_problems_through() {
    let root = tree(
        "baseline",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            ("MyQuest.psc", UNUSED),
        ],
    );
    assert_eq!(papyrus(&root, &["check"], "").status.code(), Some(1));

    let written = papyrus(&root, &["check", "--write-baseline"], "");
    assert!(written.status.success(), "{}", text(&written.stderr));
    assert!(text(&written.stderr).contains("PAP015 locals/unused"));

    // The line moved and changed, the problem did not: still known.
    let moved = UNUSED.replace("Function F()", ";\n;\nFunction F()");
    std::fs::write(root.join("MyQuest.psc"), &moved).unwrap();
    let known = papyrus(&root, &["check"], "");
    assert!(known.status.success(), "{}", text(&known.stderr));
    assert!(text(&known.stderr).contains("1 known problem(s) hidden"));
    assert_eq!(
        papyrus(&root, &["check", "--no-baseline"], "")
            .status
            .code(),
        Some(1)
    );

    // One more in the file is new.
    let twice = moved.replace("\tint unused = 1\n", "\tint unused = 1\n\tint unused = 1\n");
    std::fs::write(root.join("MyQuest.psc"), &twice).unwrap();
    assert_eq!(papyrus(&root, &["check"], "").status.code(), Some(1));

    // Fixed: the baseline says so, and writing removes it.
    let fixed = "ScriptName MyQuest extends Quest\n";
    std::fs::write(root.join("MyQuest.psc"), fixed).unwrap();
    let gone = papyrus(&root, &["check"], "");
    assert!(gone.status.success(), "{}", text(&gone.stderr));
    assert!(text(&gone.stderr).contains("1 problem(s) of the baseline are gone"));
    assert!(
        papyrus(&root, &["check", "--write-baseline"], "")
            .status
            .success()
    );
    assert!(!root.join("papyrus-baseline.json").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checking_one_file_keeps_the_baseline_of_the_others() {
    let root = tree(
        "baseline-one",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            ("MyQuest.psc", UNUSED),
            ("Other.psc", &UNUSED.replace("MyQuest", "Other")),
        ],
    );
    assert!(
        papyrus(&root, &["check", "--write-baseline"], "")
            .status
            .success()
    );
    let args = ["check", "--write-baseline", "Other.psc"];
    assert!(papyrus(&root, &args, "").status.success());
    assert!(papyrus(&root, &["check"], "").status.success());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_removes_what_does_nothing() {
    let script = "ScriptName MyQuest extends Quest\nImport Utility\nint unread\nFunction F(bool flag)\n\tint a = 1\n\tint b = Compute()\n\tint c = 2\n\tc = c\n\tif flag == True\n\t\tDebug.Trace(\"x\")\n\tendif\nEndFunction\nint Function Compute()\n\treturn 1\nEndFunction\n";
    let root = tree(
        "cleanups",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            ("MyQuest.psc", script),
        ],
    );
    papyrus(&root, &["check", "--fix"], "");
    // `c` goes with its assignment; a call stays, for its effects.
    assert_eq!(
        std::fs::read_to_string(root.join("MyQuest.psc")).unwrap(),
        "ScriptName MyQuest extends Quest\nFunction F(bool flag)\n\tint b = Compute()\n\tif flag\n\t\tDebug.Trace(\"x\")\n\tendif\nEndFunction\nint Function Compute()\n\treturn 1\nEndFunction\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_unsafe_renames_across_files() {
    let base = "ScriptName Base extends Quest\nFunction do_thing()\nEndFunction\nint Property Hit_Points Auto\nint Taken_Name\nint takenName\nFunction G()\n\tHit_Points = Taken_Name + takenName\nEndFunction\n";
    let user =
        "ScriptName User extends Base\nEvent OnInit()\n\tdo_thing()\n\tHit_Points = 1\nEndEvent\n";
    let root = tree(
        "renames",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            ("Base.psc", base),
            ("User.psc", user),
        ],
    );
    // `--fix` alone leaves names to `--unsafe`.
    papyrus(&root, &["check", "--fix"], "");
    assert_eq!(
        std::fs::read_to_string(root.join("User.psc")).unwrap(),
        user
    );

    papyrus(&root, &["check", "--fix", "--unsafe"], "");
    let base = std::fs::read_to_string(root.join("Base.psc")).unwrap();
    assert!(base.contains("Function DoThing()"), "{base}");
    assert!(base.contains("Property hitPoints"), "{base}");
    // Its new name is taken: renaming would merge the two.
    assert!(base.contains("int Taken_Name"), "{base}");
    assert_eq!(
        std::fs::read_to_string(root.join("User.psc")).unwrap(),
        user.replace("do_thing", "DoThing")
            .replace("Hit_Points", "hitPoints")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reports_go_to_standard_output() {
    let root = tree(
        "reports",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            ("MyQuest.psc", UNUSED),
        ],
    );
    let json = papyrus(&root, &["check", "--message-format", "json"], "");
    assert_eq!(json.status.code(), Some(1));
    let problems: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(problems[0]["code"], "PAP015");
    assert_eq!(problems[0]["name"], "locals/unused");
    assert_eq!(problems[0]["line"], 3);
    assert_eq!(problems[0]["column"], 6);

    let sarif = papyrus(&root, &["check", "--message-format", "sarif"], "");
    let log: serde_json::Value = serde_json::from_slice(&sarif.stdout).unwrap();
    assert_eq!(log["version"], "2.1.0");
    let result = &log["runs"][0]["results"][0];
    assert_eq!(result["ruleId"], "PAP015");
    assert_eq!(result["level"], "warning");
    let location = &result["locations"][0]["physicalLocation"];
    assert_eq!(location["artifactLocation"]["uri"], "MyQuest.psc");
    assert_eq!(location["region"]["startLine"], 3);
    // What code scanning requires of a rule.
    for rule in log["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
    {
        assert!(rule["fullDescription"]["text"].is_string(), "{rule}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// A mod described by a `.ppj`, with a script it leaves out.
fn ppj_project(name: &str) -> PathBuf {
    let ugly = "ScriptName Listed extends Quest\nFunction F()\n  int unused = 1\nEndFunction\n";
    tree(
        name,
        &[
            ("mod/Source/Listed.psc", ugly),
            (
                "mod/Source/Left.psc",
                "ScriptName Left extends Quest\nFunction G()\n  int unused = 1\nEndFunction\n",
            ),
            ("mod/Lib/Base.psc", "ScriptName Base extends Quest\n"),
            (
                "mod/mod.ppj",
                r#"<PapyrusProject xmlns="PapyrusProject.xsd" Game="sse">
                  <Imports><Import>Source</Import><Import>Lib</Import><Import>Z:\none</Import></Imports>
                  <Scripts><Script>Listed.psc</Script></Scripts>
                  <PostBuildEvent><Command>exit 1</Command></PostBuildEvent>
                </PapyrusProject>"#,
            ),
        ],
    )
}

#[test]
fn a_ppj_is_a_configuration() {
    let root = ppj_project("ppj");
    let found = papyrus(&root.join("mod"), &["check"], "");
    let stdout = String::from_utf8_lossy(&found.stdout);
    let stderr = String::from_utf8_lossy(&found.stderr);
    assert!(
        stdout.contains("Listed.psc") || stderr.contains("Listed.psc"),
        "{stdout}{stderr}"
    );
    assert!(
        !stdout.contains("Left.psc") && !stderr.contains("Left.psc"),
        "{stdout}{stderr}"
    );
    assert!(
        stderr.contains("import `Z:\\none`: not found, skipped"),
        "{stderr}"
    );
    assert!(stderr.contains("1 file(s) checked"), "{stderr}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn init_names_the_ppj() {
    let root = ppj_project("ppj-init");
    let dir = root.join("mod");
    let made = papyrus(&dir, &["init"], "");
    assert!(made.status.success());
    let stderr = String::from_utf8_lossy(&made.stderr);
    assert!(stderr.contains("PostBuildEvent"), "{stderr}");
    let toml = std::fs::read_to_string(dir.join("papyrus.toml")).unwrap();
    assert!(
        toml.contains("ppj = \"mod.ppj\"") && toml.contains("game = \"skyrim\""),
        "{toml}"
    );

    // Still only what it lists, with papyrus.toml on top.
    std::fs::write(
        dir.join("papyrus.toml"),
        format!("{toml}\n[lints]\n\"locals/unused\" = \"allow\"\n").replace("[lints]\n#", "#"),
    )
    .unwrap();
    let found = papyrus(&dir, &["check"], "");
    let stderr = String::from_utf8_lossy(&found.stderr);
    assert!(
        stderr.contains("1 file(s) checked: 0 error(s), 0 warning(s)"),
        "{stderr}"
    );

    // Two of them: nothing chosen behind the user's back.
    std::fs::remove_file(dir.join("papyrus.toml")).unwrap();
    std::fs::copy(dir.join("mod.ppj"), dir.join("other.ppj")).unwrap();
    let refused = papyrus(&dir, &["init"], "");
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--ppj"));
    let refused = papyrus(&dir, &["check"], "");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("2 `.ppj` files"));
    let made = papyrus(&dir, &["init", "--ppj", "other.ppj"], "");
    assert!(made.status.success());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn stdin_takes_the_place_of_the_file() {
    let root = tree(
        "stdin",
        &[
            (
                "papyrus.toml",
                "game = \"skyrim\"\n[fmt]\nindent = \"tab\"\n",
            ),
            (
                "Base.psc",
                "ScriptName Base extends Quest\nFunction Base1()\nEndFunction\n",
            ),
        ],
    );
    // Sees the project: the parent is on disk, what is checked is not.
    let script = "ScriptName Child extends Base\nFunction F()\n\tBase2()\nEndFunction\n";
    let args = ["check", "--stdin", "--stdin-filename", "Child.psc"];
    let output = papyrus(&root, &args, script);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("PAP008"),
        "{}",
        text(&output.stderr)
    );
    assert!(
        papyrus(&root, &args, &script.replace("Base2", "Base1"))
            .status
            .success()
    );

    // Without a name, its `ScriptName` gives one.
    assert!(
        papyrus(
            &root,
            &["check", "--stdin"],
            "ScriptName Child extends Base\n"
        )
        .status
        .success()
    );

    let args = ["fmt", "--stdin", "--stdin-filename", "Child.psc"];
    let output = papyrus(
        &root,
        &args,
        "ScriptName Child\nFunction F()\nint a = 1\nEndFunction\n",
    );
    assert_eq!(
        text(&output.stdout),
        "scriptname Child\n\nfunction F()\n\tint a = 1\nendfunction\n"
    );
    // On a syntax error, nothing to replace the text with.
    let output = papyrus(&root, &args, "Function\n");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn init_writes_a_configuration_once() {
    let root = tree(
        "init",
        &[("MyQuest.psc", "ScriptName MyQuest\nGuard G\nEndGuard\n")],
    );
    let output = papyrus(&root, &["init"], "");
    assert!(output.status.success(), "{}", text(&output.stderr));
    let written = std::fs::read_to_string(root.join("papyrus.toml")).unwrap();
    assert!(written.contains("game = \"starfield\""), "{written}");

    let again = papyrus(&root, &["init"], "");
    assert_eq!(again.status.code(), Some(2));
    assert!(text(&again.stderr).contains("already exists"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn explain_finds_a_lint_by_code_or_name() {
    let root = tree("explain", &[]);
    std::fs::create_dir_all(&root).unwrap();
    for key in ["PAP015", "locals/unused"] {
        let output = papyrus(&root, &["explain", key], "");
        assert!(output.status.success());
        assert!(text(&output.stdout).starts_with("PAP015 locals/unused — warn by default"));
    }
    let output = papyrus(&root, &["explain", "locals/unuse"], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("did you mean `locals/unused`?"));
    assert!(
        papyrus(&root, &["explain", "baseline"], "")
            .status
            .success()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_short_format_puts_each_problem_on_a_line() {
    let root = tree(
        "short",
        &[
            ("papyrus.toml", "game = \"skyrim\"\n"),
            (
                "MyQuest.psc",
                "ScriptName MyQuest extends Quest\nFunction F()\n\tint unused = 1\nEndFunction\n",
            ),
        ],
    );
    let stderr = run(&root, &["--message-format", "short"]);
    let path = std::path::absolute(root.join("MyQuest.psc")).unwrap();
    let expected = format!(
        "{}:3:6:3:12: warning[PAP015 locals/unused]: the variable `unused` is never used. Fixable with `papyrus check --fix`.",
        path.display()
    );
    assert_eq!(stderr.lines().next(), Some(expected.as_str()), "{stderr}");
    let _ = std::fs::remove_dir_all(&root);
}
