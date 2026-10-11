//! Le pont vers `rowan`.
//!
//! `rowan` ne connaît que des `u16` ; ce module lui apprend à les relire comme
//! des [`SyntaxKind`]. Les alias qui suivent évitent de répéter le paramètre de
//! type dans tout le reste de la toolchain.

use crate::SyntaxKind;

/// Le langage, au sens de `rowan`.
///
/// Type fantôme : il n'existe aucune valeur de `Papyrus`, seulement son
/// implémentation de [`rowan::Language`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Papyrus {}

impl rowan::Language for Papyrus {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind::from_raw(raw.0).expect("kind hors de l'énumération")
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.into())
    }
}

/// Un nœud de l'arbre Papyrus.
pub type SyntaxNode = rowan::SyntaxNode<Papyrus>;
/// Un token de l'arbre Papyrus.
pub type SyntaxToken = rowan::SyntaxToken<Papyrus>;
/// Un nœud ou un token.
pub type SyntaxElement = rowan::SyntaxElement<Papyrus>;
/// Itérateur sur les enfants d'un nœud, tokens compris.
pub type SyntaxElementChildren = rowan::SyntaxElementChildren<Papyrus>;
/// Itérateur sur les enfants nœuds d'un nœud.
pub type SyntaxNodeChildren = rowan::SyntaxNodeChildren<Papyrus>;
