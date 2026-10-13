//! Lexing and parsing the vanilla corpus — the exit criterion of v0.1.
//!
//! These tests only run with the `external-corpus` feature, and read their
//! paths from the environment. No game installation is required to build or
//! test the project, and no machine-specific path enters the repository.
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

/// Environment variable holding a game's sources.
fn corpus_var(game: GameId) -> &'static str {
    match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    }
}

/// Every `.psc` under `root`, subfolders included.
///
/// Fallout 4 and Starfield store their sources in subfolders (`Base`,
/// `User`, `DLC*`, `Fragments`…), so the walk must be recursive.
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
    /// `ERROR` tokens from the lexer.
    lex_errors: Vec<String>,
    /// Parser errors, all counted, the first ones detailed.
    parse_errors: Vec<String>,
    parse_error_count: usize,
}

/// `file:line:column`, columns in bytes starting at 1.
fn locate(path: &Path, source: &str, offset: usize) -> String {
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = offset - before.rfind('\n').map_or(0, |i| i + 1) + 1;
    format!("{}:{line}:{column}", path.display())
}

/// The line containing `offset`, without its line end.
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
        "{} points to {}, which is not a folder",
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
        let raw = fs::read(path).expect("read");
        // Real decoding — Windows-1252 for Skyrim, UTF-8 for the others —
        // will come with `papyrus-project`. Here, any valid `&str` is enough
        // to exercise the lexer.
        let source = String::from_utf8_lossy(&raw);

        let tokens = lex_with_dialect(&source, dialect);

        // The invariant: the tokens tile the source without gaps.
        let mut offset = 0usize;
        for token in &tokens {
            assert_eq!(
                usize::from(token.range.start()),
                offset,
                "gap in {} before {:?}",
                path.display(),
                token.kind
            );
            offset = usize::from(token.range.end());
        }
        assert_eq!(
            offset,
            source.len(),
            "end not covered in {}",
            path.display()
        );

        let rebuilt: String = tokens.iter().map(|t| t.text(&source)).collect();
        assert_eq!(
            rebuilt,
            source,
            "rebuilt text differs for {}",
            path.display()
        );

        for token in tokens.iter().filter(|t| t.kind == SyntaxKind::ERROR) {
            if stats.lex_errors.len() < 20 {
                stats.lex_errors.push(format!(
                    "{}: {:?} at {:?}",
                    path.display(),
                    token.text(&source),
                    token.range
                ));
            }
        }

        // Parsing: the tree must reproduce the source, and read it without
        // errors since these scripts compile.
        let parse = parse_tokens(&source, &tokens, dialect);
        assert_eq!(
            parse.syntax().to_string(),
            source,
            "the tree of {} does not reproduce the source",
            path.display()
        );
        // An error without a message would be a bug: every ERROR_TREE must
        // come with its own.
        let silent = parse
            .syntax()
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::ERROR_TREE)
            .count();
        assert!(
            silent == 0 || !parse.errors().is_empty(),
            "{}: {silent} ERROR_TREE without a reported error",
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
            "· {} skipped ({} not set)",
            game.display_name(),
            corpus_var(game)
        );
        return;
    };

    println!(
        "· {:<12} {:>6} files  {:>9} tokens  {:>9} bytes",
        game.display_name(),
        stats.files,
        stats.tokens,
        stats.bytes
    );

    assert!(
        stats.lex_errors.is_empty(),
        "{} ERROR tokens on the {} corpus — the lexer has a gap:\n{}",
        stats.lex_errors.len(),
        game.display_name(),
        stats.lex_errors.join("\n")
    );
    assert!(
        stats.parse_errors.is_empty(),
        "{} parse errors on the {} corpus:\n{}",
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
