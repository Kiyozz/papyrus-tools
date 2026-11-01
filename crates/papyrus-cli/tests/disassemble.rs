//! `papyrus disassemble` on the fixtures of `papyrus-pex`.

use std::path::Path;
use std::process::{Command, Output};

fn disassemble(path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_papyrus"))
        .arg("disassemble")
        .arg(path)
        .output()
        .expect("papyrus runs")
}

#[test]
fn prints_the_assembly_of_a_script() {
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/../papyrus-pex/tests/fixtures");
    let output = disassemble(&Path::new(fixtures).join("fallout4/Sample.pex"));
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with(".info\n"), "{text}");
    assert!(text.contains(".object Sample Quest"), "{text}");
    assert!(text.ends_with(".endObjectTable\n"), "{text}");
}

#[test]
fn a_file_that_is_not_a_script_is_an_error() {
    let output = disassemble(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/Cargo.toml"
    )));
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a .pex file at offset 0x0"), "{stderr}");
}
