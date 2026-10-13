//! Game detection, measured on the non-native vanilla scripts — those that
//! look most like mod code.
//!
//! Only the border with Skyrim matters: taking a Fallout 4 script for a
//! Starfield one is harmless.
#![cfg(feature = "external-corpus")]

use std::path::PathBuf;

use papyrus_dialect::{Dialect, GameId};
use papyrus_project::detect::{evidence, explain, majority};
use papyrus_project::{DiscoverOptions, decode, discover};

fn corpus(game: GameId) -> Option<PathBuf> {
    let var = match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    };
    std::env::var_os(var).map(PathBuf::from)
}

fn run(game: GameId) {
    let Some(root) = corpus(game) else {
        eprintln!("· {} skipped", game.display_name());
        return;
    };
    let scripts = discover(&[root], DiscoverOptions::default()).expect("parcours");
    let skyrim = game == GameId::Skyrim;

    let (mut right, mut undecided, mut wrong) = (0, 0, Vec::new());
    let mut verdicts = Vec::new();
    for path in &scripts {
        let decoded = decode(&std::fs::read(path).expect("lecture"));
        let parse = papyrus_syntax::parse(&decoded.text, Dialect::new(GameId::Starfield));
        let root = parse.syntax();
        // The native scripts are the source of the tables: detecting them
        // would be cheating.
        let name = path.file_stem().unwrap().to_string_lossy();
        if papyrus_stdlib::script(game, &name).is_some() {
            continue;
        }
        let found = evidence(&root);
        let verdict = found.verdict();
        verdicts.push(verdict);
        match verdict {
            None => undecided += 1,
            Some(g) if (g == GameId::Skyrim) == skyrim => right += 1,
            Some(g) => {
                let names: Vec<String> = explain(&root)
                    .into_iter()
                    .map(|v| format!("{}{:?}", v.name, v.known.map(u8::from)))
                    .collect();
                wrong.push(format!(
                    "{} → {g} (min {}) {}",
                    path.file_name().unwrap().to_string_lossy(),
                    found.minimum,
                    names.join(" ")
                ));
            }
        }
    }
    let total = right + undecided + wrong.len();
    let project = majority(verdicts.iter().copied()).map(|(g, _, _)| g);
    println!(
        "· {:<12} {total:>6} scripts  {right:>6} right  {undecided:>6} undecided  {:>4} wrong  → project: {project:?}",
        game.display_name(),
        wrong.len()
    );
    for line in wrong.iter().take(40) {
        println!("    {line}");
    }
    assert_eq!(
        project.map(|g| g == GameId::Skyrim),
        Some(skyrim),
        "majority"
    );
    assert!(
        wrong.is_empty(),
        "{} verdicts cross the Skyrim border",
        wrong.len()
    );
}

#[test]
fn skyrim() {
    run(GameId::Skyrim);
}

#[test]
fn fallout4() {
    run(GameId::Fallout4);
}

#[test]
fn starfield() {
    run(GameId::Starfield);
}
