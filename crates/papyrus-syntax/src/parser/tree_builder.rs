//! From events to the `rowan` tree.
//!
//! The parser only saw the significant tokens; this module replays its
//! events over the full list and reinserts the trivia along the way.
//!
//! # Where trivia goes
//!
//! Into the **parent** node, before the child opens. A node thus starts
//! exactly on its first significant token and ends on its last, which is
//! what diagnostics and editor selection want. Trivia following the last
//! token of the file goes into the root.

use std::mem;

use rowan::{GreenNode, GreenNodeBuilder, Language};

use super::event::Event;
use crate::SyntaxKind;
use crate::lexer::Token;
use crate::syntax_node::Papyrus;

/// Builds the tree from the parser's events.
pub(super) fn build(source: &str, tokens: &[Token], mut events: Vec<Event>) -> GreenNode {
    let mut builder = TreeBuilder {
        source,
        tokens,
        pos: 0,
        depth: 0,
        inner: GreenNodeBuilder::new(),
    };

    let mut parents = Vec::new();
    for i in 0..events.len() {
        match mem::replace(&mut events[i], Event::tombstone()) {
            Event::Start {
                kind,
                forward_parent,
            } => {
                // Walks up the chain of parents added afterwards by
                // `precede`, then opens them from outermost to innermost.
                parents.push(kind);
                let mut index = i;
                let mut next = forward_parent;
                while let Some(offset) = next {
                    index += offset as usize;
                    next = match mem::replace(&mut events[index], Event::tombstone()) {
                        Event::Start {
                            kind,
                            forward_parent,
                        } => {
                            parents.push(kind);
                            forward_parent
                        }
                        _ => unreachable!("forward_parent always points to a Start"),
                    };
                }
                for kind in parents.drain(..).rev() {
                    if kind != SyntaxKind::TOMBSTONE {
                        builder.start_node(kind);
                    }
                }
            }
            Event::Token { kind } => builder.token(kind),
            Event::Finish => builder.finish_node(),
        }
    }

    builder.inner.finish()
}

struct TreeBuilder<'a> {
    source: &'a str,
    tokens: &'a [Token],
    /// Index into `tokens`, trivia included.
    pos: usize,
    depth: usize,
    inner: GreenNodeBuilder<'static>,
}

impl TreeBuilder<'_> {
    fn start_node(&mut self, kind: SyntaxKind) {
        // The root takes everything, leading trivia included: nothing may
        // stay outside the tree.
        if self.depth > 0 {
            self.eat_trivia();
        }
        self.inner.start_node(Papyrus::kind_to_raw(kind));
        self.depth += 1;
    }

    fn token(&mut self, kind: SyntaxKind) {
        self.eat_trivia();
        let token = self.tokens[self.pos];
        debug_assert_eq!(token.kind, kind, "event and token diverge");
        self.emit(token);
    }

    fn finish_node(&mut self) {
        self.depth -= 1;
        if self.depth == 0 {
            // Safety net: whatever the parser did not consume goes into the
            // root rather than being lost. Normally only the final trivia is
            // left here.
            while let Some(&token) = self.tokens.get(self.pos) {
                debug_assert!(
                    token.kind.is_trivia(),
                    "{:?} left out of the tree by the parser",
                    token.kind
                );
                self.emit(token);
            }
        }
        self.inner.finish_node();
    }

    fn eat_trivia(&mut self) {
        while let Some(&token) = self.tokens.get(self.pos) {
            if !token.kind.is_trivia() {
                break;
            }
            self.emit(token);
        }
    }

    fn emit(&mut self, token: Token) {
        self.inner
            .token(Papyrus::kind_to_raw(token.kind), token.text(self.source));
        self.pos += 1;
    }
}
