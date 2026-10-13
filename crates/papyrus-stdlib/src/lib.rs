//! The signatures of each game's native scripts, embedded.
//!
//! Enough to reason about `extends ObjectReference` without an installed
//! game: which events a script can receive, which functions it inherits. The
//! tables are generated from the vanilla sources by `cargo xtask stdlib`, and
//! only contain **signatures** — no function bodies, no docstrings.
//!
//! Only *native* scripts are listed: those the engine implements, which carry
//! the `Native` flag or declare native functions, and those that only declare
//! — types without native functions, such as Skyrim's `Activator`, whose
//! `.psc` is a single line. They are the ones declaring the engine's types
//! and events.
//!
//! Skyrim's tables come twice: vanilla, and with the functions, events and
//! scripts of its script extender, SKSE. A [`Library`] picks one.

#![forbid(unsafe_code)]

mod generated;

use std::cmp::Ordering;

use papyrus_dialect::GameId;

/// A parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// The name, `akActionRef`.
    pub name: &'static str,
    /// The type, as written: `ObjectReference`, `int[]`.
    pub ty: &'static str,
    /// The default value, as written: `None`, `-1`.
    pub default: Option<&'static str>,
}

/// A function or an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Function {
    /// The name, with the vanilla case.
    pub name: &'static str,
    /// The return type, `None` for none.
    pub returns: Option<&'static str>,
    /// The parameters.
    pub params: &'static [Param],
    /// `Global`: called on the script, not on an instance.
    pub global: bool,
    /// `Native`: implemented by the engine.
    pub native: bool,
}

/// A property: `int Property CritStage_None = 0 AutoReadOnly`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Property {
    /// The name, with the vanilla case.
    pub name: &'static str,
    /// The type, as written.
    pub ty: &'static str,
    /// `AutoReadOnly`, `Const`, or without `Set`: cannot be assigned.
    pub readonly: bool,
}

/// A native script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Script {
    /// The name, with the vanilla case.
    pub name: &'static str,
    /// The parent script, `None` for the root.
    pub parent: Option<&'static str>,
    /// The functions, sorted by name case-insensitively.
    pub functions: &'static [Function],
    /// The events, sorted the same way.
    pub events: &'static [Function],
    /// The properties, sorted the same way.
    pub properties: &'static [Property],
}

/// A set of native scripts: a game's, with or without its script extender.
///
/// A [`GameId`] converts into its vanilla library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Library {
    /// The game.
    pub game: GameId,
    /// With SKSE's additions; only Skyrim has them.
    pub skse: bool,
}

impl Library {
    /// The vanilla library of `game`.
    pub fn new(game: GameId) -> Library {
        Library { game, skse: false }
    }

    /// The same library, with or without SKSE's additions.
    pub fn with_skse(self, skse: bool) -> Library {
        Library { skse, ..self }
    }
}

impl From<GameId> for Library {
    fn from(game: GameId) -> Library {
        Library::new(game)
    }
}

/// Compares two names the way Papyrus does: case-insensitively.
fn cmp_name(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

fn find<'a, T>(items: &'a [T], name: &str, key: impl Fn(&T) -> &str) -> Option<&'a T> {
    items
        .binary_search_by(|item| cmp_name(key(item), name))
        .ok()
        .map(|i| &items[i])
}

/// The native scripts of `library`, sorted by name.
pub fn scripts(library: impl Into<Library>) -> &'static [Script] {
    let library = library.into();
    match library.game {
        GameId::Skyrim if library.skse => generated::skyrim_skse::SCRIPTS,
        GameId::Skyrim => generated::skyrim::SCRIPTS,
        // Fallout 76 has neither a public Creation Kit nor sources: its base
        // scripts are Fallout 4's.
        GameId::Fallout4 | GameId::Fallout76 => generated::fallout4::SCRIPTS,
        GameId::Starfield => generated::starfield::SCRIPTS,
    }
}

/// The native script `name`, case-insensitively.
pub fn script(library: impl Into<Library>, name: &str) -> Option<&'static Script> {
    find(scripts(library), name, |s| s.name)
}

/// `name` and its native ancestors, from closest to farthest.
pub fn ancestors(library: impl Into<Library>, name: &str) -> impl Iterator<Item = &'static Script> {
    let library = library.into();
    let mut next = script(library, name);
    // Guard against a cycle, which badly generated tables could create.
    let mut steps = 0;
    std::iter::from_fn(move || {
        let current = next?;
        steps += 1;
        next = if steps < 64 {
            current.parent.and_then(|p| script(library, p))
        } else {
            None
        };
        Some(current)
    })
}

/// The event `event` received by a script deriving from `name`, walking up
/// the hierarchy.
pub fn find_event(
    library: impl Into<Library>,
    name: &str,
    event: &str,
) -> Option<&'static Function> {
    ancestors(library, name).find_map(|s| find(s.events, event, |f| f.name))
}

/// The function `function` inherited from `name`.
pub fn find_function(
    library: impl Into<Library>,
    name: &str,
    function: &str,
) -> Option<&'static Function> {
    ancestors(library, name).find_map(|s| find(s.functions, function, |f| f.name))
}

/// The names a library knows, lowercased, for bulk lookups.
#[derive(Debug, Default)]
struct Names {
    functions: std::collections::HashSet<String>,
    events: std::collections::HashSet<String>,
}

fn names(library: Library) -> &'static Names {
    use std::sync::OnceLock;
    static TABLES: [OnceLock<Names>; 4] = [const { OnceLock::new() }; 4];
    let slot = match library.game {
        GameId::Skyrim if library.skse => 3,
        GameId::Skyrim => 0,
        GameId::Fallout4 | GameId::Fallout76 => 1,
        GameId::Starfield => 2,
    };
    TABLES[slot].get_or_init(|| {
        let mut names = Names::default();
        for script in scripts(library) {
            names
                .functions
                .extend(script.functions.iter().map(|f| f.name.to_ascii_lowercase()));
            names
                .events
                .extend(script.events.iter().map(|f| f.name.to_ascii_lowercase()));
        }
        names
    })
}

/// True if a native script of `library` declares a function `name`.
pub fn has_function(library: impl Into<Library>, name: &str) -> bool {
    names(library.into())
        .functions
        .contains(&name.to_ascii_lowercase())
}

/// True if a native script of `library` declares an event `name`.
pub fn has_event(library: impl Into<Library>, name: &str) -> bool {
    names(library.into())
        .events
        .contains(&name.to_ascii_lowercase())
}
