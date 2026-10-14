//! The formatter, on chosen cases.

use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatError, FormatOptions, Indent, format};

#[track_caller]
fn fmt(source: &str) -> String {
    let out = match format(
        source,
        Dialect::new(GameId::Fallout4),
        &FormatOptions::default(),
    ) {
        Ok(out) => out,
        Err(FormatError::Syntax(errors)) => {
            let shown: Vec<String> = errors
                .iter()
                .map(|e| format!("{} on {:?}", e.message(), &source[e.range()]))
                .collect();
            panic!("syntax errors: {shown:#?}")
        }
        Err(e) => panic!("{e}"),
    };
    let again = format(
        &out,
        Dialect::new(GameId::Fallout4),
        &FormatOptions::default(),
    )
    .expect("the result must format again");
    assert_eq!(again, out, "formatting is not idempotent");
    out
}

/// A carelessly written script, as found in the wild.
const DESORDRE: &str = "\
ScriptName   Foo:Bar   Extends Quest   CONDITIONAL\r
{Documentation du script}\r
Import Utility\r
\r
\r
\r
Int Property Count=3 AUTO Const   ; compteur   \r
Actor Property Who Hidden\r
Actor Function GET()\r
Return Game.GetPlayer( )\r
EndFunction\r
ENDPROPERTY\r
Auto State Default\r
Event OnInit()\r
; avant la boucle\r
Int[ ] t = New Int[ Count ]\r
While t.Length>0&&!(akRef Is Actor)\r
t[0]+=1\r
If x==-1\r
Debug.Trace(\"x\"+ \\\r
t[0] , aiSeverity=2)\r
ElseIf (x)\r
; branche vide\r
Else\r
Return\r
EndIf\r
EndWhile\r
\r
EndEvent\r
EndState\r
\r
";

#[test]
fn a_messy_script() {
    insta::assert_snapshot!(fmt(DESORDRE));
}

#[test]
fn keywords_used_as_names_keep_their_case() {
    let out = fmt("ScriptName T\nAuto State Default\nEndState\nGroup Mandatory\nEndGroup\n");
    assert!(out.contains("auto state Default"), "{out}");
    assert!(out.contains("group Mandatory"), "{out}");
}

#[test]
fn identifiers_never_change() {
    let out = fmt("ScriptName MyScript\nACTOR Property MyActor Auto\n");
    assert_eq!(out, "scriptname MyScript\nACTOR property MyActor auto\n");
}

#[test]
fn the_bom_is_kept_but_never_added() {
    assert!(fmt("\u{feff}ScriptName T\n").starts_with("\u{feff}scriptname"));
    assert!(!fmt("ScriptName T\n").starts_with('\u{feff}'));
}

#[test]
fn an_invalid_source_is_not_formatted() {
    let result = format(
        "ScriptName T\nFunction F(\n",
        Dialect::default(),
        &FormatOptions::default(),
    );
    assert!(matches!(result, Err(FormatError::Syntax(_))));
}

#[test]
fn the_indent_is_configurable() {
    let source = "ScriptName T\nFunction F()\nIf x\nReturn\nEndIf\nEndFunction\n";
    let tabs = format(
        source,
        Dialect::default(),
        &FormatOptions {
            indent: Indent::Tabs,
        },
    )
    .unwrap();
    assert_eq!(
        tabs,
        "scriptname T\nfunction F()\n\tif x\n\t\treturn\n\tendif\nendfunction\n"
    );
}

#[test]
fn the_file_ends_with_a_single_line_end() {
    assert_eq!(fmt("ScriptName T"), "scriptname T\n");
    assert_eq!(fmt("\n\nScriptName T\n\n\n"), "scriptname T\n");
}

#[test]
fn default_values_keep_their_case() {
    // Compilers compare a default with the overridden function's as text:
    // `False` against the parent's `false` does not compile.
    assert_eq!(
        fmt(
            "ScriptName T\nFunction F(Bool a = False, Form f = None, Bool b = TRUE)\nBool x = False\nEndFunction\n"
        ),
        "scriptname T\nfunction F(bool a = False, Form f = None, bool b = TRUE)\n  bool x = false\nendfunction\n"
    );
}
