//! Des événements vers l'arbre `rowan`.
//!
//! Le parser n'a vu que les tokens significatifs ; ce module rejoue ses
//! événements sur la liste complète et réinsère la trivia au passage.
//!
//! # Où va la trivia
//!
//! Dans le nœud **parent**, avant l'ouverture de l'enfant. Un nœud commence
//! donc exactement sur son premier token significatif et finit sur son dernier,
//! ce que veulent les diagnostics et la sélection dans l'éditeur. La trivia
//! qui suit le dernier token du fichier va dans la racine.

use std::mem;

use rowan::{GreenNode, GreenNodeBuilder, Language};

use super::event::Event;
use crate::SyntaxKind;
use crate::lexer::Token;
use crate::syntax_node::Papyrus;

/// Construit l'arbre à partir des événements du parser.
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
                // Remonte la chaîne des parents ajoutés après coup par
                // `precede`, puis les ouvre du plus externe au plus interne.
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
                        _ => unreachable!("forward_parent pointe toujours sur un Start"),
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
    /// Index dans `tokens`, trivia comprise.
    pos: usize,
    depth: usize,
    inner: GreenNodeBuilder<'static>,
}

impl TreeBuilder<'_> {
    fn start_node(&mut self, kind: SyntaxKind) {
        // La racine prend tout, y compris la trivia de tête : rien ne doit
        // rester hors de l'arbre.
        if self.depth > 0 {
            self.eat_trivia();
        }
        self.inner.start_node(Papyrus::kind_to_raw(kind));
        self.depth += 1;
    }

    fn token(&mut self, kind: SyntaxKind) {
        self.eat_trivia();
        let token = self.tokens[self.pos];
        debug_assert_eq!(token.kind, kind, "événement et token divergent");
        self.emit(token);
    }

    fn finish_node(&mut self) {
        self.depth -= 1;
        if self.depth == 0 {
            // Filet de sécurité : ce que le parser n'aurait pas consommé entre
            // dans la racine plutôt que d'être perdu. En temps normal, il ne
            // reste ici que la trivia finale.
            while let Some(&token) = self.tokens.get(self.pos) {
                debug_assert!(
                    token.kind.is_trivia(),
                    "{:?} laissé hors de l'arbre par le parser",
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
