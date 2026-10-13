//! Which names a file mentions, so that a search for a name only reads the
//! files that may hold it.
//!
//! Each file gets a Bloom filter of its words, built from its tree once and
//! kept by salsa until the file changes: 256 bytes whatever its size, a few
//! megabytes for a game's whole corpus. A filter never misses a word it was
//! given; it may claim one it was not, about once in a hundred lookups, and
//! that file is then read for nothing.

use papyrus_dialect::GameId;

use crate::SourceFile;

const WORDS: usize = 32;
const BITS: u64 = (WORDS * 64) as u64;

/// The words of a file, case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mentions([u64; WORDS]);

impl Default for Mentions {
    fn default() -> Mentions {
        Mentions([0; WORDS])
    }
}

/// FNV-1a over the lowercased bytes, and a second hash derived from it.
fn hashes(word: &str) -> (u64, u64) {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in word.bytes() {
        hash ^= u64::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    (hash, hash.rotate_left(31) | 1)
}

/// The three bits of `word`.
fn bits(word: &str) -> impl Iterator<Item = usize> {
    let (a, b) = hashes(word);
    (0..3u64).map(move |i| (a.wrapping_add(i.wrapping_mul(b)) % BITS) as usize)
}

impl Mentions {
    fn insert(&mut self, word: &str) {
        for bit in bits(word) {
            self.0[bit / 64] |= 1 << (bit % 64);
        }
    }

    /// False if the file surely does not mention `name`. A qualified name,
    /// `DLC01:Foo`, is looked up by its last part.
    pub(crate) fn may_mention(&self, name: &str) -> bool {
        let word = name.rsplit(':').next().unwrap_or(name);
        bits(word).all(|bit| self.0[bit / 64] & (1 << (bit % 64)) != 0)
    }
}

/// The words of `file`, read for `game`: its identifiers and keywords.
#[salsa::tracked(returns(ref))]
pub(crate) fn mentions(
    db: &dyn salsa::Database,
    file: SourceFile,
    game: Option<GameId>,
) -> Mentions {
    let mut mentions = Mentions::default();
    for token in crate::parse(db, file, game)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let text = token.text();
        if text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            mentions.insert(text);
        }
    }
    mentions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_found_whatever_their_case() {
        let mut mentions = Mentions::default();
        mentions.insert("RegisterForSingleUpdate");
        assert!(mentions.may_mention("registerforsingleupdate"));
        assert!(mentions.may_mention("DLC01:REGISTERFORSINGLEUPDATE"));
        assert!(!mentions.may_mention("RegisterForUpdate"));
    }
}
