//! The founding invariant: lexing loses nothing.
//!
//! `concat(tokens) == source`, byte for byte, even on input the lexer does
//! not understand. Everything else — formatter, language server — depends
//! on it.

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::{SyntaxKind, lex, lex_with_dialect};

/// Checks that the tokens tile the source without gaps or overlaps.
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
    assert_eq!(rebuilt, source, "rebuilt text differs from the source");
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
    // A broken source must stay rebuildable: the language server constantly
    // works on half-written code.
    let sources = [
        "\u{4}\u{7}",            // control bytes
        "\"chaîne non fermée\n", // stray quote
        ";/ bloc non fermé",     // open block comment
        "{docstring non fermée", // stray brace
        "Function #!? EndFunction",
        "int x = @@@\n",
        "\\",                     // continuation without line end
        "é à ü",                  // non-ASCII outside a string
        "\u{feff}ScriptName Foo", // leading BOM
    ];
    for source in sources {
        assert_lossless(source);
    }
}

#[test]
fn le_commentaire_de_ligne_n_avale_pas_la_fin_de_ligne() {
    // The classic trap. If the comment takes the line break, the parser
    // loses the declaration's terminator and the whole line-oriented grammar
    // collapses.
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
    // `;/ … /;` starts with `;`: without arbitration, the line comment would
    // win and the rest of the block would be lexed as code.
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
    // Trivia, but the formatter must be able to find it so as not to remove
    // it.
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
    // The line end, for its part, carries meaning.
    assert!(!SyntaxKind::NEWLINE.is_trivia());
}

#[test]
fn les_litteraux_numeriques_sont_non_signes() {
    // `a-1` is a subtraction, not an identifier followed by a negative integer.
    assert_eq!(
        kinds("a-1"),
        [SyntaxKind::IDENT, SyntaxKind::MINUS, SyntaxKind::INT_NUMBER]
    );
    assert_eq!(kinds("-1"), [SyntaxKind::MINUS, SyntaxKind::INT_NUMBER]);
}

#[test]
fn les_flottants_exigent_des_chiffres_des_deux_cotes() {
    assert_eq!(kinds("1.5"), [SyntaxKind::FLOAT_NUMBER]);
    // `1.` is not a float: integer, then member access.
    assert_eq!(kinds("1."), [SyntaxKind::INT_NUMBER, SyntaxKind::DOT]);
    // Nor is `.5`.
    assert_eq!(kinds(".5"), [SyntaxKind::DOT, SyntaxKind::INT_NUMBER]);
    // No exponent notation in Papyrus.
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
    // `0x` without a digit: integer `0` then identifier `x`.
    assert_eq!(kinds("0x"), [SyntaxKind::INT_NUMBER, SyntaxKind::IDENT]);
}

#[test]
fn une_chaine_non_fermee_s_arrete_a_la_fin_de_ligne() {
    // Without this bound, a typo would carry away the rest of the file — and
    // the editor would show everything in string color.
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
    // One diagnostic per byte would be unreadable.
    let tokens = lex("###");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, SyntaxKind::ERROR);
    assert_eq!(usize::from(tokens[0].range.len()), 3);
}

#[test]
fn le_dialecte_ne_change_jamais_le_decoupage() {
    // Keyword promotion relabels tokens, it moves none: the CST must be the
    // same whatever the target.
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
        assert_eq!(ranges, reference, "different split for {game}");
    }
}
