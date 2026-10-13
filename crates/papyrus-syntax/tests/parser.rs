//! The shape of the tree.
//!
//! The snapshots show nodes and significant tokens; trivia is omitted for
//! readability, its preservation being checked separately. `⏎` marks a line
//! end, which is a token of its own.

use std::fmt::Write;

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::{NodeOrToken, Parse, SyntaxKind, SyntaxNode, parse};

/// The tree, one line per element, followed by the errors.
fn render(parse: &Parse, source: &str) -> String {
    fn walk(node: &SyntaxNode, depth: usize, out: &mut String) {
        writeln!(out, "{:indent$}{:?}", "", node.kind(), indent = depth * 2).unwrap();
        for child in node.children_with_tokens() {
            match child {
                NodeOrToken::Node(n) => walk(&n, depth + 1, out),
                NodeOrToken::Token(t) if t.kind() == SyntaxKind::NEWLINE => {
                    writeln!(out, "{:indent$}⏎", "", indent = (depth + 1) * 2).unwrap();
                }
                NodeOrToken::Token(t) if !t.kind().is_trivia() => {
                    writeln!(
                        out,
                        "{:indent$}{:?} {:?}",
                        "",
                        t.kind(),
                        t.text(),
                        indent = (depth + 1) * 2
                    )
                    .unwrap();
                }
                NodeOrToken::Token(_) => {}
            }
        }
    }

    let mut out = String::new();
    walk(&parse.syntax(), 0, &mut out);
    for error in parse.errors() {
        let start = usize::from(error.range().start());
        let end = usize::from(error.range().end());
        writeln!(
            out,
            "error {start}..{end} {:?}: {}",
            &source[start..end],
            error.message()
        )
        .unwrap();
    }
    out
}

/// Parses for `game`, checks the round trip, returns the tree.
#[track_caller]
fn tree(source: &str, game: GameId) -> String {
    let parse = parse(source, Dialect::new(game));
    assert_eq!(parse.syntax().to_string(), source, "round trip broken");
    render(&parse, source)
}

/// Like [`tree`], inside a function, to test statements.
#[track_caller]
fn body(stmts: &str, game: GameId) -> String {
    tree(
        &format!("ScriptName T\nFunction F()\n{stmts}\nEndFunction\n"),
        game,
    )
}

#[test]
fn full_header() {
    insta::assert_snapshot!(tree(
        "ScriptName DLC01:Foo extends ObjectReference Hidden Const\nimport Utility\n",
        GameId::Fallout4
    ));
}

#[test]
fn operator_precedence() {
    // `*` binds tighter than `+`, `&&` tighter than `||`, and unary wraps the
    // cast.
    insta::assert_snapshot!(body(
        "x = a + b * c\ny = a || b && c\nz = -w as Float",
        GameId::Skyrim
    ));
}

#[test]
fn declaration_assignment_or_call() {
    // All three start with an identifier; only the first is a type followed
    // by a name.
    insta::assert_snapshot!(body(
        "Actor[] list = new Actor[4]\nlist[0] = Game.GetPlayer()\nDebug.Trace(list.Length, aiSeverity = 2)",
        GameId::Skyrim
    ));
}

#[test]
fn if_elseif_else() {
    insta::assert_snapshot!(body(
        "If a\n\tx = 1\nElseIf b\n\tx = 2\nElse\n\tReturn\nEndIf",
        GameId::Skyrim
    ));
}

#[test]
fn short_and_long_property() {
    // The form is decided by the flags, not by their position.
    insta::assert_snapshot!(tree(
        "ScriptName T\n\
         Keyword Property K Mandatory Const Auto\n\
         float Property X Hidden\n\
         \tfloat Function Get()\n\
         \t\tReturn 1.0\n\
         \tEndFunction\n\
         EndProperty\n",
        GameId::Fallout4
    ));
}

#[test]
fn native_function_without_body() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nint Function Count() global native\nFunction Next()\nEndFunction\n",
        GameId::Skyrim
    ));
}

#[test]
fn remote_event() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nEvent ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)\nEndEvent\n",
        GameId::Fallout4
    ));
}

#[test]
fn state_struct_group_customevent() {
    insta::assert_snapshot!(tree(
        "ScriptName T\n\
         Auto State Default\n\
         \tEvent OnInit()\n\
         \tEndEvent\n\
         EndState\n\
         Struct Entry\n\
         \tint Count = 1\n\
         EndStruct\n\
         Group Mandatory CollapsedOnRef\n\
         \tActor Property Who Auto\n\
         EndGroup\n\
         CustomEvent OnDone\n",
        GameId::Fallout4
    ));
}

#[test]
fn starfield_guards() {
    insta::assert_snapshot!(tree(
        "ScriptName T\n\
         Guard g ProtectsFunctionLogic\n\
         int count RequiresGuard(g)\n\
         Function F()\n\
         \tLockGuard g\n\
         \t\tcount += 1\n\
         \tEndLockGuard\n\
         \tTryLockGuard g\n\
         \t\tcount += 1\n\
         \tElse\n\
         \t\tReturn\n\
         \tEndTryLockGuard\n\
         EndFunction\n",
        GameId::Starfield
    ));
}

#[test]
fn qualified_static_call() {
    insta::assert_snapshot!(body(
        "DLC04:Manager.GetScript().Attach(akRef as Actor)",
        GameId::Fallout4
    ));
}

// ── Dialects ──────────────────────────────────────────────────────────────

#[test]
fn a_skyrim_script_may_name_a_variable_var() {
    let parse = parse(
        "ScriptName T\nFunction F()\n\tint Var = 1\n\tVar = 2\nEndFunction\n",
        Dialect::new(GameId::Skyrim),
    );
    assert!(parse.ok(), "{:?}", parse.errors());
}

#[test]
fn is_is_refused_in_skyrim_with_the_reason() {
    let source = "ScriptName T
Function F()
	bool b = x is Actor
EndFunction
";
    let skyrim = parse(source, Dialect::new(GameId::Skyrim));
    assert_eq!(skyrim.errors().len(), 1, "{:?}", skyrim.errors());
    assert!(
        skyrim.errors()[0]
            .message()
            .contains("`is` is only a keyword since Fallout 4"),
        "{}",
        skyrim.errors()[0]
    );
    assert!(parse(source, Dialect::new(GameId::Fallout4)).ok());
}

#[test]
fn remote_events_are_reported_in_skyrim() {
    let source = "ScriptName T
Event Actor.OnDeath(Actor akSender, Actor akKiller)
EndEvent
";
    let skyrim = parse(source, Dialect::new(GameId::Skyrim));
    assert_eq!(skyrim.errors().len(), 1, "{:?}", skyrim.errors());
    assert!(skyrim.errors()[0].message().contains("remote events"));
    assert_eq!(skyrim.syntax().to_string(), source);

    assert!(parse(source, Dialect::new(GameId::Fallout4)).ok());
}

#[test]
fn namespaces_are_reported_in_skyrim() {
    // The dialect's second intervention point: the construct is read, the
    // tree is complete, and the target is reported as incompatible.
    let source = "ScriptName T\nFoo:Bar Property P Auto\n";
    let skyrim = parse(source, Dialect::new(GameId::Skyrim));
    assert_eq!(skyrim.errors().len(), 1, "{:?}", skyrim.errors());
    assert!(skyrim.errors()[0].message().contains("Fallout 4"));
    assert_eq!(
        skyrim.syntax().to_string(),
        source,
        "l'arbre doit rester complet"
    );

    assert!(parse(source, Dialect::new(GameId::Fallout4)).ok());
}

// ── Recovery ──────────────────────────────────────────────────────────────

#[test]
fn an_invalid_line_stays_on_its_line() {
    insta::assert_snapshot!(body("x = 1 2 3\ny = )\nz = 3", GameId::Skyrim));
}

#[test]
fn a_missing_endif_does_not_swallow_the_next_function() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nFunction A()\n\tIf x\n\t\ty()\nEndFunction\nFunction B()\nEndFunction\n",
        GameId::Skyrim
    ));
}

#[test]
fn a_missing_endproperty_does_not_swallow_what_follows() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nint Property P\nint Property Q Auto\n",
        GameId::Skyrim
    ));
}

#[test]
fn a_script_without_header_is_reported() {
    let parse = parse("Function F()\nEndFunction\n", Dialect::default());
    assert_eq!(parse.errors().len(), 1);
    assert!(parse.errors()[0].message().contains("ScriptName"));
}

// ── Lossless ──────────────────────────────────────────────────────────────

/// A script covering most of the grammar, comments included.
const FULL: &str = "\u{feff}ScriptName Foo:Bar extends Quest Conditional ; naïve header\n\
{Documentation}\n\
import Utility\n\
\n\
int Property Count = 3 Auto Const\n\
Actor Property Who Hidden\n\
\tActor Function Get()\n\
\t\tReturn Game.GetPlayer()\n\
\tEndFunction\n\
EndProperty\n\
\n\
;/ bloc\n\
   de commentaire /;\n\
Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akRef)\n\
\tint[] t = new int[Count]\n\
\tWhile t.Length > 0 && !(akRef is Actor)\n\
\t\tt[0] += 1 ; increment → next\n\
\t\tDebug.Trace(\"x\" + \\\n\
\t\t\tt[0], aiSeverity = 2)\n\
\tEndWhile\n\
EndEvent\n\
Auto State Waiting\n\
\tFunction Go() global native\n\
EndState\n";

#[test]
fn the_full_script_reads_without_errors() {
    let parse = parse(FULL, Dialect::new(GameId::Fallout4));
    assert!(parse.ok(), "{:#?}", parse.errors());
    assert_eq!(parse.syntax().to_string(), FULL);
}

#[test]
fn every_prefix_of_the_script_rebuilds() {
    // The language server parses half-typed code: every truncation must give
    // a complete tree, without panicking.
    for (end, _) in FULL.char_indices() {
        let prefix = &FULL[..end];
        let parse = parse(prefix, Dialect::new(GameId::Fallout4));
        assert_eq!(parse.syntax().to_string(), prefix, "{end}-byte prefix");
    }
}

#[test]
fn every_removed_line_keeps_the_tree_complete() {
    // A missing line is a forgotten `EndIf` or `EndFunction`.
    let lines: Vec<&str> = FULL.split_inclusive('\n').collect();
    for skip in 0..lines.len() {
        let source: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != skip)
            .map(|(_, l)| *l)
            .collect();
        let parse = parse(&source, Dialect::new(GameId::Fallout4));
        assert_eq!(parse.syntax().to_string(), source, "line {skip} removed");
    }
}

#[test]
fn invalid_inputs_stay_lossless() {
    for source in [
        "",
        "\n\n",
        "ScriptName",
        "ScriptName T extends",
        "Function",
        "EndFunction EndIf EndWhile",
        "ScriptName T\nint Function (\n",
        "ScriptName T\nFunction F(int a = , ,)\nEndFunction",
        "ScriptName T\nFunction F()\n\tx = ((((\nEndFunction",
        "ScriptName T\nFunction F()\n\tx = a.\nEndFunction",
        "ScriptName T\nFunction F()\n\tFoo(a, \nEndFunction",
        "ScriptName T\nState S\n\tint x\nEndState",
        "\u{4}\u{7}@@@ ### \"unclosed é\n;/ open block",
    ] {
        let parse = parse(source, Dialect::default());
        assert_eq!(parse.syntax().to_string(), source, "{source:?}");
    }
}

#[test]
fn a_node_starts_at_its_first_token() {
    // Leading trivia goes to the parent: a function's range does not start on
    // the indentation or the comment preceding it.
    let source = "ScriptName T\n; avant\n\tFunction F()\nEndFunction\n";
    let root = parse(source, Dialect::default()).syntax();
    let function = root
        .children()
        .find(|n| n.kind() == SyntaxKind::FUNCTION_DECL)
        .expect("fonction");
    let start = usize::from(function.text_range().start());
    assert!(source[start..].starts_with("Function"));
}
