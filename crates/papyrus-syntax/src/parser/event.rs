//! Ce que le parser produit.
//!
//! Le parser n'assemble pas l'arbre : il émet une suite d'événements plats, et
//! [`super::tree_builder`] s'en sert pour construire le `GreenNode` en y
//! réinjectant la trivia. Cette séparation est ce qui rend possible
//! `precede` — envelopper après coup un nœud déjà terminé, indispensable aux
//! expressions associatives à gauche.

use crate::SyntaxKind;

/// Un pas de l'analyse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Event {
    /// Ouverture d'un nœud.
    ///
    /// `forward_parent` est le décalage, dans la liste d'événements, vers un
    /// `Start` qui devra être ouvert *avant* celui-ci. C'est ainsi qu'un nœud
    /// se retrouve enveloppé après avoir été terminé.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    /// Un token, consommé depuis l'entrée.
    Token { kind: SyntaxKind },
    /// Fermeture du nœud ouvert le plus récemment.
    Finish,
}

impl Event {
    /// Un `Start` réservé, dont le kind sera fixé à la complétion du marqueur.
    ///
    /// Reste tel quel si le marqueur est abandonné ; le constructeur d'arbre
    /// l'ignore alors.
    pub(crate) const fn tombstone() -> Event {
        Event::Start {
            kind: SyntaxKind::TOMBSTONE,
            forward_parent: None,
        }
    }
}
