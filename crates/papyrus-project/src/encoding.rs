//! Decoding of `.psc` files, and the `PAP001 encoding/lossy` lint.
//!
//! # What the compilers do
//!
//! Measured on the three official `PapyrusCompiler.exe`:
//!
//! - **Skyrim** reads Windows-1252, and only recognizes UTF-8 by its BOM.
//!   Without a BOM, `é` (`C3 A9`) becomes `Ã©` in the `.pex`.
//! - **Fallout 4 and Starfield** read strict UTF-8, and replace any
//!   Windows-1252 byte with U+FFFD. The loss is irreversible.
//!
//! In both cases the compiler prints "Compilation succeeded". Hence the
//! lint: nobody else will say it.
//!
//! # What matters
//!
//! Only the text that ends up in the `.pex`: strings and docstrings. An
//! accent in a comment harms nobody, and reporting it would be noise.

use encoding_rs::WINDOWS_1252;
use papyrus_dialect::GameId;
use papyrus_syntax::{SyntaxKind, TextRange, TextSize, lex};

/// The lint code.
pub const ENCODING_LOSSY: &str = "PAP001";

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// The encoding a file was read with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEncoding {
    /// Pure ASCII: reads the same everywhere, no risk.
    Ascii,
    /// Valid UTF-8, without BOM.
    Utf8,
    /// Valid UTF-8, preceded by a BOM.
    Utf8Bom,
    /// Not valid UTF-8: read as Windows-1252, which accepts any byte.
    Windows1252,
}

impl SourceEncoding {
    /// Human-readable name, for messages.
    pub const fn display_name(self) -> &'static str {
        match self {
            SourceEncoding::Ascii => "ASCII",
            SourceEncoding::Utf8 => "UTF-8 sans BOM",
            SourceEncoding::Utf8Bom => "UTF-8 avec BOM",
            SourceEncoding::Windows1252 => "Windows-1252",
        }
    }
}

/// A decoded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text. A BOM, if any, stays at its head as U+FEFF: the lexer treats
    /// it as trivia, and rewriting it as is preserves the file.
    pub text: String,
    /// The detected encoding.
    pub encoding: SourceEncoding,
    /// True if decoding had to replace bytes. Rewriting `text` would then
    /// lose information: the formatter must abstain.
    pub lossy: bool,
}

/// Decodes any `.psc`. Cannot fail.
pub fn decode(bytes: &[u8]) -> Decoded {
    if bytes.is_ascii() {
        return Decoded {
            text: String::from_utf8(bytes.to_vec()).expect("l'ASCII est de l'UTF-8"),
            encoding: SourceEncoding::Ascii,
            lossy: false,
        };
    }

    let has_bom = bytes.starts_with(BOM);
    match std::str::from_utf8(bytes) {
        Ok(text) => Decoded {
            text: text.to_owned(),
            encoding: if has_bom {
                SourceEncoding::Utf8Bom
            } else {
                SourceEncoding::Utf8
            },
            lossy: false,
        },
        // A BOM followed by invalid bytes: the file claims to be UTF-8 but is
        // not. It is read as such, flagging the loss.
        Err(_) if has_bom => Decoded {
            text: String::from_utf8_lossy(bytes).into_owned(),
            encoding: SourceEncoding::Utf8Bom,
            lossy: true,
        },
        Err(_) => {
            // Windows-1252 maps each of the 256 bytes to a character: nothing
            // is replaced.
            let (text, lossy) = WINDOWS_1252.decode_without_bom_handling(bytes);
            Decoded {
                text: text.into_owned(),
                encoding: SourceEncoding::Windows1252,
                lossy,
            }
        }
    }
}

/// Re-encodes `text` as the original file was.
///
/// `None` if a character cannot be represented — an added character
/// Windows-1252 does not know, for instance.
pub fn encode(text: &str, encoding: SourceEncoding) -> Option<Vec<u8>> {
    match encoding {
        SourceEncoding::Ascii | SourceEncoding::Utf8 | SourceEncoding::Utf8Bom => {
            Some(text.as_bytes().to_vec())
        }
        SourceEncoding::Windows1252 => {
            let (bytes, _, unmappable) = WINDOWS_1252.encode(text);
            (!unmappable).then(|| bytes.into_owned())
        }
    }
}

/// A text the target's compiler will silently corrupt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingIssue {
    /// The first affected character.
    pub range: TextRange,
    /// The explanation, with the remedy.
    pub message: String,
    /// Number of affected strings and docstrings.
    pub affected: usize,
}

/// The `PAP001` lint: will this file, compiled for `game`, lose text?
pub fn check(decoded: &Decoded, game: GameId) -> Option<EncodingIssue> {
    let at_risk = match decoded.encoding {
        SourceEncoding::Utf8 => game == GameId::Skyrim,
        SourceEncoding::Windows1252 => game != GameId::Skyrim,
        SourceEncoding::Ascii | SourceEncoding::Utf8Bom => false,
    };
    if !at_risk {
        return None;
    }

    let text = &decoded.text;
    let mut touched = lex(text).into_iter().filter(|t| {
        matches!(t.kind, SyntaxKind::STRING | SyntaxKind::DOC_COMMENT) && !t.text(text).is_ascii()
    });
    let first = touched.next()?;
    let affected = 1 + touched.count();

    let (offset, ch) = first
        .text(text)
        .char_indices()
        .find(|(_, c)| !c.is_ascii())
        .expect("le token contient un caractère non ASCII");
    let range = TextRange::at(
        first.range.start() + TextSize::from(offset as u32),
        TextSize::of(ch),
    );

    let count = if affected == 1 {
        "1 chaîne ou docstring touchée".to_owned()
    } else {
        format!("{affected} chaînes ou docstrings touchées")
    };
    let message = match decoded.encoding {
        SourceEncoding::Utf8 => {
            let mut buf = [0; 4];
            let (seen, _) =
                WINDOWS_1252.decode_without_bom_handling(ch.encode_utf8(&mut buf).as_bytes());
            format!(
                "UTF-8 sans BOM : le compilateur de Skyrim lit ce fichier en Windows-1252 \
                 et écrira « {seen} » au lieu de « {ch} », sans le signaler ({count}). \
                 Ajouter un BOM UTF-8, ou enregistrer en Windows-1252."
            )
        }
        _ => format!(
            "Windows-1252 : le compilateur de {} lit l'UTF-8 strict et remplacera « {ch} » \
             par « \u{FFFD} », sans le signaler et sans retour possible ({count}). \
             Enregistrer en UTF-8.",
            game.display_name()
        ),
    };

    Some(EncodingIssue {
        range,
        message,
        affected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_GAMES: [GameId; 4] = [
        GameId::Skyrim,
        GameId::Fallout4,
        GameId::Fallout76,
        GameId::Starfield,
    ];

    #[test]
    fn la_detection_suit_les_octets() {
        assert_eq!(decode(b"int x").encoding, SourceEncoding::Ascii);
        assert_eq!(
            decode("s = \"é\"".as_bytes()).encoding,
            SourceEncoding::Utf8
        );
        assert_eq!(
            decode("\u{feff}s = \"é\"".as_bytes()).encoding,
            SourceEncoding::Utf8Bom
        );
        assert_eq!(
            decode(b"s = \"\xE9\"").encoding,
            SourceEncoding::Windows1252
        );
    }

    #[test]
    fn windows_1252_se_decode_sans_perte() {
        let decoded = decode(b"s = \"caf\xE9\"");
        assert_eq!(decoded.text, "s = \"café\"");
        assert!(!decoded.lossy);
        assert_eq!(
            encode(&decoded.text, decoded.encoding).unwrap(),
            b"s = \"caf\xE9\""
        );
    }

    #[test]
    fn le_bom_survit_a_l_aller_retour() {
        let bytes = "\u{feff}s = \"é\"".as_bytes();
        let decoded = decode(bytes);
        assert!(decoded.text.starts_with('\u{feff}'));
        assert_eq!(encode(&decoded.text, decoded.encoding).unwrap(), bytes);
    }

    #[test]
    fn un_bom_mensonger_est_marque_avec_perte() {
        let decoded = decode(b"\xEF\xBB\xBFs = \"\xE9\"");
        assert_eq!(decoded.encoding, SourceEncoding::Utf8Bom);
        assert!(decoded.lossy);
    }

    #[test]
    fn windows_1252_refuse_ce_qu_il_ne_connait_pas() {
        assert_eq!(encode("→", SourceEncoding::Windows1252), None);
    }

    #[test]
    fn utf8_sans_bom_ne_pose_probleme_qu_a_skyrim() {
        let decoded = decode("s = \"café\"".as_bytes());
        let issue = check(&decoded, GameId::Skyrim).expect("Skyrim corrompt");
        assert!(
            issue.message.contains("« Ã© » au lieu de « é »"),
            "{}",
            issue.message
        );
        assert_eq!(&decoded.text[issue.range], "é");

        for game in [GameId::Fallout4, GameId::Fallout76, GameId::Starfield] {
            assert_eq!(check(&decoded, game), None, "{game}");
        }
    }

    #[test]
    fn windows_1252_ne_pose_probleme_qu_apres_skyrim() {
        let decoded = decode(b"s = \"caf\xE9\"");
        assert_eq!(check(&decoded, GameId::Skyrim), None);
        for game in [GameId::Fallout4, GameId::Fallout76, GameId::Starfield] {
            let issue = check(&decoded, game).expect("perte");
            assert!(issue.message.contains('\u{FFFD}'));
        }
    }

    #[test]
    fn utf8_avec_bom_et_ascii_passent_partout() {
        for bytes in ["\u{feff}s = \"café\"".as_bytes(), b"s = \"cafe\""] {
            let decoded = decode(bytes);
            for game in ALL_GAMES {
                assert_eq!(check(&decoded, game), None);
            }
        }
    }

    #[test]
    fn un_accent_en_commentaire_ne_compte_pas() {
        // The comment is not compiled: no byte reaches the `.pex`.
        let decoded = decode("; café\n;/ été /;\nint x\n".as_bytes());
        assert_eq!(check(&decoded, GameId::Skyrim), None);

        let decoded = decode("{café}\n".as_bytes());
        assert!(
            check(&decoded, GameId::Skyrim).is_some(),
            "la docstring compte"
        );
    }

    #[test]
    fn toutes_les_chaines_touchees_sont_comptees() {
        let decoded = decode("a = \"é\"\nb = \"ok\"\nc = \"à\"\n".as_bytes());
        assert_eq!(check(&decoded, GameId::Skyrim).unwrap().affected, 2);
    }
}
