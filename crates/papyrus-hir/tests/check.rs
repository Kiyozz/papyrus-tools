//! The semantic checks, on chosen cases.

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Problem, Project, Semantics, SourceFile};
use salsa::Setter;

/// True for dead code, which the examples are full of: checked apart.
fn dead_code(problem: Problem) -> bool {
    matches!(
        problem,
        Problem::Unreachable | Problem::UnusedLocal | Problem::UnusedParam
    )
}

/// `(problem, targeted text)` of each diagnostic of the first file but dead
/// code; the others belong to the project, `lib/` ones are imported.
fn check_in(game: Option<GameId>, files: &[(&str, &str)]) -> Vec<(Problem, String, String)> {
    check_all_in(game, files)
        .into_iter()
        .filter(|(p, ..)| !dead_code(*p))
        .collect()
}

/// [`check_in`], dead code included.
fn check_all_in(game: Option<GameId>, files: &[(&str, &str)]) -> Vec<(Problem, String, String)> {
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
fn skse_functions_need_skse() {
    let source = "ScriptName A extends Quest\nEvent OnInit()\n\tGame.GetPerkPoints()\n\tGame.GetPlayer().GetWornForm(4)\nEndEvent\n";
    assert_eq!(
        found(GameId::Skyrim, source),
        [
            (Problem::SkseMember, "GetPerkPoints".to_owned()),
            (Problem::SkseMember, "GetWornForm".to_owned())
        ]
    );
    let message = &check(GameId::Skyrim, source)[0].2;
    assert!(message.contains("set `skse = true`"), "{message}");
    // A typo stays a typo.
    let typo = source.replace("GetWornForm", "GetWornFrom");
    assert_eq!(
        found(GameId::Skyrim, &typo)[1],
        (Problem::UnknownMember, "GetWornFrom".to_owned())
    );

    // SKSE's own scripts, as a type and as a receiver.
    let scripts = "ScriptName A extends Quest\nColorForm Property Tint Auto\nEvent OnInit()\n\tStringUtil.GetLength(\"a\")\nEndEvent\n";
    assert_eq!(
        found(GameId::Skyrim, scripts),
        [
            (Problem::SkseMember, "ColorForm".to_owned()),
            (Problem::SkseMember, "StringUtil".to_owned())
        ]
    );
    let message = &check(GameId::Skyrim, scripts)[1].2;
    assert!(
        message.starts_with("`StringUtil` is a script of SKSE"),
        "{message}"
    );
    assert_eq!(found(GameId::Fallout4, scripts), []);

    // `skse = true`.
    let mut db = Database::default();
    let file = SourceFile::new(&db, "A.psc".into(), source.to_owned());
    let project = Project::new(&db, vec![file], Vec::new(), Some(GameId::Skyrim));
    project.set_skse(&mut db).to(true);
    assert_eq!(Semantics::new(&db, project).check(file), []);

    // SKSE's sources among the imports.
    let files = [
        ("A.psc", source),
        (
            "lib/SKSE.psc",
            "ScriptName SKSE Hidden\nint Function GetVersion() global native\n",
        ),
    ];
    assert_eq!(check_in(Some(GameId::Skyrim), &files), []);
}

#[test]
fn unknown_scripts_need_the_games_sources() {
    let source = "ScriptName A extends Quest\nActr Property Who Auto\nEvent OnInit()\n\tMissing.Run()\n\tWho.Kill()\nEndEvent\n";
    // Without the game's sources, a script may simply not be imported.
    let alone: Vec<Problem> = check(GameId::Skyrim, source)
        .into_iter()
        .map(|(p, ..)| p)
        .collect();
    assert_eq!(alone, []);

    let files = [
        ("A.psc", source),
        ("lib/Form.psc", "ScriptName Form Hidden\n"),
        (
            "lib/ObjectReference.psc",
            "ScriptName ObjectReference extends Form Hidden\n",
        ),
        (
            "lib/Actor.psc",
            "ScriptName Actor extends ObjectReference Hidden\n",
        ),
        ("lib/Quest.psc", "ScriptName Quest extends Form Hidden\n"),
    ];
    let found = check_in(Some(GameId::Skyrim), &files);
    let short: Vec<(Problem, &str)> = found.iter().map(|(p, t, _)| (*p, t.as_str())).collect();
    assert_eq!(
        short,
        [
            (Problem::UnknownScript, "Actr"),
            (Problem::UnknownScript, "Missing")
        ]
    );
    assert!(
        found[0].2.ends_with("did you mean `Actor`?"),
        "{}",
        found[0].2
    );
    assert_eq!(found[1].2, "`Missing` is neither a variable nor a script");
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
fn without_a_target_what_every_game_rejects() {
    let source = script(
        "\tTake()\n\tTake(\"x\")\n\tTake(Utility.RandomInt())\n\tTake(1, Game.GetPlayer())\n\
         \tDebug.Trace()\n\tFoo(value)\n\tbar = 1\n\
         \tint[] values = new int[2]\n\tvalues.Add(1)\n\tStartTimer(1.0)",
    );
    let found: Vec<(Problem, String)> = check_in(None, &[("A.psc", &source)])
        .into_iter()
        .map(|(p, t, _)| (p, t))
        .collect();
    // `Add` only fails on Skyrim, `StartTimer` only exists from Fallout 4:
    // without a target, they may be right.
    assert_eq!(
        found,
        [
            (Problem::Arity, "()".to_owned()),
            (Problem::Mismatch, "\"x\"".to_owned()),
            (Problem::Mismatch, "Game.GetPlayer()".to_owned()),
            (Problem::Arity, "()".to_owned()),
            (Problem::UnknownMember, "Foo".to_owned()),
            (Problem::UnknownMember, "value".to_owned()),
            (Problem::UnknownMember, "bar".to_owned()),
        ]
    );
}

// ── States ────────────────────────────────────────────────────────────────

/// `(problem, targeted text)` of the problems in `kinds`.
fn only(found: Vec<(Problem, String, String)>, kinds: &[Problem]) -> Vec<(Problem, String)> {
    found
        .into_iter()
        .filter(|(p, ..)| kinds.contains(p))
        .map(|(p, t, _)| (p, t))
        .collect()
}

#[test]
fn gotostate_to_an_undeclared_state_is_reported() {
    let source = "ScriptName A extends ObjectReference
State Waiting
EndState
Event OnInit()
\tGotoState(\"Waitng\")
\tGotoState(\"waiting\")
\tGotoState(\"\")
\tself.GotoState(\"Done\")
EndEvent
";
    let found = check(GameId::Skyrim, source);
    assert_eq!(
        only(found.clone(), &[Problem::UnknownState]),
        [
            (Problem::UnknownState, "Waitng".to_owned()),
            (Problem::UnknownState, "Done".to_owned()),
        ]
    );
    assert!(
        found[0].2.contains("did you mean `Waiting`?"),
        "{}",
        found[0].2
    );
}

#[test]
fn states_come_from_parents_and_derived_scripts() {
    let base = "ScriptName Base extends ObjectReference
State Busy
EndState
Event OnInit()
\tGotoState(\"Child\")
EndEvent
";
    let child = "ScriptName Child extends Base
State Child
EndState
Event OnActivate(ObjectReference akActionRef)
\tGotoState(\"Busy\")
EndEvent
";
    let game = Some(GameId::Skyrim);
    for files in [
        [("Base.psc", base), ("Child.psc", child)],
        [("Child.psc", child), ("Base.psc", base)],
    ] {
        assert_eq!(only(check_in(game, &files), &[Problem::UnknownState]), []);
    }
    // A parent outside the project: its states are unknown.
    assert_eq!(
        only(
            check_in(game, &[("Child.psc", child)]),
            &[Problem::UnknownState]
        ),
        []
    );
}

// ── Events ────────────────────────────────────────────────────────────────

#[test]
fn an_event_must_take_the_engine_parameters() {
    let source = "ScriptName A extends ObjectReference
Event OnActivate(Actor akActionRef)
EndEvent
Event OnLoad(int x)
EndEvent
Event OnContainerChanged(ObjectReference akNewContainer, ObjectReference akOldContainer)
EndEvent
Event OnMyOwnEvent(int x)
EndEvent
";
    let found = check(GameId::Skyrim, source);
    assert_eq!(
        only(found.clone(), &[Problem::EventSignature]),
        [
            (Problem::EventSignature, "OnActivate".to_owned()),
            (Problem::EventSignature, "OnLoad".to_owned()),
        ]
    );
    assert!(
        found[0]
            .2
            .contains("`Event OnActivate(ObjectReference akActionRef)`"),
        "{}",
        found[0].2
    );
}

#[test]
fn remote_and_state_events_are_checked() {
    let source = "ScriptName A extends Quest
Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)
EndEvent
Event Actor.OnDeath(Actor akKiller)
EndEvent
Event OnStageSet(int auiStageID, int auiItemID)
EndEvent
State Busy
\tEvent OnStageSet(int auiStageID)
\tEndEvent
EndState
";
    let found = check(GameId::Fallout4, source);
    assert_eq!(
        only(found.clone(), &[Problem::EventSignature]),
        [
            (Problem::EventSignature, "OnDeath".to_owned()),
            (Problem::EventSignature, "OnStageSet".to_owned()),
        ]
    );
    assert!(
        found[0]
            .2
            .contains("`Event Actor.OnDeath(Actor akSender, Actor akKiller)`"),
        "{}",
        found[0].2
    );
    assert!(found[1].2.contains("the empty state"), "{}", found[1].2);
}

// ── Read-only properties ──────────────────────────────────────────────────

#[test]
fn read_only_properties_cannot_be_assigned() {
    let source = "ScriptName A extends Quest
int Property Fixed = 1 AutoReadOnly
Keyword Property Tag Auto Const
int Property Free Auto
int Property Computed
\tint Function Get()
\t\tReturn 1
\tEndFunction
EndProperty
GlobalVariable Property Counter Auto
Event OnInit()
\tFixed = 2
\tTag = None
\tFree = 2
\tComputed += 1
\tCounter.Value = 1.0
\tGame.GetPlayer().CritStage_None = 1
EndEvent
";
    let found = check(GameId::Fallout4, source);
    assert_eq!(
        only(found.clone(), &[Problem::ReadOnly]),
        [
            (Problem::ReadOnly, "Fixed".to_owned()),
            (Problem::ReadOnly, "Tag".to_owned()),
            (Problem::ReadOnly, "Computed".to_owned()),
            (Problem::ReadOnly, "CritStage_None".to_owned()),
        ]
    );
    assert!(found[0].2.contains("is `AutoReadOnly`"), "{}", found[0].2);
    assert!(found[1].2.contains("is `Const`"), "{}", found[1].2);
    assert!(
        found[2].2.contains("has no `Set` function"),
        "{}",
        found[2].2
    );
}

// ── Global functions ──────────────────────────────────────────────────────

#[test]
fn instance_members_need_an_instance() {
    let source = "ScriptName A extends ObjectReference
int count
Function Member()
EndFunction
Function Helper() Global
EndFunction
Function Run() Global
\tint local = 1
\tlocal += 1
\tMember()
\tHelper()
\tcount = 1
\tself.Disable()
\tDisable()
\tA.Helper()
\tDebug.Trace(\"x\")
EndFunction
Event OnInit()
\tA.Member()
\tObjectReference.Disable()
\tA.Helper()
\tMember()
EndEvent
";
    let found = check(GameId::Skyrim, source);
    assert_eq!(
        only(found, &[Problem::NotGlobal]),
        [
            (Problem::NotGlobal, "Member".to_owned()),
            (Problem::NotGlobal, "count".to_owned()),
            (Problem::NotGlobal, "self".to_owned()),
            (Problem::NotGlobal, "Disable".to_owned()),
            (Problem::NotGlobal, "Member".to_owned()),
            (Problem::NotGlobal, "Disable".to_owned()),
        ]
    );
}

// ── Dead code ─────────────────────────────────────────────────────────────

fn dead(game: GameId, files: &[(&str, &str)]) -> Vec<(Problem, String)> {
    only(
        check_all_in(Some(game), files),
        &[
            Problem::Unreachable,
            Problem::UnusedLocal,
            Problem::UnusedParam,
        ],
    )
}

#[test]
fn code_after_return_never_runs() {
    let source = "ScriptName A extends Quest
int Function Pick(bool a)
\tif a
\t\tReturn 1
\t\tDebug.Trace(\"never\")
\telse
\t\tReturn 2
\tendif
\tReturn 3
EndFunction
Function Loop()
\tWhile True
\t\tUtility.Wait(1.0)
\tEndWhile
\tDebug.Trace(\"never\")
\tDebug.Trace(\"either\")
EndFunction
";
    assert_eq!(
        dead(GameId::Skyrim, &[("A.psc", source)]),
        [
            (Problem::Unreachable, "Debug.Trace(\"never\")".to_owned()),
            (Problem::Unreachable, "Return 3".to_owned()),
            (
                Problem::Unreachable,
                "Debug.Trace(\"never\")\n\tDebug.Trace(\"either\")".to_owned()
            ),
        ]
    );
}

#[test]
fn locals_never_read_are_reported() {
    let source = "ScriptName A extends Quest
Function Run()
\tint unused = 1
\tint written = 0
\twritten = 2
\tint counted = 0
\tcounted += 1
\tint read = 1
\tDebug.Trace(read)
\tint i = 0
\tWhile i < 3
\t\ti += 1
\tEndWhile
EndFunction
";
    let found = check_all_in(Some(GameId::Skyrim), &[("A.psc", source)]);
    assert_eq!(
        only(found.clone(), &[Problem::UnusedLocal]),
        [
            (Problem::UnusedLocal, "unused".to_owned()),
            (Problem::UnusedLocal, "written".to_owned()),
        ]
    );
    assert!(
        found[1].2.contains("assigned but never read"),
        "{}",
        found[1].2
    );
}

#[test]
fn only_trailing_unused_parameters_are_reported() {
    let source = "ScriptName A extends ObjectReference
Function Second(int a, int b)
\tDebug.Trace(b)
EndFunction
Function Trailing(int a, int b, int c)
\tDebug.Trace(a)
EndFunction
Function Stub(int a)
EndFunction
Function Fragment_3(ObjectReference akSpeakerRef)
\tActor akSpeaker = akSpeakerRef as Actor
\tDebug.Trace(\"x\")
EndFunction
Event OnActivate(ObjectReference akActionRef)
\tDebug.Trace(\"x\")
EndEvent
Function Activate(ObjectReference akActivator, bool abDefaultProcessingOnly = false)
\tDebug.Trace(\"x\")
EndFunction
Function Stated(int a)
\tDebug.Trace(\"x\")
EndFunction
State Busy
\tFunction Stated(int a)
\t\tDebug.Trace(a)
\tEndFunction
EndState
";
    assert_eq!(
        dead(GameId::Skyrim, &[("A.psc", source)]),
        [
            (Problem::UnusedParam, "b".to_owned()),
            (Problem::UnusedParam, "c".to_owned()),
        ]
    );
}

#[test]
fn a_function_overridden_by_a_derived_script_keeps_its_parameters() {
    let base = "ScriptName Base extends Quest
Function Handle(int a)
\tDebug.Trace(\"x\")
EndFunction
";
    let child = "ScriptName Child extends Base
Function Handle(int a)
\tDebug.Trace(a)
EndFunction
";
    for files in [
        [("Base.psc", base), ("Child.psc", child)],
        [("Child.psc", child), ("Base.psc", base)],
    ] {
        assert_eq!(dead(GameId::Skyrim, &files), []);
    }
}

#[test]
fn locals_the_compiler_refuses_are_reported() {
    // What PapyrusCompiler refuses, Skyrim's and Fallout 4's alike.
    let source = "ScriptName A extends Quest
int field = 0
int Property P Auto
Function Run(int a)
\tint test = 32
\tint TEST = 33
\tint a = 1
\tint d = 1
\tIf a
\t\tint d = 2
\t\tint e = 1
\tElse
\t\tint e = 2
\tEndIf
\tint e = 3
\tWhile a
\t\tint a = 4
\tEndWhile
\tint field = 5
\tint P = 6
EndFunction
Function Other()
\tint test = 1
\tDebug.Trace(test)
EndFunction
";
    let found = check_all_in(Some(GameId::Skyrim), &[("A.psc", source)]);
    // One problem per name: never unused on top of duplicate.
    let unused: Vec<String> = found
        .iter()
        .filter(|(p, ..)| matches!(p, Problem::UnusedLocal | Problem::UnusedParam))
        .map(|(_, t, _)| t.to_ascii_lowercase())
        .collect();
    for name in ["test", "a", "d", "field"] {
        assert!(!unused.iter().any(|u| u == name), "{name}: {unused:?}");
    }
    assert!(unused.iter().any(|u| u == "e"), "{unused:?}");
    let duplicates: Vec<(String, String)> = found
        .into_iter()
        .filter(|(p, ..)| *p == Problem::DuplicateLocal)
        .map(|(_, t, m)| (t, m))
        .collect();
    let message = |name: &str, why: &str| (name.to_owned(), format!("the variable `{name}` {why}"));
    assert_eq!(
        duplicates,
        [
            message("TEST", "is already declared in this block"),
            message("a", "is already a parameter of this function"),
            message(
                "d",
                "is already declared in an enclosing block, which Papyrus forbids hiding"
            ),
            message(
                "a",
                "is already declared in an enclosing block, which Papyrus forbids hiding"
            ),
            message("field", "is already a variable of the script"),
        ]
    );

    let fixed = fixes(GameId::Skyrim, source, Problem::DuplicateLocal);
    assert_eq!(fixed.len(), 1, "{fixed:?}");
    assert_eq!(fixed[0].0, "Assign `TEST` instead");
    assert!(fixed[0].1.contains("\n\tTEST = 33\n"), "{}", fixed[0].1);
}

#[test]
fn names_follow_the_case_of_their_declaration() {
    let source = "ScriptName A extends Quest
Struct Point
\tint x
EndStruct
event OnInit()
\tInitialize()
endevent
function initialize()
\tdebug.trace(\"x\")
\tint count = 1
\tDebug.Trace(COUNT)
\tobjectReference ref = None
\tpoint[] points
\tTake(akWho = None)
endfunction
Function Take(Actor AKWHO)
EndFunction
";
    let found = check_all_in(Some(GameId::Fallout4), &[("A.psc", source)]);
    assert_eq!(
        only(found, &[Problem::NameCase]),
        [
            (Problem::NameCase, "Initialize".to_owned()),
            (Problem::NameCase, "debug".to_owned()),
            (Problem::NameCase, "trace".to_owned()),
            (Problem::NameCase, "COUNT".to_owned()),
            (Problem::NameCase, "objectReference".to_owned()),
            (Problem::NameCase, "point".to_owned()),
        ]
    );
    // Across files: a member and its script.
    let main = "ScriptName A extends Quest
Function Run()
\tTestScript.test()
\ttestScript.Test()
\tTestScript.Test()
EndFunction
";
    let lib = "ScriptName TestScript Hidden\nFunction Test() global\nEndFunction\n";
    let found = check_all_in(
        Some(GameId::Skyrim),
        &[("A.psc", main), ("TestScript.psc", lib)],
    );
    assert_eq!(
        only(found, &[Problem::NameCase]),
        [
            (Problem::NameCase, "test".to_owned()),
            (Problem::NameCase, "testScript".to_owned()),
        ]
    );

    let fixed = fixes(GameId::Fallout4, source, Problem::NameCase);
    assert_eq!(fixed[0].0, "Write `initialize`");
    assert!(fixed[0].1.contains("\n\tinitialize()\n"), "{}", fixed[0].1);
}

// ── Fixes ─────────────────────────────────────────────────────────────────

/// `(title, fixed text)` of each fix of the diagnostics of `problem`.
fn fixes(game: GameId, text: &str, problem: Problem) -> Vec<(String, String)> {
    let db = Database::default();
    let file = SourceFile::new(&db, "A.psc".into(), text.to_owned());
    let project = Project::new(&db, vec![file], Vec::new(), Some(game));
    let sema = Semantics::new(&db, project);
    sema.check(file)
        .into_iter()
        .filter(|d| d.problem == problem)
        .flat_map(|d| d.fixes)
        .map(|fix| {
            let mut out = text.to_owned();
            let mut edits = fix.edits;
            edits.sort_by_key(|e| std::cmp::Reverse(e.range.start()));
            for edit in edits {
                out.replace_range(std::ops::Range::<usize>::from(edit.range), &edit.text);
            }
            (fix.title, out)
        })
        .collect()
}

/// The line of `text` holding `needle`.
fn line_of<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no `{needle}` in:\n{text}"))
}

#[test]
fn a_mismatch_offers_a_cast() {
    let source = script(
        "\tint a = 1.5\n\tint b = 1.0 + 2.0\n\tActor c = Game.GetPlayer() as ObjectReference\n\
         \tQuest d = Game.GetPlayer()",
    );
    let found = fixes(GameId::Skyrim, &source, Problem::Mismatch);
    let lines: Vec<(&str, &str)> = found
        .iter()
        .map(|(title, text)| (title.as_str(), line_of(text, " as ").trim()))
        .collect();
    // `Quest` is no descendant of `Actor`: no cast.
    assert_eq!(
        lines,
        [
            ("Cast with `as int`", "int a = 1.5 as int"),
            ("Cast with `as int`", "int b = (1.0 + 2.0) as int"),
            (
                "Cast with `as Actor`",
                "Actor c = Game.GetPlayer() as Actor"
            ),
        ]
    );
}

#[test]
fn a_missing_argument_is_added() {
    let source = script("\tTake()\n\tTake(akWho = None)\n\tUtility.Wait()");
    let found = fixes(GameId::Skyrim, &source, Problem::Arity);
    let calls: Vec<(&str, &str)> = found
        .iter()
        .zip(["Take(0)", "Take(akWho = None, aiCount = 0)", "Wait(0.0)"])
        .map(|((title, text), call)| (title.as_str(), line_of(text, call).trim()))
        .collect();
    assert_eq!(
        calls,
        [
            ("Add the missing argument `aiCount`", "Take(0)"),
            (
                "Add the missing argument `aiCount`",
                "Take(akWho = None, aiCount = 0)"
            ),
            ("Add the missing argument `afSeconds`", "Utility.Wait(0.0)"),
        ]
    );
}

#[test]
fn optional_parameters_before_a_missing_one_get_their_default() {
    let source = "ScriptName A extends Quest
Function Take(int a, float b = 1.0, Actor c)
EndFunction
Event OnInit()
\tTake(1)
EndEvent
";
    let found = fixes(GameId::Skyrim, source, Problem::Arity);
    assert_eq!(found.len(), 1);
    assert_eq!(line_of(&found[0].1, "Take(1").trim(), "Take(1, 1.0, None)");
}

#[test]
fn an_unknown_function_can_be_created() {
    let source = "ScriptName A extends Quest
Event OnInit()
\tActor who = Game.GetPlayer()
\tint count = CountItems(who, 2, abSilent = true)
\tself.Notify(\"done\")
\tObscure(Missing)
EndEvent
int Function Helper() Global
\tReturn Compute(1.5)
EndFunction
";
    let found = fixes(GameId::Skyrim, source, Problem::UnknownMember);
    let created: Vec<(&str, &str)> = found
        .iter()
        .filter(|(title, _)| title.starts_with("Create"))
        .map(|(title, text)| (title.as_str(), &text[source.len()..]))
        .collect();
    // `Missing` has no known type: `Obscure` is not offered.
    assert_eq!(
        created,
        [
            (
                "Create the function `CountItems`",
                "\nint Function CountItems(Actor who, int param2, bool abSilent)\nEndFunction\n"
            ),
            (
                "Create the function `Notify`",
                "\nFunction Notify(string param1)\nEndFunction\n"
            ),
            (
                "Create the function `Compute`",
                "\nint Function Compute(float param1) Global\nEndFunction\n"
            ),
        ]
    );
}
