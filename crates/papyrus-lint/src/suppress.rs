//! `; papyrus: allow(...)`, la suppression dans le code.
//!
//! En fin de ligne, le commentaire couvre sa ligne. Seul sur sa ligne, il
//! couvre la prochaine ligne de code — commentaires et lignes vides sautés.
//!
//! Un nom que le registre ne connaît pas est relevé, pour que
//! [`crate::ALLOW_UNKNOWN`] le signale : une faute de frappe ne doit pas
//! désactiver une suppression en silence.

use std::collections::BTreeSet;
use std::ops::Range;

use papyrus_syntax::{SyntaxKind, TextRange, TextSize, lex};

use crate::{LINTS, Lint, find};

/// Les lignes où chaque lint est autorisé.
#[derive(Debug, Default)]
pub(crate) struct Suppressions {
    /// `(code, ligne)`, lignes comptées depuis 0.
    allowed: BTreeSet<(&'static str, usize)>,
    /// Début de chaque ligne, en octets.
    line_starts: Vec<usize>,
    /// Les noms que le registre ne connaît pas, et où ils sont écrits.
    pub(crate) unknown: Vec<(TextRange, String)>,
}

/// Un nom écrit dans `allow(...)`.
#[derive(Debug)]
enum Key {
    Known(&'static Lint),
    /// Un nom inconnu, et sa plage dans le commentaire.
    Unknown(Range<usize>),
}

/// Les noms d'une directive, si le commentaire en est une.
fn directive(comment: &str) -> Option<Vec<Key>> {
    let body = comment.strip_prefix(';')?.trim();
    let body = body.strip_prefix("papyrus:")?.trim();
    let list = body.strip_prefix("allow(")?.split(')').next()?;
    // `list` est une tranche de `comment` : son décalage s'en déduit.
    let mut offset = list.as_ptr() as usize - comment.as_ptr() as usize;

    let mut keys = Vec::new();
    for raw in list.split(',') {
        let key = raw.trim();
        let start = offset + (raw.len() - raw.trim_start().len());
        offset += raw.len() + 1;
        if key.is_empty() {
            continue;
        }
        // `all` couvre tout.
        if key.eq_ignore_ascii_case("all") {
            keys.extend(LINTS.iter().map(|l| Key::Known(l)));
        } else {
            keys.push(match find(key) {
                Some(lint) => Key::Known(lint),
                None => Key::Unknown(start..start + key.len()),
            });
        }
    }
    Some(keys)
}

impl Suppressions {
    pub(crate) fn collect(source: &str) -> Suppressions {
        let mut line_starts = vec![0];
        line_starts.extend(source.match_indices('\n').map(|(i, _)| i + 1));
        let mut this = Suppressions {
            allowed: BTreeSet::new(),
            line_starts,
            unknown: Vec::new(),
        };

        let tokens = lex(source);
        for (i, token) in tokens.iter().enumerate() {
            if token.kind != SyntaxKind::LINE_COMMENT {
                continue;
            }
            let text = token.text(source);
            let Some(keys) = directive(text) else {
                continue;
            };
            let mut lints = Vec::new();
            for key in keys {
                match key {
                    Key::Known(lint) => lints.push(lint),
                    Key::Unknown(range) => {
                        let at = token.range.start();
                        let absolute = TextRange::new(
                            at + TextSize::from(range.start as u32),
                            at + TextSize::from(range.end as u32),
                        );
                        this.unknown.push((absolute, text[range].to_owned()));
                    }
                }
            }
            let line = this.line_of(token.range.start());

            // Du code avant le commentaire, sur la même ligne ?
            let trailing = tokens[..i]
                .iter()
                .rev()
                .take_while(|t| t.kind != SyntaxKind::NEWLINE)
                .any(|t| !t.kind.is_trivia());
            let target = if trailing {
                Some(line)
            } else {
                tokens[i + 1..]
                    .iter()
                    .find(|t| !t.kind.is_trivia() && t.kind != SyntaxKind::NEWLINE)
                    .map(|t| this.line_of(t.range.start()))
            };
            if let Some(target) = target {
                for lint in lints {
                    this.allowed.insert((lint.code, target));
                }
            }
        }
        this
    }

    fn line_of(&self, offset: TextSize) -> usize {
        let offset = usize::from(offset);
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    /// Vrai si `lint` est autorisé à `offset`.
    pub(crate) fn covers(&self, lint: &Lint, offset: TextSize) -> bool {
        self.allowed.contains(&(lint.code, self.line_of(offset)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ENCODING_LOSSY, NAMING_LOCALS};

    fn offset_of(source: &str, needle: &str) -> TextSize {
        TextSize::from(source.find(needle).expect("présent") as u32)
    }

    #[test]
    fn en_fin_de_ligne_couvre_la_ligne() {
        let source = "int HP = 0 ; papyrus: allow(naming/locals)\nint XP = 0\n";
        let s = Suppressions::collect(source);
        assert!(s.covers(&NAMING_LOCALS, offset_of(source, "HP")));
        assert!(!s.covers(&NAMING_LOCALS, offset_of(source, "XP")));
        assert!(!s.covers(&ENCODING_LOSSY, offset_of(source, "HP")));
    }

    #[test]
    fn seule_sur_sa_ligne_couvre_le_code_suivant() {
        let source =
            "; papyrus: allow(PAP101, PAP001)\n\n; autre commentaire\nint HP = 0\nint XP\n";
        let s = Suppressions::collect(source);
        assert!(s.covers(&NAMING_LOCALS, offset_of(source, "HP")));
        assert!(s.covers(&ENCODING_LOSSY, offset_of(source, "HP")));
        assert!(!s.covers(&NAMING_LOCALS, offset_of(source, "XP")));
    }

    #[test]
    fn all_couvre_tout() {
        let source = "int HP ; papyrus: allow(all)\n";
        let s = Suppressions::collect(source);
        assert!(s.covers(&NAMING_LOCALS, offset_of(source, "HP")));
        assert!(s.covers(&ENCODING_LOSSY, offset_of(source, "HP")));
        assert!(s.unknown.is_empty());
    }

    #[test]
    fn un_commentaire_ordinaire_ne_supprime_rien() {
        for comment in [
            "; allow(naming/locals)",
            "; papyrus allow(PAP101)",
            ";papyrus: deny(PAP101)",
        ] {
            assert!(directive(comment).is_none(), "{comment}");
        }
        // Les espaces sont libres.
        assert_eq!(directive(";papyrus:allow( PAP101 )").unwrap().len(), 1);
    }

    #[test]
    fn un_nom_inconnu_est_releve_a_sa_place() {
        let source = "int HP ; papyrus: allow(PAP101,  naming/lokals)\n";
        let s = Suppressions::collect(source);
        assert_eq!(s.unknown.len(), 1);
        let (range, text) = &s.unknown[0];
        assert_eq!(text, "naming/lokals");
        assert_eq!(&source[*range], "naming/lokals");
        // Le nom connu, lui, fait son effet.
        assert!(s.covers(&NAMING_LOCALS, offset_of(source, "HP")));
    }
}
