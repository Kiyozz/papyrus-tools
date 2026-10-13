//! The bridge to `rowan`.
//!
//! `rowan` only knows `u16`s; this module teaches it to read them back as
//! [`SyntaxKind`]s. The aliases below avoid repeating the type parameter
//! throughout the rest of the toolchain.

use crate::SyntaxKind;

/// The language, in the `rowan` sense.
///
/// Phantom type: there is no value of `Papyrus`, only its
/// [`rowan::Language`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Papyrus {}

impl rowan::Language for Papyrus {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from_raw(raw.0).expect("kind outside the enumeration")
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.into())
    }
}

/// A node of the Papyrus tree.
pub type SyntaxNode = rowan::SyntaxNode<Papyrus>;
/// A token of the Papyrus tree.
pub type SyntaxToken = rowan::SyntaxToken<Papyrus>;
/// A node or a token.
pub type SyntaxElement = rowan::SyntaxElement<Papyrus>;
/// Iterator over the children of a node, tokens included.
pub type SyntaxElementChildren = rowan::SyntaxElementChildren<Papyrus>;
/// Iterator over the child nodes of a node.
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<Papyrus>;
