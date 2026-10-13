//! The compiler's `.flg` files.
//!
//! They declare the "user" flags — `Hidden`, `Conditional`, `Mandatory`… —
//! and the objects that can carry them. The format, from the header of the
//! vanilla files:
//!
//! ```text
//! Flag <name> <index>                       // allowed everywhere
//! Flag <name> <index> { <targets> }         // allowed on these objects
//! Flag <name> <child> (& <child>)+          // combination (Fallout 4+)
//! ```
//!
//! C-style comments (`//`, `/* */`), whitespace insensitive. The targets are
//! `Script`, `Property`, `Variable`, `StructVar`, `Function` and `Group`.

use std::fmt;

/// An object that can carry a flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagTarget {
    /// The `ScriptName` header.
    Script,
    /// A property.
    Property,
    /// A script variable.
    Variable,
    /// A struct field.
    StructVar,
    /// A function or an event.
    Function,
    /// A property group.
    Group,
}

impl FlagTarget {
    /// All targets.
    pub const ALL: [FlagTarget; 6] = [
        FlagTarget::Script,
        FlagTarget::Property,
        FlagTarget::Variable,
        FlagTarget::StructVar,
        FlagTarget::Function,
        FlagTarget::Group,
    ];

    fn parse(word: &str) -> Option<FlagTarget> {
        FlagTarget::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(word))
    }

    /// The name, as written in a `.flg`.
    pub const fn name(self) -> &'static str {
        match self {
            FlagTarget::Script => "Script",
            FlagTarget::Property => "Property",
            FlagTarget::Variable => "Variable",
            FlagTarget::StructVar => "StructVar",
            FlagTarget::Function => "Function",
            FlagTarget::Group => "Group",
        }
    }
}

/// A declared flag, its combinations resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagDef {
    /// The name, with the file's case.
    pub name: String,
    /// The objects that can carry it. A combination inherits the
    /// intersection of its children.
    pub targets: Vec<FlagTarget>,
}

/// An unreadable `.flg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagsError(pub String);

impl fmt::Display for FlagsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FlagsError {}

/// Strips C-style comments.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("//") {
            rest = after.find('\n').map_or("", |i| &after[i..]);
        } else if let Some(after) = rest.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |i| &after[i + 2..]);
            out.push(' ');
        } else {
            let ch = rest.chars().next().expect("not empty");
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    out
}

/// Splits into words, braces and standalone `&`.
fn words(text: &str) -> Vec<String> {
    let spaced = text
        .replace('{', " { ")
        .replace('}', " } ")
        .replace('&', " & ");
    spaced.split_whitespace().map(str::to_owned).collect()
}

/// Reads a `.flg`.
pub fn parse_flags(text: &str) -> Result<Vec<FlagDef>, FlagsError> {
    let words = words(&strip_comments(text));
    let mut words = words.iter().map(String::as_str).peekable();
    let mut defs: Vec<FlagDef> = Vec::new();

    while let Some(word) = words.next() {
        if !word.eq_ignore_ascii_case("flag") {
            return Err(FlagsError(format!("expected `Flag`, found `{word}`")));
        }
        let name = words
            .next()
            .ok_or_else(|| FlagsError("expected a flag name".into()))?
            .to_owned();
        let second = words
            .next()
            .ok_or_else(|| FlagsError(format!("expected an index after `{name}`")))?;

        let targets = if let Ok(index) = second.parse::<u8>() {
            if index > 31 {
                return Err(FlagsError(format!("`{name}`: index {index} out of 0..=31")));
            }
            if words.peek() == Some(&"{") {
                words.next();
                let mut targets = Vec::new();
                loop {
                    match words.next() {
                        Some("}") => break,
                        Some(t) => targets.push(FlagTarget::parse(t).ok_or_else(|| {
                            FlagsError(format!("`{name}`: unknown target `{t}`"))
                        })?),
                        None => return Err(FlagsError(format!("`{name}`: missing `}}`"))),
                    }
                }
                targets
            } else {
                FlagTarget::ALL.to_vec()
            }
        } else {
            // Combination: allowed where all its children are.
            let mut children = vec![second];
            while words.peek() == Some(&"&") {
                words.next();
                children.push(
                    words
                        .next()
                        .ok_or_else(|| FlagsError(format!("`{name}`: expected a child")))?,
                );
            }
            let mut targets = FlagTarget::ALL.to_vec();
            for child in children {
                let def = defs
                    .iter()
                    .find(|d| d.name.eq_ignore_ascii_case(child))
                    .ok_or_else(|| FlagsError(format!("`{name}`: unknown flag `{child}`")))?;
                targets.retain(|t| def.targets.contains(t));
            }
            targets
        };
        defs.push(FlagDef { name, targets });
    }
    Ok(defs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_format_vanilla_se_lit() {
        let defs = parse_flags(
            "/*\n en-tête { Flag ignoré 9 }\n*/\n\
             // Flag hides the script\n\
             Flag Hidden 0\n{\n\tScript\n\tProperty\n}\n\
             Flag CollapsedOnRef 3 { Group }\n\
             Flag CollapsedOnBase 4 { Group }\n\
             Flag Collapsed CollapsedOnRef & CollapsedOnBase\n\
             Flag Anywhere 7\n",
        )
        .unwrap();
        assert_eq!(defs.len(), 5);
        assert_eq!(defs[0].name, "Hidden");
        assert_eq!(defs[0].targets, [FlagTarget::Script, FlagTarget::Property]);
        assert_eq!(defs[3].name, "Collapsed");
        assert_eq!(defs[3].targets, [FlagTarget::Group]);
        assert_eq!(defs[4].targets, FlagTarget::ALL);
    }

    #[test]
    fn les_erreurs_sont_signalees() {
        assert!(parse_flags("Flag X 32").is_err());
        assert!(parse_flags("Flag X 1 { Truc }").is_err());
        assert!(parse_flags("Flag X 1 { Script").is_err());
        assert!(parse_flags("Flag X Y & Z").is_err());
        assert!(parse_flags("Drapeau X 1").is_err());
    }
}
