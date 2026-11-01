//! What the compilers refuse, each case checked against the official
//! PapyrusCompiler of Skyrim, Fallout 4 and Starfield.

use papyrus_dialect::GameId;
use papyrus_hir::{Database, Problem, Project, Semantics, SourceFile};

const ALL: [GameId; 3] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield];

/// `(problem, targeted text)` of the diagnostics of the first file; `lib/`
/// files are imported, the others belong to the project.
fn check_in(game: GameId, files: &[(&str, &str)]) -> Vec<(Problem, String)> {
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
    let project = Project::new(&db, own, imports, Some(game));
    let sema = Semantics::new(&db, project);
    let text = first.text(&db);
    sema.check(first)
        .into_iter()
        .filter(|d| {
            !matches!(
                d.problem,
                Problem::UnusedLocal | Problem::UnusedParam | Problem::Unreachable
            )
        })
        .map(|d| (d.problem, text[d.range].to_owned()))
        .collect()
}

fn check(game: GameId, text: &str) -> Vec<(Problem, String)> {
    check_in(game, &[("A.psc", text)])
}

/// The diagnostics of `problem`, their targeted text.
fn only(game: GameId, text: &str, problem: Problem) -> Vec<String> {
    check(game, text)
        .into_iter()
        .filter(|(p, _)| *p == problem)
        .map(|(_, t)| t)
        .collect()
}

/// What each game reports of `problem`.
fn per_game(text: &str, problem: Problem) -> [Vec<String>; 3] {
    ALL.map(|game| only(game, text, problem))
}

fn body(statements: &str) -> String {
    format!(
        "ScriptName A extends ObjectReference\n\
         Function Nothing()\nEndFunction\n\
         Function Run(Actor akActor, Quest akQuest, Int aiCount, Float afValue, Bool abFlag, String asText)\n\
         {statements}\nEndFunction\n"
    )
}

// ── The script ───────────────────────────────────────────────────────────

#[test]
fn the_script_is_named_like_its_file() {
    for game in ALL {
        assert_eq!(
            only(game, "ScriptName Other\n", Problem::ScriptName),
            ["Other"]
        );
        assert_eq!(only(game, "scriptname a\n", Problem::ScriptName), [""; 0]);
    }
}

#[test]
fn a_namespace_needs_its_folder() {
    let named = |path, game| {
        let text = "ScriptName MyMod:Sub:Stray\n";
        check_in(game, &[(path, text)])
            .into_iter()
            .filter(|(problem, _)| *problem == Problem::ScriptName)
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
    };
    for game in [GameId::Fallout4, GameId::Starfield] {
        let wrong = ["MyMod:Sub:Stray"];
        assert_eq!(named("Source/Stray.psc", game), wrong);
        assert_eq!(named("Source/Sub/Stray.psc", game), wrong);
        assert_eq!(named("Source/Other/Sub/Stray.psc", game), wrong);
        assert_eq!(named("Source/MyMod/Sub/Stray.psc", game), [""; 0]);
        assert_eq!(named("Source/mymod/SUB/stray.psc", game), [""; 0]);
    }
}

#[test]
fn a_script_cannot_extend_itself() {
    let files = [
        ("A.psc", "ScriptName A extends B\n"),
        ("B.psc", "ScriptName B extends A\n"),
    ];
    assert_eq!(
        check_in(GameId::Skyrim, &files),
        [(Problem::ExtendsCycle, "B".to_owned())]
    );
    assert_eq!(
        only(
            GameId::Skyrim,
            "ScriptName A extends A\n",
            Problem::ExtendsCycle
        ),
        ["A"]
    );
}

// ── Duplicates ───────────────────────────────────────────────────────────

#[test]
fn members_declared_twice_are_reported() {
    let source = "ScriptName A\n\
                  Int x\nInt X\n\
                  Int Property P Auto\nInt Property p Auto\n\
                  Int v\nInt Property V Auto\n\
                  Function F()\nEndFunction\nFunction f()\nEndFunction\n\
                  Auto State One\n  Function F()\n  EndFunction\nEndState\n\
                  State One\n  Function F()\n  EndFunction\nEndState\n\
                  Auto State Two\nEndState\n";
    let [skyrim, fallout4, starfield] = per_game(source, Problem::DuplicateMember);
    // A state may come in two blocks, not hold a function twice; a property
    // named like a variable only from Fallout 4.
    assert_eq!(skyrim, ["X", "p", "f", "F", "Two"]);
    assert_eq!(fallout4, ["X", "p", "V", "f", "F", "Two"]);
    assert_eq!(starfield, fallout4);
}

// ── Returns ──────────────────────────────────────────────────────────────

#[test]
fn returns_match_the_declaration() {
    let source = "ScriptName A extends ObjectReference\n\
                  Function Nothing()\nEndFunction\n\
                  Function Run()\n  Return 5\nEndFunction\n\
                  Int Function Get()\n  Return\nEndFunction\n\
                  Event OnInit()\n  Return 1\nEndEvent\n\
                  Event OnReset()\n  Return\nEndEvent\n\
                  Function Chain()\n  Return Nothing()\nEndFunction\n\
                  Function Clear()\n  Return None\nEndFunction\n";
    let [skyrim, fallout4, starfield] = per_game(source, Problem::ReturnValue);
    assert_eq!(skyrim, ["Return 5", "Return", "Return 1"]);
    // Only Skyrim accepts the value of a call that returns nothing.
    assert_eq!(
        fallout4,
        ["Return 5", "Return", "Return 1", "Return Nothing()"]
    );
    assert_eq!(starfield, fallout4);
}

// ── Overrides ────────────────────────────────────────────────────────────

#[test]
fn overrides_keep_the_signature_and_the_defaults_as_written() {
    let parent = "ScriptName Base\n\
                  Function Take(Int a, Bool b = False, Form c = None, Float d = 1.0)\nEndFunction\n\
                  Int Function Count()\n  Return 0\nEndFunction\n\
                  Function Helper() Global\nEndFunction\n";
    let child = "ScriptName A extends Base\n\
                  Function Take(Int a, Bool b = false, Form c = None, Float d = 1.00)\nEndFunction\n\
                  Bool Function Count()\n  Return True\nEndFunction\n\
                  Function Helper() Global\nEndFunction\n";
    let files = [("A.psc", child), ("Base.psc", parent)];
    for game in ALL {
        let found: Vec<String> = check_in(game, &files)
            .into_iter()
            .filter(|(p, _)| *p == Problem::OverrideSignature)
            .map(|(_, t)| t)
            .collect();
        // `1.00` is `1.0`; `false` is not `False`.
        assert_eq!(found, ["Take", "Count", "Helper"], "{game:?}");
    }
    let types = "ScriptName A extends Base\nFunction Take(Float a)\nEndFunction\n";
    assert_eq!(
        check_in(GameId::Skyrim, &[("A.psc", types), ("Base.psc", parent)]),
        [(Problem::OverrideSignature, "Take".to_owned())]
    );
}

#[test]
fn a_state_compares_with_the_empty_state() {
    let source = "ScriptName A\n\
                  Function Run(Bool a = False)\nEndFunction\n\
                  State Busy\n  Function Run(Bool a = false)\n  EndFunction\nEndState\n";
    for game in ALL {
        assert_eq!(only(game, source, Problem::OverrideSignature), ["Run"]);
    }
}

#[test]
fn native_functions_follow_the_game_rules() {
    // `ObjectReference.Disable(bool abFadeOut = false)`.
    let source = "ScriptName A extends ObjectReference\n\
                  Function Disable(bool abFadeOut = false)\nEndFunction\n\
                  Function OnActivate(ObjectReference akActionRef)\nEndFunction\n";
    let [skyrim, fallout4, starfield] = per_game(source, Problem::OverrideSignature);
    assert_eq!(skyrim, [""; 0]);
    // A function named like an event; Starfield forbids overriding a
    // native function.
    assert_eq!(fallout4, ["OnActivate"]);
    assert_eq!(starfield, ["Disable", "OnActivate"]);
}

// ── States ───────────────────────────────────────────────────────────────

#[test]
fn a_state_function_needs_the_empty_state() {
    let source = "ScriptName A extends ObjectReference\n\
                  Function Known()\nEndFunction\n\
                  State Busy\n\
                  Function Known()\nEndFunction\n\
                  Function Disable(bool abFadeOut = false)\nEndFunction\n\
                  Function Alone()\nEndFunction\n\
                  Function Shared() Global\nEndFunction\n\
                  EndState\n";
    for game in [GameId::Skyrim, GameId::Fallout4] {
        assert_eq!(
            only(game, source, Problem::StateFunction),
            ["Alone", "Shared"],
            "{game:?}"
        );
    }
}

// ── Properties ───────────────────────────────────────────────────────────

#[test]
fn properties_hold_get_and_set() {
    let source = "ScriptName A\n\
                  Int Property Fixed AutoReadOnly\n\
                  Int Property Full = 1\n  Int Function Get()\n    Return 1\n  EndFunction\nEndProperty\n\
                  Int Property Bad\n\
                    Float Function Get()\n    Return 1.0\n  EndFunction\n\
                    Int Function Set(Int v)\n    Return v\n  EndFunction\n\
                    Function Other()\n  EndFunction\n\
                  EndProperty\n\
                  Int Property Good\n\
                    Int Function Get()\n    Return 1\n  EndFunction\n\
                    Function Set(int value)\n  EndFunction\n\
                  EndProperty\n";
    for game in ALL {
        assert_eq!(
            only(game, source, Problem::PropertyAccessor),
            ["Fixed", "Full", "Get", "Set", "Other"],
            "{game:?}"
        );
    }
}

// ── Values ───────────────────────────────────────────────────────────────

#[test]
fn array_sizes_follow_the_game() {
    let source = body(
        "Int[] a = new Int[0]\nInt[] b = new Int[aiCount]\nInt[] c = new Int[200]\n\
         Int[] d = new Int[128]\nInt[] e = new Int[0x10]",
    );
    let [skyrim, fallout4, starfield] = per_game(&source, Problem::ArraySize);
    assert_eq!(skyrim, ["0", "aiCount", "200"]);
    assert_eq!(fallout4, ["200"]);
    assert_eq!(starfield, ["200"]);
}

#[test]
fn only_variables_properties_and_elements_are_assigned() {
    let source = body("Int[] a = new Int[2]\nNothing() = 1\na.Length = 2\na[0] = 1\naiCount = 2");
    for game in ALL {
        assert_eq!(
            only(game, &source, Problem::NotAssignable),
            ["Nothing()", "a.Length"]
        );
    }
}

#[test]
fn initial_values_are_literals() {
    let source = "ScriptName A\n\
                  Int v = 1 + 2\nInt w = -1\nFloat x = -1.5\n\
                  Int Property P = 1 + 2 Auto\n\
                  Function F(Int a = 1 + 2, Int b = -3, String s = \"x\", Form f = None)\nEndFunction\n";
    for game in ALL {
        assert_eq!(
            only(game, source, Problem::NotLiteral),
            ["1 + 2", "1 + 2", "1 + 2"]
        );
    }
}

#[test]
fn integers_fit_32_bits() {
    let source = "ScriptName A\nInt a = 3000000000\nInt b = 0xFFFFFFFF\nInt c = 0x100000000\nInt d = 2147483647\n";
    let [skyrim, fallout4, starfield] = per_game(source, Problem::IntRange);
    assert_eq!(skyrim, ["0x100000000"]);
    assert_eq!(fallout4, ["3000000000", "0x100000000"]);
    assert_eq!(starfield, fallout4);
}

#[test]
fn fallout4_wants_defaults_last_and_native_scripts() {
    let source = "ScriptName A\n\
                  Function F(Int a = 1, Int b, Int c = 2)\nEndFunction\n\
                  Function G() Native\n";
    let [skyrim, fallout4, _] = per_game(source, Problem::DefaultOrder);
    assert_eq!(skyrim, [""; 0]);
    assert_eq!(fallout4, ["Int b"]);
    let [skyrim, fallout4, starfield] = per_game(source, Problem::NativeFunction);
    assert_eq!(skyrim, [""; 0]);
    assert_eq!(fallout4, ["Native"]);
    assert_eq!(starfield, ["Native"]);
    let native = "ScriptName A Native\nFunction G() Native\n";
    assert_eq!(
        only(GameId::Fallout4, native, Problem::NativeFunction),
        [""; 0]
    );
}

// ── Types ────────────────────────────────────────────────────────────────

#[test]
fn operands_and_casts_of_unrelated_types_are_refused() {
    let source = body(
        "Quest q = akActor as Quest\n\
         Int i = akActor as Int\n\
         Bool s = akActor as Bool\n\
         Bool c = akActor == akQuest\n\
         Bool d = akActor == 1\n\
         Bool e = akActor == True\n\
         Bool f = akActor != None\n\
         Int g = abFlag + 1\n\
         String h = asText - \"a\"\n\
         String k = \"a\" + akActor\n\
         Float m = afValue % 2.0\n\
         Int n = aiCount % 2\n\
         Bool o = abFlag < abFlag\n\
         Bool p = asText < aiCount\n\
         Bool r = -abFlag\n\
         Bool t = !akActor",
    );
    let found = only(GameId::Skyrim, &source, Problem::Mismatch);
    assert_eq!(
        found,
        [
            "akActor as Quest",
            "akActor as Int",
            "akActor == akQuest",
            "akActor == 1",
            "abFlag + 1",
            "asText - \"a\"",
            "afValue % 2.0",
            "abFlag < abFlag",
            "-abFlag",
        ]
    );
}

#[test]
fn a_call_returning_nothing_is_not_a_value() {
    let source = body(
        "Int i = Nothing()\nTake(Nothing())\nObjectReference r = Nothing()\n\
         If Nothing()\nEndIf",
    )
    .replace(
        "Function Nothing()",
        "Function Take(Int a)\nEndFunction\nFunction Nothing()",
    );
    let [skyrim, fallout4, _] = per_game(&source, Problem::Mismatch);
    // Like `None`: an object accepts it, a number does not; from Fallout 4,
    // neither does a condition.
    assert_eq!(skyrim, ["Nothing()", "Nothing()"]);
    assert_eq!(fallout4, ["Nothing()", "Nothing()", "Nothing()"]);
}
