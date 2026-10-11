//! Le formateur sur le corpus vanilla — critère de sortie de la v0.2.
//!
//! Sur chaque script : le formatage aboutit, son garde-fou ne se déclenche
//! jamais, et reformater le résultat ne change plus rien.
//!
//! ```sh
//! PAPYRUS_CORPUS_SKYRIM=… PAPYRUS_CORPUS_FALLOUT4=… PAPYRUS_CORPUS_STARFIELD=… \
//!   cargo test -p papyrus-fmt --release --features external-corpus -- --nocapture
//! ```
#![cfg(feature = "external-corpus")]

use std::path::PathBuf;

use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatOptions, format};
use papyrus_project::{DiscoverOptions, decode, discover};

fn corpus_var(game: GameId) -> &'static str {
    match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    }
}

fn run(game: GameId) {
    let Some(root) = std::env::var_os(corpus_var(game)) else {
        eprintln!("· {} ignoré", game.display_name());
        return;
    };
    // Fragments compris : ce sont aussi des scripts valides.
    let scripts = discover(
        &[PathBuf::from(root)],
        DiscoverOptions {
            include_fragments: true,
        },
    )
    .expect("parcours du corpus");

    let dialect = Dialect::new(game);
    let options = FormatOptions::default();
    let mut failures = Vec::new();
    let mut changed = 0;
    for path in &scripts {
        let decoded = decode(&std::fs::read(path).expect("lecture"));
        let once = match format(&decoded.text, dialect, &options) {
            Ok(out) => out,
            Err(e) => {
                failures.push(format!("{} : {e}", path.display()));
                continue;
            }
        };
        let twice = format(&once, dialect, &options).expect("le résultat se reformate");
        if twice != once {
            failures.push(format!("{} : non idempotent", path.display()));
        }
        changed += usize::from(once != decoded.text);
    }

    println!(
        "· {:<12} {:>6} fichiers  {:>6} reformatés",
        game.display_name(),
        scripts.len(),
        changed
    );
    assert!(
        failures.is_empty(),
        "{} échecs :\n{}",
        failures.len(),
        failures
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
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
