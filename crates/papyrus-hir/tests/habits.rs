//! What compiles but should not stay.

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Problem, Project, Semantics, SourceFile};

/// `(problem, targeted text, fixes)` of the habits of the first file.
fn habits(game: GameId, files: &[(&str, &str)]) -> Vec<(Problem, String, Vec<String>)> {
    let db = Database::default();
    let sources: Vec<SourceFile> = files
        .iter()
        .map(|(name, text)| SourceFile::new(&db, (*name).into(), (*text).to_owned()))
        .collect();
    let first = sources[0];
    let project = Project::new(&db, sources, Vec::new(), Some(game));
    let sema = Semantics::new(&db, project);
    let text = first.text(&db);
    sema.check(first)
        .into_iter()
        .filter(|d| {
            matches!(
                d.problem,
                Problem::RepeatingUpdate
                    | Problem::GetPlayerInLoop
                    | Problem::BoolComparison
                    | Problem::SelfAssign
                    | Problem::SelfCompare
                    | Problem::UnusedVariable
                    | Problem::UnusedImport
            )
        })
        .map(|d| {
            let fixes = d
                .fixes
                .iter()
                .flat_map(|f| f.edits.iter().map(|e| e.text.clone()))
                .collect();
            (d.problem, text[d.range].to_owned(), fixes)
        })
        .collect()
}

fn one(game: GameId, text: &str) -> Vec<(Problem, String, Vec<String>)> {
    habits(game, &[("A.psc", text)])
}

#[test]
fn repeating_updates_and_the_player_in_loops_are_reported() {
    let source = "ScriptName A extends Quest\n\
                  Event OnInit()\n\
                  \tRegisterForUpdate(5.0)\n\
                  \tRegisterForSingleUpdate(5.0)\n\
                  \tRegisterForUpdateGameTime(1.0)\n\
                  \tActor before = Game.GetPlayer()\n\
                  \tWhile before.IsInCombat()\n\
                  \t\tIf Game.GetPlayer().IsDead()\n\t\t\tReturn\n\t\tEndIf\n\
                  \tEndWhile\n\
                  EndEvent\n";
    let found = one(GameId::Skyrim, source);
    assert_eq!(
        found,
        [
            (
                Problem::RepeatingUpdate,
                "RegisterForUpdate".to_owned(),
                vec!["RegisterForSingleUpdate".to_owned()]
            ),
            (
                Problem::RepeatingUpdate,
                "RegisterForUpdateGameTime".to_owned(),
                vec!["RegisterForSingleUpdateGameTime".to_owned()]
            ),
            (
                Problem::GetPlayerInLoop,
                "Game.GetPlayer()".to_owned(),
                vec![]
            ),
        ]
    );
}

#[test]
fn booleans_are_not_compared_with_true() {
    let source = "ScriptName A extends ObjectReference\n\
                  Bool flag\n\
                  Function F(Actor akActor)\n\
                  \tflag = True\n\
                  \tIf flag == True\n\tEndIf\n\
                  \tIf IsDisabled() == False\n\tEndIf\n\
                  \tIf True != (flag && flag)\n\tEndIf\n\
                  \tIf akActor == True\n\tEndIf\n\
                  EndFunction\n";
    let fixes: Vec<(String, Vec<String>)> = one(GameId::Skyrim, source)
        .into_iter()
        .filter(|(p, ..)| *p == Problem::BoolComparison)
        .map(|(_, t, f)| (t, f))
        .collect();
    // An object compared with `True` is a test of `None`: left alone.
    assert_eq!(
        fixes,
        [
            ("flag == True".to_owned(), vec!["flag".to_owned()]),
            (
                "IsDisabled() == False".to_owned(),
                vec!["!IsDisabled()".to_owned()]
            ),
            (
                "True != (flag && flag)".to_owned(),
                vec!["!(flag && flag)".to_owned()]
            ),
        ]
    );
}

#[test]
fn values_assigned_or_compared_to_themselves_are_reported() {
    let source = "ScriptName A\n\
                  Int count\n\
                  Function F(Int[] a)\n\
                  \tcount = count\n\
                  \ta[0] = a[0]\n\
                  \tcount = count + 1\n\
                  \tBool b = count == count\n\
                  \tb = count < count\n\
                  \tb = Next() == Next()\n\
                  EndFunction\n\
                  Int Function Next()\n\tReturn count\nEndFunction\n";
    let found: Vec<(Problem, String)> = one(GameId::Skyrim, source)
        .into_iter()
        .map(|(p, t, _)| (p, t))
        .collect();
    // Calls may differ from one to the next: left alone.
    assert_eq!(
        found,
        [
            (Problem::SelfAssign, "count = count".to_owned()),
            (Problem::SelfAssign, "a[0] = a[0]".to_owned()),
            (Problem::SelfCompare, "count == count".to_owned()),
            (Problem::SelfCompare, "count < count".to_owned()),
        ]
    );
}

#[test]
fn script_variables_never_read_are_reported() {
    let source = "ScriptName A extends Quest Conditional\n\
                  Int written\nInt read\nInt counted\nInt shown Conditional\nInt unused\n\
                  Int Property Value\n\tInt Function Get()\n\t\tReturn read\n\tEndFunction\nEndProperty\n\
                  Event OnInit()\n\twritten = 1\n\tcounted += 1\nEndEvent\n";
    let found: Vec<String> = one(GameId::Skyrim, source)
        .into_iter()
        .filter(|(p, ..)| *p == Problem::UnusedVariable)
        .map(|(_, t, _)| t)
        .collect();
    // `+=` reads; the game's conditions read `Conditional` variables.
    assert_eq!(found, ["written", "unused"]);
}

#[test]
fn unused_imports_are_reported() {
    let source = "ScriptName A extends Quest\n\
                  Import Utility\nImport Debug\nImport Game\n\
                  Event OnInit()\n\tWait(1.0)\n\tGame.GetPlayer()\nEndEvent\n";
    let found: Vec<String> = one(GameId::Skyrim, source)
        .into_iter()
        .filter(|(p, ..)| *p == Problem::UnusedImport)
        .map(|(_, t, _)| t)
        .collect();
    // `Game.GetPlayer()` needs no `Import Game`.
    assert_eq!(found, ["Import Debug", "Import Game"]);
}
