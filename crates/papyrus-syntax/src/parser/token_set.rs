//! Token sets, for error recovery.
//!
//! A `TokenSet` says "here are the tokens I can latch onto". That is what
//! keeps a missing `EndIf` from carrying away the rest of the file.

use crate::SyntaxKind;

/// Number of 64-bit words needed to cover all kinds.
const WORDS: usize = 4;

const _: () = assert!(
    SyntaxKind::ALL.len() <= WORDS * 64,
    "too many SyntaxKind for the bitset: increase WORDS"
);

/// A set of [`SyntaxKind`], buildable in `const`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenSet([u64; WORDS]);

impl TokenSet {
    /// Builds a set from a list of kinds.
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

    /// True if `kind` belongs to the set.
    pub(crate) const fn contains(self, kind: SyntaxKind) -> bool {
        let raw = kind as usize;
        self.0[raw / 64] & (1 << (raw % 64)) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_is_exact() {
        let set = TokenSet::new(&[SyntaxKind::IF_KW, SyntaxKind::ENDIF_KW]);
        assert!(set.contains(SyntaxKind::IF_KW));
        assert!(set.contains(SyntaxKind::ENDIF_KW));
        assert!(!set.contains(SyntaxKind::ELSE_KW));
    }

    #[test]
    fn every_kind_is_representable() {
        // The bitset indexes by discriminant: an out-of-bounds kind would
        // silently corrupt another word.
        for kind in SyntaxKind::ALL {
            let set = TokenSet::new(&[*kind]);
            assert!(set.contains(*kind), "{kind:?} not representable");
        }
    }
}
