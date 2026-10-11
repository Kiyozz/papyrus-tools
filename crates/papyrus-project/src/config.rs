//! `papyrus.toml`, la configuration d'un projet.
//!
//! Cherché en remontant depuis le chemin traité, comme `rustfmt.toml` ou
//! `.editorconfig`. Tout y est facultatif : sans fichier, les valeurs par
//! défaut s'appliquent, et l'option de ligne de commande l'emporte toujours.
//!
//! ```toml
//! game = "skyrim"
//! include-fragments = false
//!
//! [fmt]
//! indent = 2          # ou "tab"
//!
//! [lints]
//! "naming/locals" = "error"
//! PAP001 = "allow"
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use papyrus_dialect::GameId;
use serde::Deserialize;

/// Le nom du fichier.
pub const FILE_NAME: &str = "papyrus.toml";

/// Ce qu'un lint fait quand il trouve quelque chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Désactivé.
    Allow,
    /// Signalé comme un avertissement.
    Warn,
    /// Signalé comme une erreur.
    Error,
}

impl FromStr for Level {
    type Err = String;

    fn from_str(s: &str) -> Result<Level, String> {
        match s.to_ascii_lowercase().as_str() {
            "allow" | "off" => Ok(Level::Allow),
            "warn" | "warning" => Ok(Level::Warn),
            "error" | "deny" => Ok(Level::Error),
            _ => Err(format!("niveau `{s}` inconnu : allow, warn ou error")),
        }
    }
}

/// Une unité d'indentation, telle qu'écrite dans la configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentSetting {
    /// Ce nombre d'espaces.
    Spaces(u8),
    /// Des tabulations.
    Tab,
}

/// La configuration, validée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Le fichier d'où elle vient, `None` pour les valeurs par défaut.
    pub path: Option<PathBuf>,
    /// Le jeu ciblé.
    pub game: Option<GameId>,
    /// Inclure les fragments du Creation Kit.
    pub include_fragments: bool,
    /// L'indentation du formateur.
    pub indent: Option<IndentSetting>,
    /// Un fichier `.flg` qui remplace les flags vanilla du jeu.
    pub flags: Option<PathBuf>,
    /// Le niveau de chaque lint, par code (`PAP001`) ou par nom
    /// (`encoding/lossy`). La validation des clés revient au registre.
    pub lints: BTreeMap<String, Level>,
}

/// Une configuration illisible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// Le fichier fautif.
    pub path: PathBuf,
    /// Ce qui ne va pas.
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} : {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Le fichier tel qu'écrit, avant validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Raw {
    game: Option<String>,
    #[serde(default)]
    include_fragments: bool,
    flags: Option<PathBuf>,
    #[serde(default)]
    fmt: RawFmt,
    #[serde(default)]
    lints: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFmt {
    indent: Option<RawIndent>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawIndent {
    Spaces(i64),
    Named(String),
}

impl Config {
    /// Lit et valide le contenu d'un `papyrus.toml`.
    pub fn parse(text: &str, path: &Path) -> Result<Config, ConfigError> {
        let error = |message: String| ConfigError {
            path: path.to_owned(),
            message,
        };
        let raw: Raw = toml::from_str(text).map_err(|e| error(e.message().to_owned()))?;

        let game = raw
            .game
            .map(|g| g.parse::<GameId>().map_err(|e| error(e.to_string())))
            .transpose()?;
        let indent = match raw.fmt.indent {
            None => None,
            Some(RawIndent::Spaces(n @ 1..=8)) => Some(IndentSetting::Spaces(n as u8)),
            Some(RawIndent::Named(s)) if s.eq_ignore_ascii_case("tab") => Some(IndentSetting::Tab),
            Some(other) => {
                return Err(error(format!(
                    "fmt.indent : attendu 1 à 8 espaces ou \"tab\", trouvé {other:?}"
                )));
            }
        };
        let lints = raw
            .lints
            .into_iter()
            .map(|(key, level)| {
                let level = level
                    .parse::<Level>()
                    .map_err(|e| error(format!("lints.\"{key}\" : {e}")))?;
                Ok((key, level))
            })
            .collect::<Result<_, ConfigError>>()?;

        // Un chemin relatif s'entend depuis le fichier de configuration.
        let flags = raw.flags.map(|f| match path.parent() {
            Some(dir) if f.is_relative() => dir.join(f),
            _ => f,
        });

        Ok(Config {
            path: Some(path.to_owned()),
            game,
            include_fragments: raw.include_fragments,
            indent,
            flags,
            lints,
        })
    }

    /// La configuration qui s'applique à `start` : le premier `papyrus.toml`
    /// trouvé en remontant, ou les valeurs par défaut.
    pub fn discover(start: &Path) -> Result<Config, ConfigError> {
        let start = std::path::absolute(start).unwrap_or_else(|_| start.to_owned());
        let dir = if start.is_dir() {
            Some(start.as_path())
        } else {
            start.parent()
        };
        for dir in dir.into_iter().flat_map(Path::ancestors) {
            let candidate = dir.join(FILE_NAME);
            if candidate.is_file() {
                let text = std::fs::read_to_string(&candidate).map_err(|e| ConfigError {
                    path: candidate.clone(),
                    message: e.to_string(),
                })?;
                return Config::parse(&text, &candidate);
            }
        }
        Ok(Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("/projet/papyrus.toml"))
    }

    #[test]
    fn un_fichier_vide_donne_les_valeurs_par_defaut() {
        let config = parse("").unwrap();
        assert_eq!(config.game, None);
        assert_eq!(config.indent, None);
        assert!(config.lints.is_empty());
    }

    #[test]
    fn un_fichier_complet_se_lit() {
        let config = parse(
            "game = \"sse\"\n\
             include-fragments = true\n\
             flags = \"TESV_Papyrus_Flags.flg\"\n\
             [fmt]\n\
             indent = \"tab\"\n\
             [lints]\n\
             \"naming/locals\" = \"error\"\n\
             PAP001 = \"allow\"\n",
        )
        .unwrap();
        assert_eq!(config.game, Some(GameId::Skyrim));
        assert!(config.include_fragments);
        assert_eq!(config.indent, Some(IndentSetting::Tab));
        assert_eq!(
            config.flags.as_deref(),
            Some(
                Path::new("/projet")
                    .join("TESV_Papyrus_Flags.flg")
                    .as_path()
            )
        );
        assert_eq!(config.lints["naming/locals"], Level::Error);
        assert_eq!(config.lints["PAP001"], Level::Allow);
    }

    #[test]
    fn les_fautes_sont_signalees() {
        // Une faute de frappe dans une clé ne doit pas passer en silence.
        assert!(parse("gmae = \"skyrim\"").is_err());
        assert!(parse("game = \"morrowind\"").is_err());
        assert!(parse("[fmt]\nindent = 0").is_err());
        assert!(parse("[lints]\nPAP001 = \"loud\"").is_err());
    }
}
