//! The semantic model on the vanilla corpus.
//!
//! Every `extends` chain must reach its root: a single unresolved parent
//! would leave a whole hierarchy without members. Names must resolve at
//! 99.99 % at least; the most frequent misses are reported, and
//! `PAPYRUS_SHOW_MISSES=name,…` locates them. Completion must
//! answer within 200 ms on a 500-script project, even right after an edit.
#![cfg(feature = "external-corpus")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Project, ScriptRef, Semantics, SourceFile};
use papyrus_project::{DiscoverOptions, decode, discover};
use papyrus_syntax::{SyntaxKind, TextSize};
use salsa::Setter;

fn corpus(game: GameId) -> Option<PathBuf> {
    let var = match game {
        GameId::Skyrim => "PAPYRUS_CORPUS_SKYRIM",
        GameId::Fallout4 | GameId::Fallout76 => "PAPYRUS_CORPUS_FALLOUT4",
        GameId::Starfield => "PAPYRUS_CORPUS_STARFIELD",
    };
    std::env::var_os(var).map(PathBuf::from)
}

fn load(db: &Database, game: GameId, limit: usize) -> Option<Vec<SourceFile>> {
    let Some(root) = corpus(game) else {
        eprintln!("· {} skipped", game.display_name());
        return None;
    };
    let paths = discover(
        &[root],
        DiscoverOptions {
            include_fragments: true,
        },
    )
    .expect("walk");
    Some(
        paths
            .into_iter()
            .take(limit)
            .map(|path| {
                let text = decode(&std::fs::read(&path).expect("read")).text;
                SourceFile::new(db, path, text)
            })
            .collect(),
    )
}

fn resolve_game(game: GameId) {
    let db = Database::default();
    let Some(files) = load(&db, game, usize::MAX) else {
        return;
    };
    let project = Project::new(&db, files.clone(), Vec::new(), Some(game));
    let sema = Semantics::new(&db, project);

    // Every chain reaches its root.
    let mut broken = Vec::new();
    for &file in &files {
        let items = sema.items(file);
        if items.name.is_empty() {
            continue;
        }
        let last = sema.chain(ScriptRef::Source(file)).last().unwrap();
        let root_parent = match last {
            ScriptRef::Source(f) => sema.items(f).parent.clone(),
            ScriptRef::Native(s) => s.parent.map(str::to_owned),
        };
        if let Some(parent) = root_parent {
            broken.push(format!("{} → {parent}", items.name));
        }
    }

    // The share of names resolved.
    let mut total = 0usize;
    let mut resolved = 0usize;
    let mut misses: HashMap<String, usize> = HashMap::new();
    let show: Vec<String> = std::env::var("PAPYRUS_SHOW_MISSES")
        .map(|v| v.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    for &file in &files {
        for node in sema.root(file).descendants() {
            if node.kind() != SyntaxKind::NAME_REF
                || node
                    .first_token()
                    .is_some_and(|t| t.kind() == SyntaxKind::LENGTH_KW)
            {
                continue;
            }
            // Named arguments are left to the callee's parameters.
            let named = node.parent().is_some_and(|p| p.kind() == SyntaxKind::ARG)
                && node
                    .siblings_with_tokens(rowan::Direction::Next)
                    .any(|e| e.kind() == SyntaxKind::EQ);
            if named {
                continue;
            }
            total += 1;
            if sema.resolve_name_ref(file, &node).is_some() {
                resolved += 1;
            } else {
                let name = node.text().to_string().to_ascii_lowercase();
                // `PAPYRUS_SHOW_MISSES=a,b` locates the misses of those names.
                if show.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                    let text = file.text(&db);
                    let line = text[..usize::from(node.text_range().start())]
                        .lines()
                        .count();
                    println!("    miss {name} at {}:{line}", file.path(&db).display());
                }
                *misses.entry(name).or_default() += 1;
            }
        }
    }
    let mut misses: Vec<(String, usize)> = misses.into_iter().collect();
    misses.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!(
        "· {:<12} {:>6} scripts  {:>8} names  {:>6.2} % resolved  {} broken chains",
        game.display_name(),
        files.len(),
        total,
        100.0 * resolved as f64 / total.max(1) as f64,
        broken.len()
    );
    for (name, count) in misses.iter().take(15) {
        println!("    {count:>6} {name}");
    }
    assert!(
        broken.is_empty(),
        "unresolved parents:\n{}",
        broken.join("\n")
    );
    // What remains are vanilla's own mistakes: a script not shipped, an
    // array function called on a single value.
    assert!(resolved * 10_000 >= total * 9_999, "resolution regressed");
}

#[test]
fn skyrim_resolves() {
    resolve_game(GameId::Skyrim);
}

#[test]
fn fallout4_resolves() {
    resolve_game(GameId::Fallout4);
}

#[test]
fn starfield_resolves() {
    resolve_game(GameId::Starfield);
}

/// Completion on a 500-script project, cold, warm, and after an edit.
#[test]
fn completion_is_fast_enough() {
    let mut db = Database::default();
    let Some(files) = load(&db, GameId::Skyrim, 500) else {
        return;
    };
    let project = Project::new(&db, files.clone(), Vec::new(), Some(GameId::Skyrim));
    // The longest script, at the end of its first function body.
    let file = *files.iter().max_by_key(|f| f.text(&db).len()).unwrap();
    let text = file.text(&db).to_owned();
    let lower = text.to_ascii_lowercase();
    let Some(end) = lower.find("endfunction").or_else(|| lower.find("endevent")) else {
        return;
    };
    let offset = TextSize::new(end as u32);

    let time = |db: &Database| {
        let start = Instant::now();
        let found = Semantics::new(db, project).completions(file, offset);
        (start.elapsed(), found.len())
    };
    let (cold, count) = time(&db);
    let (warm, _) = time(&db);
    // Typing in the body: one file changes, the index survives.
    let edited = format!("{}\n{}", &text[..end], &text[end..]);
    file.set_text(&mut db).to(edited);
    let (after_edit, _) = time(&db);
    println!(
        "· completion: {count} items, cold {cold:?}, warm {warm:?}, after an edit {after_edit:?}"
    );
    let budget = Duration::from_millis(200);
    assert!(warm < budget && after_edit < budget);
}
