//! Lexage et parsing du corpus vanilla — le critère de sortie de la v0.1.
//!
//! Ces tests ne tournent qu'avec la feature `external-corpus`, et lisent les
//! chemins dans l'environnement. Aucune installation de jeu n'est requise pour
//! construire ou tester le projet, et aucun chemin propre à une machine
//! n'entre dans le dépôt.
//!
//! ```sh
//! PAPYRUS_CORPUS_SKYRIM=".../Data/Source/Scripts" \
//! PAPYRUS_CORPUS_FALLOUT4=".../Data/Scripts/Source" \
//! PAPYRUS_CORPUS_STARFIELD=".../Data/Scripts/Source" \
//!   cargo test --release --features external-corpus -- --nocapture
//! ```
#![cfg(feature = "external-corpus")]

use std::fs;
use std::path::{Path, PathBuf};

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::{SyntaxKind, lex_with_dialect, parse_tokens};

/// Variable d'environnement portant les sources d'un jeu.
fn corpus_var(game: GameId) -> &'static str {
    match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    }
}

/// Tous les `.psc` sous `root`, sous-dossiers compris.
///
/// Fallout 4 et Starfield rangent leurs sources en sous-dossiers (`Base`,
/// `User`, `DLC*`, `Fragments`…), le parcours doit donc être récursif.
fn collect_scripts(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scripts(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("psc"))
        {
            out.push(path);
        }
    }
}

struct Stats {
    files: usize,
    tokens: usize,
    bytes: usize,
    /// Tokens `ERROR` du lexer.
    lex_errors: Vec<String>,
    /// Erreurs du parser, toutes comptées, les premières détaillées.
    parse_errors: Vec<String>,
    parse_error_count: usize,
}

/// `fichier:ligne:colonne`, colonnes en octets à partir de 1.
fn locate(path: &Path, source: &str, offset: usize) -> String {
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = offset - before.rfind('\n').map_or(0, |i| i + 1) + 1;
    format!("{}:{line}:{column}", path.display())
}

/// La ligne qui contient `offset`, sans sa fin de ligne.
fn line_at(source: &str, offset: usize) -> &str {
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = source[offset..]
        .find(['\r', '\n'])
        .map_or(source.len(), |i| offset + i);
    source[start..end].trim()
}

fn lex_corpus(game: GameId) -> Option<Stats> {
    let root = std::env::var_os(corpus_var(game))?;
    let root = PathBuf::from(root);
    assert!(
        root.is_dir(),
        "{} pointe sur {}, qui n'est pas un dossier",
        corpus_var(game),
        root.display()
    );

    let mut scripts = Vec::new();
    collect_scripts(&root, &mut scripts);
    assert!(!scripts.is_empty(), "aucun .psc sous {}", root.display());

    let dialect = Dialect::new(game);
    let mut stats = Stats {
        files: scripts.len(),
        tokens: 0,
        bytes: 0,
        lex_errors: Vec::new(),
        parse_errors: Vec::new(),
        parse_error_count: 0,
    };

    for path in &scripts {
        let raw = fs::read(path).expect("lecture");
        // Le décodage réel — Windows-1252 pour Skyrim, UTF-8 pour les autres —
        // viendra avec `papyrus-project`. Ici, n'importe quel `&str` valide
        // suffit à éprouver le lexer.
        let source = String::from_utf8_lossy(&raw);

        let tokens = lex_with_dialect(&source, dialect);

        // L'invariant : les tokens pavent la source sans trou.
        let mut offset = 0usize;
        for token in &tokens {
            assert_eq!(
                usize::from(token.range.start()),
                offset,
                "trou dans {} avant {:?}",
                path.display(),
                token.kind
            );
            offset = usize::from(token.range.end());
        }
        assert_eq!(
            offset,
            source.len(),
            "fin non couverte dans {}",
            path.display()
        );

        let rebuilt: String = tokens.iter().map(|t| t.text(&source)).collect();
        assert_eq!(
            rebuilt,
            source,
            "reconstruction différente pour {}",
            path.display()
        );

        for token in tokens.iter().filter(|t| t.kind == SyntaxKind::ERROR) {
            if stats.lex_errors.len() < 20 {
                stats.lex_errors.push(format!(
                    "{}: {:?} à {:?}",
                    path.display(),
                    token.text(&source),
                    token.range
                ));
            }
        }

        // Le parsing : l'arbre doit reproduire la source, et la lire sans
        // erreur puisque ces scripts compilent.
        let parse = parse_tokens(&source, &tokens, dialect);
        assert_eq!(
            parse.syntax().to_string(),
            source,
            "l'arbre de {} ne reproduit pas la source",
            path.display()
        );
        // Une erreur sans message serait un bug : tout ERROR_TREE doit
        // venir avec la sienne.
        let silent = parse
            .syntax()
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::ERROR_TREE)
            .count();
        assert!(
            silent == 0 || !parse.errors().is_empty(),
            "{} : {silent} ERROR_TREE sans erreur signalée",
            path.display()
        );
        for error in parse.errors() {
            stats.parse_error_count += 1;
            if stats.parse_errors.len() < 40 {
                let offset = usize::from(error.range().start());
                stats.parse_errors.push(format!(
                    "{}: {}\n      | {}",
                    locate(path, &source, offset),
                    error.message(),
                    line_at(&source, offset)
                ));
            }
        }

        stats.tokens += tokens.len();
        stats.bytes += source.len();
    }

    Some(stats)
}

fn report(game: GameId) {
    let Some(stats) = lex_corpus(game) else {
        eprintln!(
            "· {} ignoré ({} non défini)",
            game.display_name(),
            corpus_var(game)
        );
        return;
    };

    println!(
        "· {:<12} {:>6} fichiers  {:>9} tokens  {:>9} octets",
        game.display_name(),
        stats.files,
        stats.tokens,
        stats.bytes
    );

    assert!(
        stats.lex_errors.is_empty(),
        "{} tokens ERROR sur le corpus {} — le lexer a un trou :\n{}",
        stats.lex_errors.len(),
        game.display_name(),
        stats.lex_errors.join("\n")
    );
    assert!(
        stats.parse_errors.is_empty(),
        "{} erreurs de parsing sur le corpus {} :\n{}",
        stats.parse_error_count,
        game.display_name(),
        stats.parse_errors.join("\n")
    );
}

#[test]
fn skyrim() {
    report(GameId::Skyrim);
}

#[test]
fn fallout4() {
    report(GameId::Fallout4);
}

#[test]
fn starfield() {
    report(GameId::Starfield);
}
