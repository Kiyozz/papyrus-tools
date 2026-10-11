//! La forme de l'arbre.
//!
//! Les snapshots montrent les nœuds et les tokens significatifs ; la trivia
//! est omise pour la lisibilité, sa conservation étant vérifiée à part. `⏎`
//! marque une fin de ligne, qui est un token à part entière.

use std::fmt::Write;

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::{NodeOrToken, Parse, SyntaxKind, SyntaxNode, parse};

/// L'arbre, une ligne par élément, suivi des erreurs.
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
            "erreur {start}..{end} {:?} : {}",
            &source[start..end],
            error.message()
        )
        .unwrap();
    }
    out
}

/// Parse pour `game`, vérifie le round-trip, rend l'arbre.
#[track_caller]
fn tree(source: &str, game: GameId) -> String {
    let parse = parse(source, Dialect::new(game));
    assert_eq!(parse.syntax().to_string(), source, "round-trip rompu");
    render(&parse, source)
}

/// Comme [`tree`], dans une fonction, pour tester des instructions.
#[track_caller]
fn body(stmts: &str, game: GameId) -> String {
    tree(
        &format!("ScriptName T\nFunction F()\n{stmts}\nEndFunction\n"),
        game,
    )
}

#[test]
fn en_tete_complet() {
    insta::assert_snapshot!(tree(
        "ScriptName DLC01:Foo extends ObjectReference Hidden Const\nimport Utility\n",
        GameId::Fallout4
    ));
}

#[test]
fn precedence_des_operateurs() {
    // `*` lie plus fort que `+`, `&&` plus fort que `||`, et l'unaire
    // enveloppe le cast.
    insta::assert_snapshot!(body(
        "x = a + b * c\ny = a || b && c\nz = -w as Float",
        GameId::Skyrim
    ));
}

#[test]
fn declaration_affectation_ou_appel() {
    // Les trois commencent par un identifiant ; seule la première est un type
    // suivi d'un nom.
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
fn property_courte_et_longue() {
    // La forme se décide aux flags, pas à leur position.
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
fn fonction_native_sans_corps() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nint Function Count() global native\nFunction Next()\nEndFunction\n",
        GameId::Skyrim
    ));
}

#[test]
fn evenement_distant() {
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
fn verrous_starfield() {
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
fn appel_statique_qualifie() {
    insta::assert_snapshot!(body(
        "DLC04:Manager.GetScript().Attach(akRef as Actor)",
        GameId::Fallout4
    ));
}

// ── Dialectes ─────────────────────────────────────────────────────────────

#[test]
fn un_script_skyrim_peut_nommer_une_variable_var() {
    let parse = parse(
        "ScriptName T\nFunction F()\n\tint Var = 1\n\tVar = 2\nEndFunction\n",
        Dialect::new(GameId::Skyrim),
    );
    assert!(parse.ok(), "{:?}", parse.errors());
}

#[test]
fn is_est_refuse_en_skyrim_et_dit_pourquoi() {
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
            .contains("`is` n'est un mot-clé qu'à partir de Fallout 4"),
        "{}",
        skyrim.errors()[0]
    );
    assert!(parse(source, Dialect::new(GameId::Fallout4)).ok());
}

#[test]
fn les_evenements_distants_sont_signales_en_skyrim() {
    let source = "ScriptName T
Event Actor.OnDeath(Actor akSender, Actor akKiller)
EndEvent
";
    let skyrim = parse(source, Dialect::new(GameId::Skyrim));
    assert_eq!(skyrim.errors().len(), 1, "{:?}", skyrim.errors());
    assert!(skyrim.errors()[0].message().contains("événements distants"));
    assert_eq!(skyrim.syntax().to_string(), source);

    assert!(parse(source, Dialect::new(GameId::Fallout4)).ok());
}

#[test]
fn les_namespaces_sont_signales_en_skyrim() {
    // Le second point d'intervention du dialecte : la construction est lue,
    // l'arbre est complet, et la cible est dite incompatible.
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

// ── Récupération ──────────────────────────────────────────────────────────

#[test]
fn une_ligne_invalide_reste_sur_sa_ligne() {
    insta::assert_snapshot!(body("x = 1 2 3\ny = )\nz = 3", GameId::Skyrim));
}

#[test]
fn un_endif_oublie_n_emporte_pas_la_fonction_suivante() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nFunction A()\n\tIf x\n\t\ty()\nEndFunction\nFunction B()\nEndFunction\n",
        GameId::Skyrim
    ));
}

#[test]
fn un_endproperty_oublie_n_emporte_pas_la_suite() {
    insta::assert_snapshot!(tree(
        "ScriptName T\nint Property P\nint Property Q Auto\n",
        GameId::Skyrim
    ));
}

#[test]
fn un_script_sans_en_tete_est_signale() {
    let parse = parse("Function F()\nEndFunction\n", Dialect::default());
    assert_eq!(parse.errors().len(), 1);
    assert!(parse.errors()[0].message().contains("ScriptName"));
}

// ── Sans perte ────────────────────────────────────────────────────────────

/// Un script qui couvre l'essentiel de la grammaire, commentaires compris.
const COMPLET: &str = "\u{feff}ScriptName Foo:Bar extends Quest Conditional ; en-tête\n\
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
\t\tt[0] += 1 ; incrément\n\
\t\tDebug.Trace(\"x\" + \\\n\
\t\t\tt[0], aiSeverity = 2)\n\
\tEndWhile\n\
EndEvent\n\
Auto State Waiting\n\
\tFunction Go() global native\n\
EndState\n";

#[test]
fn le_script_complet_se_lit_sans_erreur() {
    let parse = parse(COMPLET, Dialect::new(GameId::Fallout4));
    assert!(parse.ok(), "{:#?}", parse.errors());
    assert_eq!(parse.syntax().to_string(), COMPLET);
}

#[test]
fn chaque_prefixe_du_script_se_reconstruit() {
    // Le LSP parse du code à moitié tapé : chaque troncature doit donner un
    // arbre complet, sans panique.
    for (end, _) in COMPLET.char_indices() {
        let prefix = &COMPLET[..end];
        let parse = parse(prefix, Dialect::new(GameId::Fallout4));
        assert_eq!(
            parse.syntax().to_string(),
            prefix,
            "préfixe de {end} octets"
        );
    }
}

#[test]
fn chaque_ligne_retiree_garde_l_arbre_complet() {
    // Une ligne manquante, c'est un `EndIf` ou un `EndFunction` oublié.
    let lines: Vec<&str> = COMPLET.split_inclusive('\n').collect();
    for skip in 0..lines.len() {
        let source: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != skip)
            .map(|(_, l)| *l)
            .collect();
        let parse = parse(&source, Dialect::new(GameId::Fallout4));
        assert_eq!(parse.syntax().to_string(), source, "ligne {skip} retirée");
    }
}

#[test]
fn les_entrees_invalides_restent_sans_perte() {
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
        "\u{4}\u{7}@@@ ### \"non fermée\n;/ bloc ouvert",
    ] {
        let parse = parse(source, Dialect::default());
        assert_eq!(parse.syntax().to_string(), source, "{source:?}");
    }
}

#[test]
fn un_noeud_commence_sur_son_premier_token() {
    // La trivia de tête va au parent : la plage d'une fonction ne commence
    // pas sur l'indentation ou le commentaire qui la précède.
    let source = "ScriptName T\n; avant\n\tFunction F()\nEndFunction\n";
    let root = parse(source, Dialect::default()).syntax();
    let function = root
        .children()
        .find(|n| n.kind() == SyntaxKind::FUNCTION_DECL)
        .expect("fonction");
    let start = usize::from(function.text_range().start());
    assert!(source[start..].starts_with("Function"));
}
