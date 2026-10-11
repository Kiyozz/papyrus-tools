//! Les lints sur le corpus vanilla.
//!
//! Le vanilla compile : tout `PAP002` y serait un faux positif. Les `.flg`
//! réels doivent coïncider avec les tables embarquées. Et `--fix`, appliqué
//! en mémoire à chaque script, doit rester sûr et aller au bout.
#![cfg(feature = "external-corpus")]

use std::collections::BTreeMap;
use std::path::PathBuf;

use papyrus_dialect::{Dialect, GameId};
use papyrus_lint::{Context, Levels, apply_fixes, run, vanilla_flags};
use papyrus_project::flags::parse_flags;
use papyrus_project::{DiscoverOptions, decode, discover};

fn corpus(game: GameId) -> Option<PathBuf> {
    let var = match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    };
    std::env::var_os(var).map(PathBuf::from)
}

fn flg_path(game: GameId, root: &std::path::Path) -> PathBuf {
    match game {
        GameId::Skyrim => root.join("TESV_Papyrus_Flags.flg"),
        GameId::Fallout4 | GameId::Fallout76 => root.join("Base/Institute_Papyrus_Flags.flg"),
        GameId::Starfield => root.join("Starfield_Papyrus_Flags.flg"),
    }
}

fn run_game(game: GameId) {
    let Some(root) = corpus(game) else {
        eprintln!("· {} ignoré", game.display_name());
        return;
    };

    // La table embarquée reproduit le `.flg` du jeu.
    let real = parse_flags(&std::fs::read_to_string(flg_path(game, &root)).expect(".flg"))
        .expect(".flg lisible");
    assert_eq!(real, vanilla_flags(Some(game)), "table de flags {game}");

    let scripts = discover(
        &[root],
        DiscoverOptions {
            include_fragments: true,
        },
    )
    .expect("parcours");
    let dialect = Dialect::new(game);
    let levels = Levels::default();
    let flags = vanilla_flags(Some(game));

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut false_positives = Vec::new();
    let mut fixed_files = 0;
    for path in &scripts {
        let decoded = decode(&std::fs::read(path).expect("lecture"));
        let parse = papyrus_syntax::parse(&decoded.text, dialect);
        let root = parse.syntax();
        let context = Context {
            decoded: &decoded,
            root: &root,
            game: Some(game),
            flags: &flags,
        };
        let diagnostics = run(&context, &levels);
        for d in &diagnostics {
            *counts.entry(d.lint.code).or_default() += 1;
            if d.lint.code == "PAP002" && false_positives.len() < 20 {
                false_positives.push(format!("{} : {}", path.display(), d.message));
            }
        }

        if diagnostics.iter().any(|d| d.fix.is_some()) {
            let fixed = apply_fixes(&decoded.text, &diagnostics)
                .unwrap_or_else(|| panic!("correctif refusé sur {}", path.display()));
            let reparse = papyrus_syntax::parse(&fixed, dialect);
            assert!(
                reparse.ok(),
                "{} : le correctif casse la syntaxe",
                path.display()
            );

            // Un second passage ne trouve plus rien à corriger.
            let decoded = decode(fixed.as_bytes());
            let root = reparse.syntax();
            let again = run(
                &Context {
                    decoded: &decoded,
                    root: &root,
                    game: Some(game),
                    flags: &flags,
                },
                &levels,
            );
            assert!(
                again.iter().all(|d| d.fix.is_none()),
                "{} : correctif incomplet",
                path.display()
            );
            fixed_files += 1;
        }
    }

    println!(
        "· {:<12} {:>6} fichiers  {:>6} corrigeables  {counts:?}",
        game.display_name(),
        scripts.len(),
        fixed_files
    );
    assert!(
        false_positives.is_empty(),
        "PAP002 sur du code qui compile :\n{}",
        false_positives.join("\n")
    );
}

#[test]
fn skyrim() {
    run_game(GameId::Skyrim);
}

#[test]
fn fallout4() {
    run_game(GameId::Fallout4);
}

#[test]
fn starfield() {
    run_game(GameId::Starfield);
}
