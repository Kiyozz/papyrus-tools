//! Décodage des `.psc`, et le lint `PAP001 encoding/lossy`.
//!
//! # Ce que font les compilateurs
//!
//! Mesuré sur les trois `PapyrusCompiler.exe` officiels :
//!
//! - **Skyrim** lit en Windows-1252, et ne reconnaît l'UTF-8 qu'à son BOM.
//!   Sans BOM, `é` (`C3 A9`) devient `Ã©` dans le `.pex`.
//! - **Fallout 4 et Starfield** lisent en UTF-8 strict, et remplacent tout
//!   octet Windows-1252 par U+FFFD. La perte est irréversible.
//!
//! Dans les deux cas le compilateur affiche « Compilation succeeded ». D'où le
//! lint : personne d'autre ne le dira.
//!
//! # Ce qui compte
//!
//! Seul le texte qui finit dans le `.pex` : les chaînes et les docstrings. Un
//! accent dans un commentaire ne fait de tort à personne, et le signaler
//! serait du bruit.

use encoding_rs::WINDOWS_1252;
use papyrus_dialect::GameId;
use papyrus_syntax::{SyntaxKind, TextRange, TextSize, lex};

/// Le code du lint.
pub const ENCODING_LOSSY: &str = "PAP001";

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// L'encodage dans lequel un fichier a été lu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEncoding {
    /// ASCII pur : se lit pareil partout, aucun risque.
    Ascii,
    /// UTF-8 valide, sans BOM.
    Utf8,
    /// UTF-8 valide, précédé d'un BOM.
    Utf8Bom,
    /// Pas de l'UTF-8 valide : lu en Windows-1252, qui accepte tout octet.
    Windows1252,
}

impl SourceEncoding {
    /// Nom lisible, pour les messages.
    pub const fn display_name(self) -> &'static str {
        match self {
            SourceEncoding::Ascii => "ASCII",
            SourceEncoding::Utf8 => "UTF-8 sans BOM",
            SourceEncoding::Utf8Bom => "UTF-8 avec BOM",
            SourceEncoding::Windows1252 => "Windows-1252",
        }
    }
}

/// Un fichier décodé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Le texte. Un BOM éventuel y reste, en tête, sous forme de U+FEFF : le
    /// lexer le traite en trivia, et le réécrire tel quel préserve le fichier.
    pub text: String,
    /// L'encodage détecté.
    pub encoding: SourceEncoding,
    /// Vrai si le décodage a dû remplacer des octets. Réécrire `text`
    /// perdrait alors de l'information : le formateur doit s'abstenir.
    pub lossy: bool,
}

/// Décode un `.psc`, quel qu'il soit. Ne peut pas échouer.
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
        // Un BOM suivi d'octets invalides : le fichier se dit UTF-8 mais ne
        // l'est pas. On le lit comme tel, en marquant la perte.
        Err(_) if has_bom => Decoded {
            text: String::from_utf8_lossy(bytes).into_owned(),
            encoding: SourceEncoding::Utf8Bom,
            lossy: true,
        },
        Err(_) => {
            // Windows-1252 donne un caractère à chacun des 256 octets : rien
            // n'est remplacé.
            let (text, lossy) = WINDOWS_1252.decode_without_bom_handling(bytes);
            Decoded {
                text: text.into_owned(),
                encoding: SourceEncoding::Windows1252,
                lossy,
            }
        }
    }
}

/// Réencode `text` comme l'était le fichier d'origine.
///
/// `None` si un caractère n'y est pas représentable — un caractère ajouté que
/// Windows-1252 ne connaît pas, par exemple.
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

/// Un texte que le compilateur de la cible va corrompre sans le dire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingIssue {
    /// Le premier caractère touché.
    pub range: TextRange,
    /// L'explication, avec le remède.
    pub message: String,
    /// Nombre de chaînes et docstrings touchées.
    pub affected: usize,
}

/// Le lint `PAP001` : ce fichier, compilé pour `game`, perdra-t-il du texte ?
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
        // Le commentaire n'est pas compilé : aucun octet n'atteint le `.pex`.
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
