//! The semantic checks, on chosen cases.

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Problem, Project, Semantics, SourceFile};

/// `(problem, targeted text)` of each diagnostic of the first file; the
/// others belong to the project, `lib/` ones are imported.
fn check_in(game: Option<GameId>, files: &[(&str, &str)]) -> Vec<(Problem, String, String)> {
    let db = Database::default();
    let mut own = Vec::new();
    let mut imports = Vec::new();
    for (name, text) in files {
        let file = SourceFile::new(&db, (*name).into(), (*text).to_owned());
        if name.starts_with("lib/") {
            imports.push(file);
        } else {
            own.push(file);
        }
    }
    let first = own[0];
    let project = Project::new(&db, own, imports, game);
    let sema = Semantics::new(&db, project);
    let text = first.text(&db);
    sema.check(first)
        .into_iter()
        .map(|d| (d.problem, text[d.range].to_owned(), d.message))
        .collect()
}

fn check(game: GameId, text: &str) -> Vec<(Problem, String, String)> {
    check_in(Some(game), &[("A.psc", text)])
}

/// `(problem, targeted text)`, messages dropped.
fn found(game: GameId, text: &str) -> Vec<(Problem, String)> {
    check(game, text)
        .into_iter()
        .map(|(p, t, _)| (p, t))
        .collect()
}

fn script(body: &str) -> String {
    format!(
        "ScriptName A extends ObjectReference\n\
         Function Take(int aiCount, float afScale = 1.0, Actor akWho = None)\nEndFunction\n\
         Event OnInit()\n{body}\nEndEvent\n"
    )
}

// ── Arity and named arguments ────────────────────────────────────────────

#[test]
fn too_many_and_missing_arguments_are_reported() {
    let source = script("\tTake(1, 2.0, None, 4, 5)\n\tTake()\n\tTake(1)\n\tTake(1, akWho = None)");
    let found = check(GameId::Skyrim, &source);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!((found[0].0, found[0].1.as_str()), (Problem::Arity, "4, 5"));
    assert!(
        found[0].2.contains("takes 3 arguments, 5 given"),
        "{}",
        found[0].2
    );
    assert_eq!((found[1].0, found[1].1.as_str()), (Problem::Arity, "()"));
    assert!(
        found[1].2.contains("missing argument `aiCount`"),
        "{}",
        found[1].2
    );
}

#[test]
fn native_functions_have_their_arity() {
    let source = script("\tDebug.Trace(\"x\", 0, 1)\n\tUtility.Wait()\n\tDisable(true)");
    let found = found(GameId::Skyrim, &source);
    assert_eq!(
        found,
        [
            (Problem::Arity, "1".to_owned()),
            (Problem::Arity, "()".to_owned()),
        ]
    );
}

#[test]
fn named_arguments_must_exist_once() {
    let source = script("\tTake(1, afScal = 2.0)\n\tTake(1, aiCount = 2)\n\tTake(aiCount = 1)");
    let found = check(GameId::Skyrim, &source);
    let summary: Vec<(Problem, &str)> = found.iter().map(|(p, t, _)| (*p, t.as_str())).collect();
    assert_eq!(
        summary,
        [
            (Problem::NamedArg, "afScal"),
            (Problem::NamedArg, "aiCount")
        ]
    );
    assert!(
        found[0].2.contains("did you mean `afScale`?"),
        "{}",
        found[0].2
    );
    assert!(found[1].2.contains("already given"), "{}", found[1].2);
}

// ── Types ─────────────────────────────────────────────────────────────────

#[test]
fn implicit_casts_follow_the_compiler() {
    let source = "\
ScriptName A extends ObjectReference
Function Number(int i, float f)
EndFunction
Function Anything(string s, bool b)
EndFunction
Function Who(Actor a, ObjectReference r)
EndFunction
Event OnInit()
\tActor player = Game.GetPlayer()
\tObjectReference ref = player
\tNumber(1, 1)
\tNumber(1.5, 2.5)
\tAnything(player, ref)
\tAnything(1.0, None)
\tWho(None, player)
\tWho(ref, ref)
\tNumber(\"1\", None)
EndEvent
";
    let found = check(GameId::Skyrim, source);
    let summary: Vec<(Problem, &str)> = found.iter().map(|(p, t, _)| (*p, t.as_str())).collect();
    assert_eq!(
        summary,
        [
            (Problem::Mismatch, "1.5"),
            (Problem::Mismatch, "ref"),
            (Problem::Mismatch, "\"1\""),
            (Problem::Mismatch, "None"),
        ]
    );
    assert!(
        found[0]
            .2
            .contains("`Number` expects `int` for `i`, found `float` — cast it with `as int`"),
        "{}",
        found[0].2
    );
    assert!(
        found[1].2.contains("cast it with `as Actor`"),
        "{}",
        found[1].2
    );
}

#[test]
fn assignments_initializers_and_returns_are_typed() {
    let source = "\
ScriptName A extends ObjectReference
int count = 1.5
float scale = 1
int Function Get()
\tReturn \"x\"
EndFunction
Event OnInit()
\tcount = 2.0
\tcount = 1 + 2
\tscale = 1 + 2.0
\tcount = 1 + 2.0
\tstring s = 1 + \"x\"
\tbool b = s
EndEvent
";
    let found = found(GameId::Skyrim, source);
    assert_eq!(
        found,
        [
            (Problem::Mismatch, "1.5".to_owned()),
            (Problem::Mismatch, "\"x\"".to_owned()),
            (Problem::Mismatch, "2.0".to_owned()),
            (Problem::Mismatch, "1 + 2.0".to_owned()),
        ]
    );
}

#[test]
fn arrays_and_var_are_judged_cautiously() {
    let source = "\
ScriptName A extends ObjectReference
Function Take(float[] values, ObjectReference[] refs, var anything)
EndFunction
Event OnInit()
\tint[] ints = new int[2]
\tActor[] actors = new Actor[2]
\tvar v = 1
\tTake(ints, actors, ints)
\tTake(None, None, None)
\tint i = v
\tTake(v, v, 1)
EndEvent
";
    // `int[]` is not `float[]`; whether `Actor[]` passes for
    // `ObjectReference[]` is left to the compiler, as is `var`.
    assert_eq!(
        found(GameId::Fallout4, source),
        [(Problem::Mismatch, "ints".to_owned())]
    );
}

#[test]
fn an_unknown_ancestor_silences_types() {
    let source = "\
ScriptName A extends ObjectReference
Function Who(Actor a)
EndFunction
Event OnInit()
\tMyRef other = None
\tWho(other)
\tWho(1)
EndEvent
";
    // `MyRef` may extend `Actor`; `1` never does.
    assert_eq!(
        found(GameId::Skyrim, source),
        [(Problem::Mismatch, "1".to_owned())]
    );

    let files = [
        (
            "A.psc",
            "ScriptName A extends ObjectReference\nFunction Who(Actor a)\nEndFunction\nEvent OnInit()\n\tB other = None\n\tWho(other)\nEndEvent\n",
        ),
        ("B.psc", "ScriptName B extends Missing\n"),
    ];
    assert_eq!(check_in(Some(GameId::Skyrim), &files), []);
}

#[test]
fn structs_are_their_own_type() {
    let source = "\
ScriptName A extends Quest
Struct Entry
\tint count
EndStruct
Struct Other
\tint count
EndStruct
Function Take(Entry e)
EndFunction
Event OnInit()
\tTake(new Entry)
\tTake(new Other)
\tTake(None)
EndEvent
";
    assert_eq!(
        found(GameId::Fallout4, source),
        [(Problem::Mismatch, "new Other".to_owned())]
    );
}

// ── Members ───────────────────────────────────────────────────────────────

#[test]
fn unknown_functions_are_reported_with_a_suggestion() {
    let source = "\
ScriptName A extends ObjectReference
Event OnInit()
\tActor player = Game.GetPlayer()
\tplayer.GetActorBas()
\tDebug.Trac(\"x\")
\tDisabel()
\tself.GetState()
EndEvent
";
    let found = check(GameId::Skyrim, source);
    let summary: Vec<(Problem, &str)> = found.iter().map(|(p, t, _)| (*p, t.as_str())).collect();
    assert_eq!(
        summary,
        [
            (Problem::UnknownMember, "GetActorBas"),
            (Problem::UnknownMember, "Trac"),
            (Problem::UnknownMember, "Disabel"),
        ]
    );
    assert!(
        found[0].2.contains(
            "`Actor` and its parents have no function `GetActorBas` — did you mean `GetActorBase`?"
        ),
        "{}",
        found[0].2
    );
    assert!(
        found[2].2.contains("did you mean `Disable`?"),
        "{}",
        found[2].2
    );
}

#[test]
fn unknown_variables_and_fields_are_reported() {
    let source = "\
ScriptName A extends Quest
GlobalVariable Property Counter Auto
Struct Entry
\tint count
EndStruct
Event OnInit()
\tint total = 0
\ttotl = 1
\tfloat f = Counter.Value
\tf = Counter.Valeu
\tEntry e = new Entry
\te.cont = 1
\tif true
\t\tint later = 1
\tendif
\tlater = 2
EndEvent
";
    let found = check(GameId::Fallout4, source);
    let summary: Vec<(Problem, &str)> = found.iter().map(|(p, t, _)| (*p, t.as_str())).collect();
    assert_eq!(
        summary,
        [
            (Problem::UnknownMember, "totl"),
            (Problem::UnknownMember, "Valeu"),
            (Problem::UnknownMember, "cont"),
        ]
    );
    assert!(
        found[0].2.contains("did you mean `total`?"),
        "{}",
        found[0].2
    );
    assert!(
        found[2].2.contains("struct `Entry` has no field `cont`"),
        "{}",
        found[2].2
    );
}

#[test]
fn skyrim_arrays_only_find() {
    let source = script("\tint[] values = new int[2]\n\tvalues.Find(1)\n\tvalues.Add(1)");
    assert_eq!(
        found(GameId::Skyrim, &source),
        [(Problem::UnknownMember, "Add".to_owned())]
    );
    assert_eq!(found(GameId::Fallout4, &source), []);
}

#[test]
fn unknown_scripts_and_ancestors_stay_quiet() {
    let source = "\
ScriptName A extends MissingParent
Event OnInit()
\tFoo()
\tbar = 1
\tSKSE.GetVersion()
EndEvent
";
    assert_eq!(found(GameId::Skyrim, source), []);

    // Every import must be known for a bare call to be unknown.
    let source = "ScriptName A extends Quest\nimport Missing\nEvent OnInit()\n\tFoo()\nEndEvent\n";
    assert_eq!(found(GameId::Skyrim, source), []);
}

#[test]
fn imported_globals_are_callable() {
    let files = [
        (
            "A.psc",
            "ScriptName A extends Quest\nimport Tools\nEvent OnInit()\n\tHelp(1)\n\tHelp()\nEndEvent\n",
        ),
        (
            "lib/Tools.psc",
            "ScriptName Tools\nFunction Help(int a) Global\nEndFunction\n",
        ),
    ];
    let found: Vec<(Problem, String)> = check_in(Some(GameId::Skyrim), &files)
        .into_iter()
        .map(|(p, t, _)| (p, t))
        .collect();
    assert_eq!(found, [(Problem::Arity, "()".to_owned())]);
}

// ── Returns ───────────────────────────────────────────────────────────────

#[test]
fn typed_functions_must_return_on_every_path() {
    let source = "\
ScriptName A extends Quest
int Function Missing(bool b)
\tif b
\t\tReturn 1
\tendif
EndFunction
int Function Branches(bool b)
\tif b
\t\tReturn 1
\telseif !b
\t\tReturn 2
\telse
\t\tReturn 3
\tendif
EndFunction
int Function Loop()
\tWhile (True)
\tEndWhile
EndFunction
int Function Stub()
EndFunction
Function Untyped()
\tint i = 0
EndFunction
int Function Native() Native
";
    let found = check(GameId::Skyrim, source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].0, found[0].1.as_str()),
        (Problem::MissingReturn, "Missing")
    );
    assert!(found[0].2.contains("returns `int`"), "{}", found[0].2);
}

#[test]
fn nothing_without_a_target() {
    let source = script("\tTake()\n\tFoo()");
    assert_eq!(check_in(None, &[("A.psc", &source)]), []);
}
