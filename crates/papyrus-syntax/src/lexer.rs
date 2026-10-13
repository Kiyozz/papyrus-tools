//! Splitting text into tokens.
//!
//! The lexer does not know the target: it recognizes the superset of all
//! games and produces [`SyntaxKind::IDENT`] for every identifier, reserved
//! words included. [`crate::token_source`] then promotes identifiers to
//! keywords according to the dialect. Lexing thus stays a pure function of
//! the text, shareable between targets and cacheable.
//!
//! # Invariant
//!
//! Concatenating the tokens reproduces the source byte for byte, even on
//! invalid input. Anything the lexer does not recognize becomes
//! [`SyntaxKind::ERROR`] instead of being dropped. Without this invariant,
//! neither the formatter nor the language server can work.

use logos::{Lexer, Logos};
use text_size::{TextRange, TextSize};

use crate::SyntaxKind;

/// A token, and the part of the text it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Kind of the token.
    pub kind: SyntaxKind,
    /// Range covered in the source text, in bytes.
    pub range: TextRange,
}

impl Token {
    /// Text covered by this token.
    ///
    /// # Panics
    ///
    /// If `source` is not the text this token comes from.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range]
    }
}

/// Splits `text` into tokens, regardless of any dialect.
///
/// # Panics
///
/// If `text` exceeds 4 GiB — the limit of `TextSize`. No Papyrus script
/// comes close.
pub fn lex(text: &str) -> Vec<Token> {
    assert!(text.len() <= u32::MAX as usize, "source larger than 4 GiB");

    let mut tokens: Vec<Token> = Vec::new();
    let mut lexer = Raw::lexer(text);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        let kind = match result {
            Ok(raw) => raw.kind(),
            Err(()) => SyntaxKind::ERROR,
        };

        // Unrecognized bytes often come in runs. Merging them avoids drowning
        // the user in one diagnostic per byte.
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

/// Comment shape recognized by [`comment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    Line,
    Block,
}

/// Raw tokens, before any dialect is taken into account.
///
/// Keywords are not among them: they are [`Raw::Ident`] until promotion.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
enum Raw {
    // ── Trivia ────────────────────────────────────────────────────────────
    #[regex(r"[ \t\x0C]+")]
    // A UTF-8 BOM is a zero-width no-break space: treating it as trivia keeps
    // it in the tree without disturbing the grammar. It matters, since the
    // Skyrim compiler uses it to recognize UTF-8.
    #[token("\u{feff}")]
    Whitespace,

    /// Plain `;` or `;/ … /;` — the distinction is made in the callback,
    /// otherwise the line comment would swallow the block opener.
    #[token(";", comment)]
    Comment(CommentKind),

    #[token("{", doc_comment)]
    DocComment,

    /// `\` followed by the line end. Trivia the formatter must never remove:
    /// it carries the structure of the logical line.
    #[regex(r"\\[ \t]*\r?\n")]
    LineContinuation,

    // ── Line end, significant ─────────────────────────────────────────────
    #[regex(r"\r\n|\r|\n")]
    Newline,

    // ── Literals ──────────────────────────────────────────────────────────
    #[regex(r"0[xX][0-9a-fA-F]+", priority = 4)]
    HexNumber,

    /// Digits are required on both sides of the dot: `1.` lexes as an integer
    /// followed by a dot, and `.5` as a dot followed by an integer.
    #[regex(r"[0-9]+\.[0-9]+", priority = 3)]
    FloatNumber,

    /// Always unsigned: in `a-1`, the `-` is an operator.
    #[regex(r"[0-9]+", priority = 2)]
    IntNumber,

    #[token("\"", string)]
    String,

    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Ident,

    // ── Punctuation ───────────────────────────────────────────────────────
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

/// Consumes a comment, starting from the already read `;`.
///
/// A `;/` opens a block running up to `/;`, even across lines. Otherwise the
/// comment stops **before** the line end: that is a token of its own, which
/// the parser expects.
fn comment(lex: &mut Lexer<'_, Raw>) -> CommentKind {
    let rest = lex.remainder();

    if let Some(after_slash) = rest.strip_prefix('/') {
        match after_slash.find("/;") {
            Some(offset) => lex.bump(1 + offset + 2),
            // Unclosed block: swallow the rest of the file rather than relex
            // comment text as code.
            None => lex.bump(rest.len()),
        }
        CommentKind::Block
    } else {
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        lex.bump(end);
        CommentKind::Line
    }
}

/// Consumes a `{ … }` docstring, starting from the already read brace.
///
/// Braces do not nest in Papyrus: the first closing one ends it.
fn doc_comment(lex: &mut Lexer<'_, Raw>) {
    let rest = lex.remainder();
    match rest.find('}') {
        Some(offset) => lex.bump(offset + 1),
        None => lex.bump(rest.len()),
    }
}

/// Consumes a string, starting from the already read quote.
///
/// An unclosed string stops at the end of the line: without this bound, a
/// single typo would carry away the rest of the file.
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
