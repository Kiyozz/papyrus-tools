//! The formatter on the vanilla corpus — exit criterion of v0.2.
//!
//! On every script: formatting succeeds, its safeguard never fires, and
//! reformatting the result changes nothing.
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
        eprintln!("· {} skipped", game.display_name());
        return;
    };
    // Fragments included: they are valid scripts too.
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
        let twice = format(&once, dialect, &options).expect("the result formats again");
        if twice != once {
            failures.push(format!("{} : non idempotent", path.display()));
        }
        changed += usize::from(once != decoded.text);
    }

    println!(
        "· {:<12} {:>6} files  {:>6} reformatted",
        game.display_name(),
        scripts.len(),
        changed
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
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
