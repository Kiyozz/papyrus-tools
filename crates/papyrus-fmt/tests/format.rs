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
                .map(|e| format!("{} sur {:?}", e.message(), &source[e.range()]))
                .collect();
            panic!("erreurs de syntaxe : {shown:#?}")
        }
        Err(e) => panic!("{e}"),
    };
    let again = format(
        &out,
        Dialect::new(GameId::Fallout4),
        &FormatOptions::default(),
    )
    .expect("le résultat doit se reformater");
    assert_eq!(again, out, "le formatage n'est pas idempotent");
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
fn un_script_en_desordre() {
    insta::assert_snapshot!(fmt(DESORDRE));
}

#[test]
fn les_mots_cles_servant_de_noms_gardent_leur_casse() {
    let out = fmt("ScriptName T\nAuto State Default\nEndState\nGroup Mandatory\nEndGroup\n");
    assert!(out.contains("auto state Default"), "{out}");
    assert!(out.contains("group Mandatory"), "{out}");
}

#[test]
fn les_identifiants_ne_changent_jamais() {
    let out = fmt("ScriptName MyScript\nACTOR Property MyActor Auto\n");
    assert_eq!(out, "scriptname MyScript\nACTOR property MyActor auto\n");
}

#[test]
fn le_bom_est_conserve_mais_jamais_ajoute() {
    assert!(fmt("\u{feff}ScriptName T\n").starts_with("\u{feff}scriptname"));
    assert!(!fmt("ScriptName T\n").starts_with('\u{feff}'));
}

#[test]
fn une_source_invalide_n_est_pas_formatee() {
    let result = format(
        "ScriptName T\nFunction F(\n",
        Dialect::default(),
        &FormatOptions::default(),
    );
    assert!(matches!(result, Err(FormatError::Syntax(_))));
}

#[test]
fn l_indentation_se_regle() {
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
fn le_fichier_finit_par_une_seule_fin_de_ligne() {
    assert_eq!(fmt("ScriptName T"), "scriptname T\n");
    assert_eq!(fmt("\n\nScriptName T\n\n\n"), "scriptname T\n");
}
