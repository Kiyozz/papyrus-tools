//! `papyrus.toml`, the configuration of a project.
//!
//! Looked up by walking up from the processed path, like `rustfmt.toml` or
//! `.editorconfig`. Everything in it is optional: without a file the
//! defaults apply, and the command-line option always wins.
//!
//! ```toml
//! game = "skyrim"
//! include-fragments = false
//!
//! [fmt]
//! indent = 2          # or "tab"
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

/// The file name.
pub const FILE_NAME: &str = "papyrus.toml";

/// What a lint does when it finds something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Disabled.
    Allow,
    /// Reported as a warning.
    Warn,
    /// Reported as an error.
    Error,
}

impl FromStr for Level {
    type Err = String;

    fn from_str(s: &str) -> Result<Level, String> {
        match s.to_ascii_lowercase().as_str() {
            "allow" | "off" => Ok(Level::Allow),
            "warn" | "warning" => Ok(Level::Warn),
            "error" | "deny" => Ok(Level::Error),
            _ => Err(format!("unknown level `{s}`: allow, warn or error")),
        }
    }
}

/// An indentation unit, as written in the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentSetting {
    /// This many spaces.
    Spaces(u8),
    /// Tabs.
    Tab,
}

/// The validated configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// The file it comes from, `None` for the defaults.
    pub path: Option<PathBuf>,
    /// The target game.
    pub game: Option<GameId>,
    /// Include the Creation Kit fragments.
    pub include_fragments: bool,
    /// The formatter indentation.
    pub indent: Option<IndentSetting>,
    /// A `.flg` file replacing the game's vanilla flags.
    pub flags: Option<PathBuf>,
    /// Folders of scripts the project builds on — a library, another mod —
    /// read but never checked, like the compiler's import folders.
    pub imports: Vec<PathBuf>,
    /// The level of each lint, by code (`PAP001`) or by name
    /// (`encoding/lossy`). Validating the keys is the registry's job.
    pub lints: BTreeMap<String, Level>,
}

/// An unreadable configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// The faulty file.
    pub path: PathBuf,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ConfigError {}

/// `path` without its `.` and `..` components, resolved lexically:
/// `mod/../lib` becomes `lib`.
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The file as written, before validation.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Raw {
    game: Option<String>,
    #[serde(default)]
    include_fragments: bool,
    flags: Option<PathBuf>,
    #[serde(default)]
    imports: Vec<PathBuf>,
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
    /// Reads and validates the contents of a `papyrus.toml`.
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
                    "fmt.indent: expected 1 to 8 spaces or \"tab\", found {other:?}"
                )));
            }
        };
        let lints = raw
            .lints
            .into_iter()
            .map(|(key, level)| {
                let level = level
                    .parse::<Level>()
                    .map_err(|e| error(format!("lints.\"{key}\": {e}")))?;
                Ok((key, level))
            })
            .collect::<Result<_, ConfigError>>()?;

        // A relative path is relative to the configuration file.
        let relative = |f: PathBuf| match path.parent() {
            Some(dir) if f.is_relative() => normalize(&dir.join(f)),
            _ => f,
        };
        let flags = raw.flags.map(relative);
        let imports = raw.imports.into_iter().map(relative).collect();

        Ok(Config {
            path: Some(path.to_owned()),
            game,
            include_fragments: raw.include_fragments,
            indent,
            flags,
            imports,
            lints,
        })
    }

    /// The configuration that applies to `start`: the first `papyrus.toml`
    /// found walking up, or the defaults.
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
        Config::parse(text, Path::new("/project/papyrus.toml"))
    }

    #[test]
    fn an_empty_file_gives_the_defaults() {
        let config = parse("").unwrap();
        assert_eq!(config.game, None);
        assert_eq!(config.indent, None);
        assert!(config.lints.is_empty());
    }

    #[test]
    fn a_full_file_reads() {
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
                Path::new("/project")
                    .join("TESV_Papyrus_Flags.flg")
                    .as_path()
            )
        );
        assert_eq!(config.lints["naming/locals"], Level::Error);
        assert_eq!(config.lints["PAP001"], Level::Allow);
    }

    #[test]
    fn mistakes_are_reported() {
        // A typo in a key must not go unnoticed.
        assert!(parse("gmae = \"skyrim\"").is_err());
        assert!(parse("game = \"morrowind\"").is_err());
        assert!(parse("[fmt]\nindent = 0").is_err());
        assert!(parse("[lints]\nPAP001 = \"loud\"").is_err());
    }
}
