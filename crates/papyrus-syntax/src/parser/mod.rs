//! The parser: from tokens to a concrete syntax tree.
//!
//! Built after `rust-analyzer`. The grammar only sees significant tokens and
//! emits flat [`Event`]s; [`tree_builder`] replays them, reinjecting the
//! trivia, to obtain a `rowan` tree that reproduces the source byte for byte.
//!
//! # Guarantees
//!
//! - **Always a tree.** An invalid source gives a complete tree, dotted with
//!   [`SyntaxKind::ERROR_TREE`] nodes, and a list of errors.
//! - **Lossless.** `parse(src).syntax().to_string() == src`, whatever the
//!   input.
//! - **Local recovery.** An error does not spill over its line, nor a badly
//!   closed block over its declaration.

mod event;
mod grammar;
mod token_set;
mod tree_builder;

use std::cell::Cell;
use std::fmt;

use papyrus_dialect::Dialect;
use rowan::GreenNode;
use text_size::{TextRange, TextSize};

use crate::lexer::Token;
use crate::syntax_node::SyntaxNode;
use crate::{SyntaxKind, lex_with_dialect, reserved_later};

use event::Event;
use token_set::TokenSet;

/// A passage the parser could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
    range: TextRange,
}

impl ParseError {
    /// What is wrong, in one sentence.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where, in bytes in the source.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.range, self.message)
    }
}

/// The result of a parse: a tree, always, and the errors encountered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    /// The root of the tree, of kind [`SyntaxKind::SOURCE_FILE`].
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// The green tree, immutable and shareable between threads.
    pub fn green(&self) -> &GreenNode {
        &self.green
    }

    /// The errors, in source order.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// True if the source was read without errors.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Parses `source` for the `dialect` target.
///
/// This is the normal parsing entry point.
pub fn parse(source: &str, dialect: Dialect) -> Parse {
    let tokens = lex_with_dialect(source, dialect);
    parse_tokens(source, &tokens, dialect)
}

/// Parses tokens already produced by [`lex_with_dialect`].
///
/// # Panics
///
/// If `tokens` is not the tokenization of `source`.
pub fn parse_tokens(source: &str, tokens: &[Token], dialect: Dialect) -> Parse {
    let mut p = Parser::new(source, tokens, dialect);
    grammar::source_file(&mut p);
    let Parser { events, errors, .. } = p;
    let green = tree_builder::build(source, tokens, events);
    let mut errors = errors;
    errors.sort_by_key(|e| e.range.start());
    Parse { green, errors }
}

/// Parse steps allowed without consuming a token.
///
/// Every grammar loop must advance; if one spins in place, it is a bug, and
/// better to see it than to freeze the editor.
const FUEL: u32 = 256;

/// The parser state: position in the significant tokens, emitted events,
/// errors.
#[derive(Debug)]
pub(crate) struct Parser<'t> {
    /// The text, to explain errors.
    source: &'t str,
    /// Kinds of the significant tokens, trivia removed, followed by `EOF`.
    kinds: Vec<SyntaxKind>,
    /// The matching ranges, to locate errors.
    ranges: Vec<TextRange>,
    pos: usize,
    events: Vec<Event>,
    errors: Vec<ParseError>,
    dialect: Dialect,
    fuel: Cell<u32>,
}

impl<'t> Parser<'t> {
    fn new(source: &'t str, tokens: &[Token], dialect: Dialect) -> Parser<'t> {
        let end = TextSize::of(source);
        let (mut kinds, mut ranges): (Vec<_>, Vec<_>) = tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, t.range))
            .unzip();
        kinds.push(SyntaxKind::EOF);
        ranges.push(TextRange::empty(end));

        Parser {
            source,
            kinds,
            ranges,
            pos: 0,
            events: Vec::new(),
            errors: Vec::new(),
            dialect,
            fuel: Cell::new(FUEL),
        }
    }

    /// The target of the parse.
    pub(crate) fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// The `n`-th upcoming significant token, `EOF` past the end.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let fuel = self.fuel.get();
        assert!(fuel != 0, "parser bloqué à l'index {}", self.pos);
        self.fuel.set(fuel - 1);

        self.kinds
            .get(self.pos + n)
            .copied()
            .unwrap_or(SyntaxKind::EOF)
    }

    /// The current token.
    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// True if the current token is `kind`.
    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth(0) == kind
    }

    /// True if the current token belongs to `set`.
    pub(crate) fn at_ts(&self, set: TokenSet) -> bool {
        set.contains(self.nth(0))
    }

    /// Consumes the current token if it is `kind`.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if !self.at(kind) {
            return false;
        }
        self.do_bump(kind);
        true
    }

    /// Consumes the current token, which must be `kind`.
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        assert!(
            self.eat(kind),
            "attendu {kind:?}, trouvé {:?}",
            self.current()
        );
    }

    /// Consumes the current token, whatever it is. No effect on `EOF`.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current();
        if kind != SyntaxKind::EOF {
            self.do_bump(kind);
        }
    }

    /// Consumes `kind` or reports it missing, consuming nothing else.
    pub(crate) fn expect(&mut self, kind: SyntaxKind, what: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(format!("{what} attendu"));
        false
    }

    /// Reports an error on the current token.
    ///
    /// Only one per token: when an expression is missing, the line end that
    /// does not come is only a consequence, no need to report it too.
    pub(crate) fn error(&mut self, message: impl Into<String>) {
        let range = self.current_range();
        if self.errors.last().is_some_and(|e| e.range == range) {
            return;
        }
        let mut message = message.into();
        // `x is Actor` in Skyrim, `Guard g` in Fallout 4: the error falls on a
        // word the target does not reserve. Might as well say why.
        if self.current() == SyntaxKind::IDENT {
            let text = &self.source[range];
            if let Some((_, since)) = reserved_later(text, self.dialect) {
                message = format!(
                    "{message} — `{text}` n'est un mot-clé qu'à partir de {}",
                    since.display_name()
                );
            }
        }
        self.error_at(range, message);
    }

    /// Reports an error on a given range.
    pub(crate) fn error_at(&mut self, range: TextRange, message: impl Into<String>) {
        self.errors.push(ParseError {
            message: message.into(),
            range,
        });
    }

    /// Range of the current token.
    pub(crate) fn current_range(&self) -> TextRange {
        self.ranges[self.pos.min(self.ranges.len() - 1)]
    }

    /// Opens a node whose kind is set on completion.
    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.events.push(Event::tombstone());
        Marker { pos }
    }

    fn do_bump(&mut self, kind: SyntaxKind) {
        self.pos += 1;
        self.fuel.set(FUEL);
        self.events.push(Event::Token { kind });
    }
}

/// An open node, waiting for its kind.
#[derive(Debug)]
#[must_use = "un marqueur doit être complété ou abandonné"]
pub(crate) struct Marker {
    pos: u32,
}

impl Marker {
    /// Closes the node and gives it its kind.
    pub(crate) fn complete(self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        match &mut p.events[self.pos as usize] {
            Event::Start { kind: slot, .. } => *slot = kind,
            _ => unreachable!("un marqueur pointe toujours sur un Start"),
        }
        p.events.push(Event::Finish);
        CompletedMarker { pos: self.pos }
    }

    /// Gives up the node; its children move up into the parent.
    pub(crate) fn abandon(self, p: &mut Parser) {
        if self.pos as usize == p.events.len() - 1 {
            p.events.pop();
        }
    }
}

/// A closed node, which can still be wrapped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompletedMarker {
    pos: u32,
}

impl CompletedMarker {
    /// Opens a node that will contain this one.
    ///
    /// This is what allows reading `a + b` left to right: we find out that
    /// `a` is the left operand of an addition after finishing it.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        let parent = p.start();
        match &mut p.events[self.pos as usize] {
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some(parent.pos - self.pos);
            }
            _ => unreachable!("un marqueur pointe toujours sur un Start"),
        }
        parent
    }
}
