//! Découpage du texte en tokens.
//!
//! Le lexer ne connaît pas la cible : il reconnaît le sur-ensemble de tous les
//! jeux et produit [`SyntaxKind::IDENT`] pour tout identifiant, mots réservés
//! compris. C'est [`crate::token_source`] qui promeut ensuite les identifiants
//! en mots-clés selon le dialecte. Le lexage reste ainsi une fonction pure du
//! texte, donc partageable entre cibles et cacheable.
//!
//! # Invariant
//!
//! La concaténation des tokens reproduit la source octet pour octet, y compris
//! sur une entrée invalide. Tout ce que le lexer ne reconnaît pas devient
//! [`SyntaxKind::ERROR`] plutôt que d'être ignoré. Sans cet invariant, ni le
//! formateur ni le serveur LSP ne peuvent fonctionner.

use logos::{Lexer, Logos};
use text_size::{TextRange, TextSize};

use crate::SyntaxKind;

/// Un token, et l'endroit du texte qu'il occupe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Nature du token.
    pub kind: SyntaxKind,
    /// Plage occupée dans le texte source, en octets.
    pub range: TextRange,
}

impl Token {
    /// Texte couvert par ce token.
    ///
    /// # Panics
    ///
    /// Si `source` n'est pas le texte dont ce token est issu.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range]
    }
}

/// Découpe `text` en tokens, sans tenir compte d'un quelconque dialecte.
///
/// # Panics
///
/// Si `text` dépasse 4 Gio — la limite de `TextSize`. Aucun script Papyrus
/// n'en approche.
pub fn lex(text: &str) -> Vec<Token> {
    assert!(text.len() <= u32::MAX as usize, "source de plus de 4 Gio");

    let mut tokens: Vec<Token> = Vec::new();
    let mut lexer = Raw::lexer(text);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let kind = match result {
            Ok(raw) => raw.kind(),
            Err(()) => SyntaxKind::ERROR,
        };

        // Les octets non reconnus se suivent souvent. Les fusionner évite de
        // noyer l'utilisateur sous un diagnostic par octet.
        if kind == SyntaxKind::ERROR {
            if let Some(last) = tokens.last_mut() {
                if last.kind == SyntaxKind::ERROR && usize::from(last.range.end()) == span.start {
                    last.range =
                        TextRange::new(last.range.start(), TextSize::from(span.end as u32));
                    continue;
                }
            }
        }

        tokens.push(Token {
            kind,
            range: TextRange::new(
                TextSize::from(span.start as u32),
                TextSize::from(span.end as u32),
            ),
        });
    }

    tokens
}

/// Forme du commentaire reconnue par [`comment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    Line,
    Block,
}

/// Tokens bruts, avant toute prise en compte du dialecte.
///
/// Les mots-clés n'y figurent pas : ce sont des [`Raw::Ident`] jusqu'à la
/// promotion.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
enum Raw {
    // ── Trivia ────────────────────────────────────────────────────────────
    #[regex(r"[ \t\x0C]+")]
    // Un BOM UTF-8 est un espace insécable de largeur nulle : le traiter comme
    // de la trivia le conserve dans l'arbre sans perturber la grammaire. Il
    // compte, puisque le compilateur de Skyrim s'en sert pour reconnaître
    // l'UTF-8.
    #[token("\u{feff}")]
    Whitespace,

    /// `;` ordinaire ou `;/ … /;` — la distinction se fait dans le callback,
    /// faute de quoi le commentaire de ligne avalerait l'ouverture du bloc.
    #[token(";", comment)]
    Comment(CommentKind),

    #[token("{", doc_comment)]
    DocComment,

    /// `\` suivi de la fin de ligne. Trivia que le formateur ne doit jamais
    /// supprimer : elle porte la structure de la ligne logique.
    #[regex(r"\\[ \t]*\r?\n")]
    LineContinuation,

    // ── Fin de ligne, significative ───────────────────────────────────────
    #[regex(r"\r\n|\r|\n")]
    Newline,

    // ── Littéraux ─────────────────────────────────────────────────────────
    #[regex(r"0[xX][0-9a-fA-F]+", priority = 4)]
    HexNumber,

    /// Des chiffres des deux côtés du point sont exigés : `1.` se lexe en
    /// entier suivi d'un point, et `.5` en point suivi d'un entier.
    #[regex(r"[0-9]+\.[0-9]+", priority = 3)]
    FloatNumber,

    /// Toujours non signé : dans `a-1`, le `-` est un opérateur.
    #[regex(r"[0-9]+", priority = 2)]
    IntNumber,

    #[token("\"", string)]
    String,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,

    // ── Ponctuation ───────────────────────────────────────────────────────
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("==")]
    Eq2,
    #[token("!=")]
    Neq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&&")]
    Amp2,
    #[token("||")]
    Pipe2,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("=")]
    Eq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("!")]
    Bang,
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBrack,
    #[token("]")]
    RBrack,
    #[token(":")]
    Colon,
}

impl Raw {
    fn kind(self) -> SyntaxKind {
        match self {
            Raw::Whitespace => SyntaxKind::WHITESPACE,
            Raw::Comment(CommentKind::Line) => SyntaxKind::LINE_COMMENT,
            Raw::Comment(CommentKind::Block) => SyntaxKind::BLOCK_COMMENT,
            Raw::DocComment => SyntaxKind::DOC_COMMENT,
            Raw::LineContinuation => SyntaxKind::LINE_CONTINUATION,
            Raw::Newline => SyntaxKind::NEWLINE,
            Raw::HexNumber => SyntaxKind::HEX_NUMBER,
            Raw::FloatNumber => SyntaxKind::FLOAT_NUMBER,
            Raw::IntNumber => SyntaxKind::INT_NUMBER,
            Raw::String => SyntaxKind::STRING,
            Raw::Ident => SyntaxKind::IDENT,
            Raw::PlusEq => SyntaxKind::PLUS_EQ,
            Raw::MinusEq => SyntaxKind::MINUS_EQ,
            Raw::StarEq => SyntaxKind::STAR_EQ,
            Raw::SlashEq => SyntaxKind::SLASH_EQ,
            Raw::PercentEq => SyntaxKind::PERCENT_EQ,
            Raw::Eq2 => SyntaxKind::EQ2,
            Raw::Neq => SyntaxKind::NEQ,
            Raw::LtEq => SyntaxKind::LTEQ,
            Raw::GtEq => SyntaxKind::GTEQ,
            Raw::Amp2 => SyntaxKind::AMP2,
            Raw::Pipe2 => SyntaxKind::PIPE2,
            Raw::Plus => SyntaxKind::PLUS,
            Raw::Minus => SyntaxKind::MINUS,
            Raw::Star => SyntaxKind::STAR,
            Raw::Slash => SyntaxKind::SLASH,
            Raw::Percent => SyntaxKind::PERCENT,
            Raw::Eq => SyntaxKind::EQ,
            Raw::Lt => SyntaxKind::LT,
            Raw::Gt => SyntaxKind::GT,
            Raw::Bang => SyntaxKind::BANG,
            Raw::Dot => SyntaxKind::DOT,
            Raw::Comma => SyntaxKind::COMMA,
            Raw::LParen => SyntaxKind::L_PAREN,
            Raw::RParen => SyntaxKind::R_PAREN,
            Raw::LBrack => SyntaxKind::L_BRACK,
            Raw::RBrack => SyntaxKind::R_BRACK,
            Raw::Colon => SyntaxKind::COLON,
        }
    }
}

/// Consomme un commentaire, à partir du `;` déjà lu.
///
/// Un `;/` ouvre un bloc qui court jusqu'à `/;`, même sur plusieurs lignes.
/// Sinon le commentaire s'arrête **avant** la fin de ligne : celle-ci est un
/// token à part entière, que le parser attend.
fn comment(lex: &mut Lexer<'_, Raw>) -> CommentKind {
    let rest = lex.remainder();

    if let Some(after_slash) = rest.strip_prefix('/') {
        match after_slash.find("/;") {
            Some(offset) => lex.bump(1 + offset + 2),
            // Bloc non refermé : on avale la fin du fichier plutôt que de
            // relexer du texte de commentaire comme du code.
            None => lex.bump(rest.len()),
        }
        CommentKind::Block
    } else {
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        lex.bump(end);
        CommentKind::Line
    }
}

/// Consomme une docstring `{ … }`, à partir de l'accolade déjà lue.
///
/// Les accolades ne s'imbriquent pas en Papyrus : la première fermante termine.
fn doc_comment(lex: &mut Lexer<'_, Raw>) {
    let rest = lex.remainder();
    match rest.find('}') {
        Some(offset) => lex.bump(offset + 1),
        None => lex.bump(rest.len()),
    }
}

/// Consomme une chaîne, à partir du guillemet déjà lu.
///
/// Une chaîne non refermée s'arrête à la fin de la ligne : sans cette borne,
/// une simple faute de frappe emporterait le reste du fichier.
fn string(lex: &mut Lexer<'_, Raw>) {
    let rest = lex.remainder();
    let mut escaped = false;
    let mut end = rest.len();

    for (offset, ch) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => {
                end = offset + 1;
                break;
            }
            '\r' | '\n' => {
                end = offset;
                break;
            }
            _ => {}
        }
    }

    lex.bump(end);
}
