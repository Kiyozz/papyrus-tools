//! Ensembles de tokens, pour la récupération d'erreurs.
//!
//! Un `TokenSet` dit « voici les tokens sur lesquels je peux me raccrocher ».
//! C'est ce qui empêche un `EndIf` manquant d'emporter la suite du fichier.

use crate::SyntaxKind;

/// Nombre de mots de 64 bits nécessaires pour couvrir tous les kinds.
const WORDS: usize = 4;

const _: () = assert!(
    SyntaxKind::ALL.len() <= WORDS * 64,
    "trop de SyntaxKind pour le bitset : augmenter WORDS"
);

/// Un ensemble de [`SyntaxKind`], constructible en `const`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenSet([u64; WORDS]);

impl TokenSet {
    /// Construit un ensemble à partir d'une liste de kinds.
    pub(crate) const fn new(kinds: &[SyntaxKind]) -> TokenSet {
        let mut bits = [0u64; WORDS];
        let mut i = 0;
        while i < kinds.len() {
            let raw = kinds[i] as usize;
            bits[raw / 64] |= 1 << (raw % 64);
            i += 1;
        }
        TokenSet(bits)
    }

    /// Vrai si `kind` appartient à l'ensemble.
    pub(crate) const fn contains(self, kind: SyntaxKind) -> bool {
        let raw = kind as usize;
        self.0[raw / 64] & (1 << (raw % 64)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_appartenance_est_exacte() {
        let set = TokenSet::new(&[SyntaxKind::IF_KW, SyntaxKind::ENDIF_KW]);
        assert!(set.contains(SyntaxKind::IF_KW));
        assert!(set.contains(SyntaxKind::ENDIF_KW));
        assert!(!set.contains(SyntaxKind::ELSE_KW));
    }

    #[test]
    fn tous_les_kinds_sont_representables() {
        // Le bitset indexe par discriminant : un kind hors borne corromprait
        // silencieusement un autre mot.
        for kind in SyntaxKind::ALL {
            let set = TokenSet::new(&[*kind]);
            assert!(set.contains(*kind), "{kind:?} non représentable");
        }
    }
}
