//! Against `PapyrusAssembler`, on the scripts of `tests/fixtures`, compiled by
//! each game's compiler and disassembled by its assembler. `PAPYRUS_PEX_CORPUS`
//! adds folders of `.pex` files, separated by `;`: each must read, and where
//! `PapyrusAssembler -D` left a `.disassemble.pas` beside it, the text must be
//! the same, but for the dates and the order of the tables: `PapyrusAssembler`
//! writes them in the order of its own hash tables, this crate in the order of
//! the file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use papyrus_pex::{Pex, disassemble};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Labels numbered from 0 in each function, as the order of the functions
/// decides their numbers.
fn relabel(block: &[&str]) -> String {
    let mut labels = HashMap::new();
    let mut out = String::new();
    for line in block {
        let mut rest = *line;
        let start = out.len();
        while let Some(at) = rest.find("_label") {
            let (before, label) = rest.split_at(at);
            let digits = label[6..].find(|c: char| !c.is_ascii_digit());
            let end = digits.map_or(label.len(), |digits| digits + 6);
            let next = labels.len();
            let number = *labels.entry(&label[..end]).or_insert(next);
            out.push_str(before);
            out.push_str(&format!("_L{number}"));
            rest = &label[end..];
        }
        out.push_str(rest);
        // The width of a number moved the comment.
        if out[start..].contains("_L") {
            let line = out.split_off(start);
            out.push_str(&line.split_whitespace().collect::<Vec<_>>().join(" "));
        }
        out.push('\n');
    }
    out
}

/// Functions and variables as blocks, the other lines alone, sorted; without
/// the two lines holding a date in the machine's time zone.
fn comparable(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(".modifyTime") || trimmed.starts_with(".compileTime") {
            continue;
        }
        let end = match trimmed.split(' ').next() {
            Some(".function") => ".endFunction",
            Some(".variable") => ".endVariable",
            _ => {
                blocks.push(line.to_owned());
                continue;
            }
        };
        let mut block = vec![line];
        for line in lines.by_ref() {
            block.push(line);
            if line.trim_start() == end {
                break;
            }
        }
        blocks.push(relabel(&block));
    }
    blocks.sort();
    blocks
}

/// What differs from the official text beside `path`, if there is one.
fn compare(path: &Path) -> Option<Result<(), String>> {
    let name = path.display();
    let bytes = std::fs::read(path).unwrap();
    let pex = Pex::read(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    let official = std::fs::read(path.with_extension("disassemble.pas")).ok()?;
    let (official, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(&official);
    let (ours, official) = (comparable(&disassemble(&pex)), comparable(&official));
    let line = ours.iter().zip(&official).position(|(a, b)| a != b);
    Some(if let Some(line) = line {
        let (ours, official) = (ours[line].lines(), official[line].lines());
        let pair = ours.zip(official).find(|(a, b)| a != b);
        Err(format!("{name}: {pair:?}"))
    } else if ours.len() != official.len() {
        let (ours, official) = (ours.len(), official.len());
        Err(format!("{name}: {ours} lines, not {official}"))
    } else {
        Ok(())
    })
}

fn scripts(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scripts(&path, out);
        } else if path.extension().is_some_and(|e| e == "pex") {
            out.push(path);
        }
    }
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    scripts(Path::new(FIXTURES), &mut paths);
    assert_eq!(paths.len(), 6);
    paths
}

#[test]
fn every_game_has_the_official_text() {
    for path in fixtures() {
        compare(&path).expect("its .pas").unwrap();
    }
}

#[test]
fn the_fixtures_hold_the_opcodes_of_their_game() {
    let text = |game: &str| {
        let path = Path::new(FIXTURES).join(game).join("Sample.pex");
        disassemble(&Pex::read(&std::fs::read(path).unwrap()).unwrap())
    };
    let starfield = text("starfield");
    for op in [
        "LockGuards",
        "UnlockGuards",
        "TryLockGuards",
        "ArrayGetAllMatchingStructs",
    ] {
        assert!(starfield.contains(op), "{op}");
    }
    assert!(starfield.contains(".guard FrontDoor"));
    let fallout = text("fallout4");
    for op in [
        "StructCreate",
        "StructSet",
        "ArrayFindStruct",
        "ArrayRemoveLastElement",
        "Is ",
    ] {
        assert!(fallout.contains(op), "{op}");
    }
    assert!(text("skyrim").contains("ArrayRFindElement"));
}

#[test]
fn a_stripped_script_says_so() {
    for game in ["skyrim", "fallout4", "starfield"] {
        let path = Path::new(FIXTURES).join(format!("{game}-stripped/Sample.pex"));
        let pex = Pex::read(&std::fs::read(path).unwrap()).unwrap();
        assert!(pex.debug.is_none());
        let text = disassemble(&pex);
        assert!(text.contains(".modifyTime 0 ;Debug info: No"), "{game}");
        assert!(text.contains(";@line ??"), "{game}");
    }
}

#[test]
fn a_damaged_file_is_an_error() {
    for path in fixtures() {
        let bytes = std::fs::read(&path).unwrap();
        for length in 0..bytes.len() {
            assert!(Pex::read(&bytes[..length]).is_err(), "cut at {length}");
        }
        // Whatever it reads as, nothing panics, down to the text.
        for at in 0..bytes.len() {
            for value in [0, 0x7F, 0xFF] {
                let mut bytes = bytes.clone();
                bytes[at] = value;
                if let Ok(pex) = Pex::read(&bytes) {
                    disassemble(&pex);
                }
            }
        }
    }
}

#[test]
fn not_a_pex_says_so() {
    let error = Pex::read(b"ScriptName Sample").unwrap_err();
    assert_eq!(error.to_string(), "not a .pex file at offset 0x0");
}

#[test]
fn the_corpus_has_the_official_text() {
    let Ok(corpus) = std::env::var("PAPYRUS_PEX_CORPUS") else {
        eprintln!("· PAPYRUS_PEX_CORPUS not set, skipped");
        return;
    };
    for folder in corpus.split(';') {
        let mut paths = Vec::new();
        scripts(Path::new(folder), &mut paths);
        let results: Vec<_> = paths.iter().filter_map(|path| compare(path)).collect();
        let different: Vec<_> = results.iter().filter_map(|r| r.as_ref().err()).collect();
        eprintln!(
            "{folder}: {} read, {} compared, {} different",
            paths.len(),
            results.len(),
            different.len()
        );
        for difference in different.iter().take(12) {
            eprintln!("{difference}");
        }
        assert!(!paths.is_empty() && different.is_empty(), "{folder}");
    }
}
