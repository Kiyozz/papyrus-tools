//! Writing to `papyrus.toml`: the editor's quick actions, and a new file.
//!
//! The file stays the user's: comments, order and layout are kept, only the
//! changed key is touched.

use papyrus_dialect::GameId;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::Level;

/// The JSON schema of `papyrus.toml`, for TOML editors.
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/Kiyozz/papyrus-tools/main/schemas/papyrus.schema.json";

/// A change to `papyrus.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change<'a> {
    /// The level of a lint. The key already written for it, its code or
    /// its name, is kept; else its name is added.
    Lint {
        /// `PAP001`.
        code: &'a str,
        /// `encoding/lossy`.
        name: &'a str,
        /// The new level.
        level: Level,
    },
    /// `skse = true`.
    EnableSkse,
}

/// `text`, the contents of a `papyrus.toml`, with `change` applied; `None`
/// if it is not valid TOML.
pub fn apply(text: &str, change: Change<'_>) -> Option<String> {
    let mut doc: DocumentMut = text.parse().ok()?;
    match change {
        Change::Lint { code, name, level } => {
            let lints = doc
                .entry("lints")
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_like_mut()?;
            let key = lints
                .iter()
                .map(|(k, _)| k)
                .find(|k| k.eq_ignore_ascii_case(code) || k.eq_ignore_ascii_case(name))
                .unwrap_or(name)
                .to_owned();
            set(lints.entry(&key).or_insert(Item::None), level.as_str());
        }
        Change::EnableSkse => set(doc.entry("skse").or_insert(Item::None), true),
    }
    Some(doc.to_string())
}

/// Replaces `item` by `new`, keeping the comment that may follow it.
fn set(item: &mut Item, new: impl Into<toml_edit::Value>) {
    let mut new = value(new);
    if let (Some(old), Some(fresh)) = (item.as_value(), new.as_value_mut()) {
        *fresh.decor_mut() = old.decor().clone();
    }
    *item = new;
}

/// The name of the only `.ppj` of `dir`, for [`template`].
pub fn sole_ppj(dir: &std::path::Path) -> Option<String> {
    match crate::ppj::in_folder(dir).as_slice() {
        [only] => Some(only.file_name()?.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// A new `papyrus.toml` for a project targeting `game`, on top of the `.ppj`
/// of that name, its other keys commented out.
pub fn template(game: Option<GameId>, ppj: Option<&str>) -> String {
    let mut out = format!(
        "#:schema {SCHEMA_URL}\n\
         # Papyrus project: https://github.com/Kiyozz/papyrus-tools#configuration\n\n"
    );
    match ppj {
        Some(ppj) => out.push_str(&format!("ppj = \"{ppj}\"\n")),
        None => out.push_str("# ppj = \"mod.ppj\"  # a Creation Kit or Pyro project to read\n"),
    }
    match game {
        Some(game) => out.push_str(&format!("game = \"{}\"\n", game.as_str())),
        None => out.push_str("# game = \"skyrim\"  # fallout4, fallout76, starfield\n"),
    }
    if matches!(game, Some(GameId::Skyrim) | None) {
        out.push_str("# Skyrim with SKSE installed: its functions and scripts are known.\n");
        out.push_str("# skse = true\n");
    }
    out.push_str(
        "# Folders of scripts the project builds on, read but never checked.\n\
         # imports = [\"../Library/Source/Scripts\"]\n\
         \n\
         [lints]\n\
         # \"naming/locals\" = \"error\"\n\
         \n\
         [build]\n\
         # compiler = \"path/to/PapyrusCompiler.exe\"  # or Caprica\n\
         # output = \"Scripts\"\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::Config;

    fn lint(level: Level) -> Change<'static> {
        Change::Lint {
            code: "PAP101",
            name: "naming/locals",
            level,
        }
    }

    #[test]
    fn a_lint_level_is_added_by_name() {
        assert_eq!(
            apply("", lint(Level::Allow)).unwrap(),
            "[lints]\n\"naming/locals\" = \"allow\"\n"
        );
        let text = "game = \"skyrim\" # the target\n\n[build]\noutput = \"Scripts\"\n";
        assert_eq!(
            apply(text, lint(Level::Error)).unwrap(),
            "game = \"skyrim\" # the target\n\n[build]\noutput = \"Scripts\"\n\
             \n[lints]\n\"naming/locals\" = \"error\"\n"
        );
    }

    #[test]
    fn the_key_already_written_is_kept() {
        let text = "[lints]\n# loud\npap101 = \"warn\"\nPAP001 = \"allow\"\n";
        assert_eq!(
            apply(text, lint(Level::Allow)).unwrap(),
            "[lints]\n# loud\npap101 = \"allow\"\nPAP001 = \"allow\"\n"
        );
        assert_eq!(
            apply("[lints]\nPAP101 = \"warn\"  # noisy\n", lint(Level::Error)).unwrap(),
            "[lints]\nPAP101 = \"error\"  # noisy\n"
        );
        let inline = "lints = { \"naming/locals\" = \"warn\" }\n";
        assert_eq!(
            apply(inline, lint(Level::Error)).unwrap(),
            "lints = { \"naming/locals\" = \"error\" }\n"
        );
    }

    #[test]
    fn skse_is_enabled_among_the_top_keys() {
        let text = "game = \"skyrim\"\n\n[lints]\nPAP001 = \"allow\"\n";
        assert_eq!(
            apply(text, Change::EnableSkse).unwrap(),
            "game = \"skyrim\"\nskse = true\n\n[lints]\nPAP001 = \"allow\"\n"
        );
        assert_eq!(
            apply("skse = false\n", Change::EnableSkse).unwrap(),
            "skse = true\n"
        );
    }

    #[test]
    fn invalid_toml_is_left_alone() {
        assert_eq!(apply("game = ", Change::EnableSkse), None);
        assert_eq!(apply("lints = 3", lint(Level::Allow)), None);
    }

    #[test]
    fn the_template_reads() {
        let path = Path::new("/project/papyrus.toml");
        for game in [None, Some(GameId::Skyrim), Some(GameId::Starfield)] {
            let text = template(game, None);
            let config = Config::parse(&text, path).unwrap();
            assert_eq!(config.game, game);
            assert_eq!(text.contains("skse"), game != Some(GameId::Starfield));
        }
        let config = Config::parse(&template(None, Some("mod.ppj")), path).unwrap();
        assert_eq!(config.ppj, Some(PathBuf::from("/project/mod.ppj")));
    }
}
