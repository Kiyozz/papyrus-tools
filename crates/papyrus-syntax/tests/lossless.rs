//! L'invariant fondateur : le lexage ne perd rien.
//!
//! `concat(tokens) == source`, octet pour octet, y compris sur une entrée que
//! le lexer ne comprend pas. Tout le reste — formateur, LSP — en dépend.

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::{SyntaxKind, lex, lex_with_dialect};

/// Vérifie que les tokens pavent la source sans trou ni recouvrement.
#[track_caller]
fn assert_lossless(source: &str) {
    let tokens = lex(source);

    let mut offset = 0usize;
    for token in &tokens {
        assert_eq!(
            usize::from(token.range.start()),
            offset,
            "trou ou recouvrement avant {:?} dans {source:?}",
            token.kind
        );
        offset = usize::from(token.range.end());
    }
    assert_eq!(
        offset,
        source.len(),
        "la fin de {source:?} n'est pas couverte"
    );

    let rebuilt: String = tokens.iter().map(|t| t.text(source)).collect();
    assert_eq!(rebuilt, source, "reconstruction différente de la source");
}

fn kinds(source: &str) -> Vec<SyntaxKind> {
    lex(source).into_iter().map(|t| t.kind).collect()
}

#[test]
fn le_lexage_est_lossless_sur_du_code_ordinaire() {
    let sources = [
        "",
        "\n",
        "\r\n",
        "ScriptName Foo extends ObjectReference\n",
        "Function Bar(int aiCount = 3) global native\n",
        "\tif (a == b) || !c\n\t\tReturn None\n\tEndIf\n",
        "float f = 1.5\nint h = 0xFF\nstring s = \"texte\"\n",
        "Actor a = akRef as Actor\n",
        "Foo:Bar:Baz obj = new Foo:Bar:Baz\n",
        "Guard myGuard ProtectsFunctionLogic\n",
        "\tLockGuard myGuard\n\t\tDoThing()\n\tEndLockGuard\n",
    ];
    for source in sources {
        assert_lossless(source);
    }
}

#[test]
fn le_lexage_est_lossless_sur_une_entree_invalide() {
    // Une source cassée doit rester reconstructible : le LSP travaille en
    // permanence sur du code à moitié écrit.
    let sources = [
        "\u{4}\u{7}",            // octets de contrôle
        "\"chaîne non fermée\n", // guillemet orphelin
        ";/ bloc non fermé",     // commentaire bloc ouvert
        "{docstring non fermée", // accolade orpheline
        "Function #!? EndFunction",
        "int x = @@@\n",
        "\\",                     // continuation sans fin de ligne
        "é à ü",                  // non-ASCII hors chaîne
        "\u{feff}ScriptName Foo", // BOM en tête
    ];
    for source in sources {
        assert_lossless(source);
    }
}

#[test]
fn le_commentaire_de_ligne_n_avale_pas_la_fin_de_ligne() {
    // Le piège classique. Si le commentaire emporte le retour à la ligne, le
    // parser perd le terminateur de la déclaration et toute la grammaire
    // orientée ligne s'effondre.
    assert_eq!(
        kinds("; commentaire\n"),
        [SyntaxKind::LINE_COMMENT, SyntaxKind::NEWLINE]
    );
    assert_eq!(
        kinds("; commentaire\r\n"),
        [SyntaxKind::LINE_COMMENT, SyntaxKind::NEWLINE]
    );
    assert_eq!(
        kinds(";\n"),
        [SyntaxKind::LINE_COMMENT, SyntaxKind::NEWLINE]
    );
}

#[test]
fn le_commentaire_bloc_prime_sur_le_commentaire_de_ligne() {
    // `;/ … /;` commence par `;` : sans arbitrage, le commentaire de ligne
    // gagnerait et le reste du bloc serait lexé comme du code.
    assert_eq!(kinds(";/ bloc /;"), [SyntaxKind::BLOCK_COMMENT]);
    assert_eq!(kinds(";//;"), [SyntaxKind::BLOCK_COMMENT]);
    assert_eq!(
        kinds(";/ sur\ndeux lignes /;\n"),
        [SyntaxKind::BLOCK_COMMENT, SyntaxKind::NEWLINE]
    );

    let source = ";/ a /; int x";
    assert_eq!(lex(source)[0].text(source), ";/ a /;");
}

#[test]
fn la_continuation_de_ligne_est_un_token_a_part() {
    // Trivia, mais le formateur doit pouvoir la retrouver pour ne pas la
    // supprimer.
    assert_eq!(
        kinds("Foo(a, \\\n\tb)"),
        [
            SyntaxKind::IDENT,
            SyntaxKind::L_PAREN,
            SyntaxKind::IDENT,
            SyntaxKind::COMMA,
            SyntaxKind::WHITESPACE,
            SyntaxKind::LINE_CONTINUATION,
            SyntaxKind::WHITESPACE,
            SyntaxKind::IDENT,
            SyntaxKind::R_PAREN,
        ]
    );
    assert!(SyntaxKind::LINE_CONTINUATION.is_trivia());
    // La fin de ligne, elle, porte du sens.
    assert!(!SyntaxKind::NEWLINE.is_trivia());
}

#[test]
fn les_litteraux_numeriques_sont_non_signes() {
    // `a-1` est une soustraction, pas un identifiant suivi d'un entier négatif.
    assert_eq!(
        kinds("a-1"),
        [SyntaxKind::IDENT, SyntaxKind::MINUS, SyntaxKind::INT_NUMBER]
    );
    assert_eq!(kinds("-1"), [SyntaxKind::MINUS, SyntaxKind::INT_NUMBER]);
}

#[test]
fn les_flottants_exigent_des_chiffres_des_deux_cotes() {
    assert_eq!(kinds("1.5"), [SyntaxKind::FLOAT_NUMBER]);
    // `1.` n'est pas un flottant : entier, puis accès membre.
    assert_eq!(kinds("1."), [SyntaxKind::INT_NUMBER, SyntaxKind::DOT]);
    // `.5` non plus.
    assert_eq!(kinds(".5"), [SyntaxKind::DOT, SyntaxKind::INT_NUMBER]);
    // Pas de notation exponentielle en Papyrus.
    assert_eq!(
        kinds("1e5"),
        [SyntaxKind::INT_NUMBER, SyntaxKind::IDENT],
        "`1e5` doit se lexer en entier suivi d'un identifiant"
    );
}

#[test]
fn les_hexadecimaux_priment_sur_les_entiers() {
    assert_eq!(kinds("0xDEADBEEF"), [SyntaxKind::HEX_NUMBER]);
    assert_eq!(kinds("0XFF"), [SyntaxKind::HEX_NUMBER]);
    // `0x` sans chiffre : entier `0` puis identifiant `x`.
    assert_eq!(kinds("0x"), [SyntaxKind::INT_NUMBER, SyntaxKind::IDENT]);
}

#[test]
fn une_chaine_non_fermee_s_arrete_a_la_fin_de_ligne() {
    // Sans cette borne, une faute de frappe emporterait le reste du fichier —
    // et l'éditeur afficherait tout en couleur de chaîne.
    let source = "\"oups\nint x = 1\n";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, SyntaxKind::STRING);
    assert_eq!(tokens[0].text(source), "\"oups");
    assert_eq!(tokens[1].kind, SyntaxKind::NEWLINE);
    assert_lossless(source);
}

#[test]
fn les_echappements_de_chaine_sont_respectes() {
    let source = r#""un \" guillemet" apres"#;
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, SyntaxKind::STRING);
    assert_eq!(tokens[0].text(source), r#""un \" guillemet""#);
}

#[test]
fn la_docstring_peut_courir_sur_plusieurs_lignes() {
    let source = "{doc\nsur deux lignes}\n";
    let tokens = lex(source);
    assert_eq!(tokens[0].kind, SyntaxKind::DOC_COMMENT);
    assert_eq!(tokens[0].text(source), "{doc\nsur deux lignes}");
    assert_eq!(tokens[1].kind, SyntaxKind::NEWLINE);
}

#[test]
fn les_octets_invalides_consecutifs_sont_fusionnes() {
    // Un diagnostic par octet serait illisible.
    let tokens = lex("###");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, SyntaxKind::ERROR);
    assert_eq!(usize::from(tokens[0].range.len()), 3);
}

#[test]
fn le_dialecte_ne_change_jamais_le_decoupage() {
    // La promotion en mots-clés réétiquette des tokens, elle n'en déplace
    // aucun : le CST doit être le même quelle que soit la cible.
    let source = "int Var = 1\nGuard g\nif x is Actor\n";
    let reference: Vec<_> = lex(source).into_iter().map(|t| t.range).collect();

    for game in [
        GameId::Skyrim,
        GameId::Fallout4,
        GameId::Fallout76,
        GameId::Starfield,
    ] {
        let ranges: Vec<_> = lex_with_dialect(source, Dialect::new(game))
            .into_iter()
            .map(|t| t.range)
            .collect();
        assert_eq!(ranges, reference, "découpage différent pour {game}");
    }
}
