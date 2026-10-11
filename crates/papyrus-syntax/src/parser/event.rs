//! What the parser produces.
//!
//! The parser does not assemble the tree: it emits a flat sequence of events,
//! which [`super::tree_builder`] uses to build the `GreenNode`, reinjecting
//! the trivia. This separation is what makes `precede` possible — wrapping
//! an already finished node afterwards, essential for left-associative
//! expressions.

use crate::SyntaxKind;

/// One step of the parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    /// Opening of a node.
    ///
    /// `forward_parent` is the offset, in the event list, to a `Start` that
    /// must be opened *before* this one. That is how a node gets wrapped after
    /// being finished.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    /// A token, consumed from the input.
    Token { kind: SyntaxKind },
    /// Closing of the most recently opened node.
    Finish,
}

impl Event {
    /// A reserved `Start`, whose kind is set when the marker completes.
    ///
    /// Stays as is if the marker is abandoned; the tree builder then ignores
    /// it.
    pub(crate) const fn tombstone() -> Event {
        Event::Start {
            kind: SyntaxKind::TOMBSTONE,
            forward_parent: None,
        }
    }
}
