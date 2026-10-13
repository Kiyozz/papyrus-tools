//! A project importing a whole game: the vanilla corpus as `imports`, and a
//! script of its own. Prints what each request costs, and keeps it within
//! budget.
#![cfg(feature = "external-corpus")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Project, Semantics, SourceFile};
use papyrus_project::{DiscoverOptions, decode, discover};
use papyrus_syntax::TextSize;

const OWN: &str = "\
ScriptName MyQuest extends Quest

Actor Property PlayerRef Auto

Function Helper(int aiCount)
\tDebug.Notification(\"count \" + aiCount)
EndFunction

Event OnInit()
\tRegisterForSingleUpdate(1.0)
\tHelper(1)
EndEvent

Event OnUpdate()
\tHelper(2)
\tPlayerRef.AddItem(None, 1)
EndEvent
";

fn at(name: &str) -> TextSize {
    TextSize::new(OWN.find(name).expect(name) as u32)
}

#[test]
fn a_project_importing_skyrim() {
    let Some(root) = std::env::var_os("PAPYRUS_CORPUS_SKYRIM").map(PathBuf::from) else {
        eprintln!("· Skyrim skipped");
        return;
    };
    let start = Instant::now();
    let db = Database::default();
    let imports: Vec<SourceFile> = discover(
        &[root],
        DiscoverOptions {
            include_fragments: true,
        },
    )
    .expect("walk")
    .into_iter()
    .map(|path| {
        let text = decode(&std::fs::read(&path).expect("read")).text;
        SourceFile::new(&db, path, text)
    })
    .collect();
    let own = SourceFile::new(&db, "MyQuest.psc".into(), OWN.to_owned());
    let project = Project::new(&db, vec![own], imports.clone(), Some(GameId::Skyrim));
    let sema = Semantics::new(&db, project);
    eprintln!("read {} imports: {:?}", imports.len(), start.elapsed());

    // Prints a cold run and the best of three warm ones. The latter must
    // stay within a second: ten times what the slowest search takes, so that
    // a busy machine passes, and a search reading every file again fails.
    let time = |label: &str, f: &dyn Fn() -> usize| {
        let start = Instant::now();
        let n = f();
        eprintln!("{label} (cold): {n} in {:?}", start.elapsed());
        let best = (0..3)
            .map(|_| {
                let start = Instant::now();
                f();
                start.elapsed()
            })
            .min()
            .unwrap();
        eprintln!("{label} (warm): {n} in {best:?}");
        assert!(best < Duration::from_secs(1), "{label}: {best:?}");
    };
    time("index", &|| sema.index().files().count());
    time("check", &|| sema.check(own).len());
    time("hover", &|| {
        usize::from(sema.hover(own, at("AddItem")).is_some())
    });
    time("completion", &|| {
        sema.completions(own, at("\tHelper(2)")).len()
    });
    for name in [
        "Helper",
        "RegisterForSingleUpdate",
        "OnInit",
        "AddItem",
        " Quest",
        "Debug",
    ] {
        // A leading space tells `extends Quest` from `MyQuest`.
        let skip = TextSize::new((name.len() - name.trim_start().len()) as u32);
        let def = sema.def_at(own, at(name) + skip).expect(name);
        time(&format!("references of {name}"), &|| {
            sema.references_of(&def).len()
        });
    }
}
