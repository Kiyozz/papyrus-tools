//! Papyrus compilation targets.
//!
//! Papyrus has no specification: each game ships its own compiler version,
//! which adds keywords and constructs without ever removing any. The language
//! is therefore a chain of supersets, which [`GameId::rank`] models.
//!
//! This crate knows neither syntax nor tokens: it only says "from which game
//! on does this exist". The table mapping keywords to the game that introduced
//! them lives in `papyrus-syntax`, which depends on this crate.

#![forbid(unsafe_code)]

use std::fmt;
use std::str::FromStr;

/// Target game, hence the Papyrus compiler version whose behavior we mimic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameId {
    /// Skyrim, all editions — LE, Special Edition, Anniversary.
    ///
    /// The language is identical; only the source layout on disk differs,
    /// which is a matter of project layout, not of dialect.
    Skyrim,
    /// Fallout 4. Adds namespaces, `Struct`, `Group`, `Var`, `Is`, `Const`,
    /// `DebugOnly` / `BetaOnly` and `CustomEvent`.
    Fallout4,
    /// Fallout 76.
    ///
    /// Exactly the same keyword set as Fallout 4 — in Caprica,
    /// `FALLOUT76_MAX_KEYWORD == FALLOUT4_MAX_KEYWORD`. There is no public
    /// Creation Kit nor extractable `.psc` source, so nothing can be verified:
    /// the value is accepted in configuration but promises nothing more than
    /// [`GameId::Fallout4`].
    Fallout76,
    /// Starfield. Superset of Fallout 4: `Guard`, `LockGuard`, `TryLockGuard`,
    /// `RequiresGuard`, `ProtectsFunctionLogic` and the `Private` /
    /// `Protected` / `SelfOnly` access modifiers.
    ///
    /// The default when nothing is configured: the widest superset, hence the
    /// one producing the fewest false positives.
    #[default]
    Starfield,
}

impl GameId {
    /// Position in the chain of supersets.
    ///
    /// Fallout 76 shares the rank of Fallout 4: both accept exactly the same
    /// language.
    pub const fn rank(self) -> u8 {
        match self {
            GameId::Skyrim => 0,
            GameId::Fallout4 | GameId::Fallout76 => 1,
            GameId::Starfield => 2,
        }
    }

    /// True if a construct introduced in `since` exists in this game.
    pub const fn accepts(self, since: GameId) -> bool {
        self.rank() >= since.rank()
    }

    /// Canonical name, as written in `papyrus.toml`.
    pub const fn as_str(self) -> &'static str {
        match self {
            GameId::Skyrim => "skyrim",
            GameId::Fallout4 => "fallout4",
            GameId::Fallout76 => "fallout76",
            GameId::Starfield => "starfield",
        }
    }

    /// Human-readable name, for diagnostic messages.
    pub const fn display_name(self) -> &'static str {
        match self {
            GameId::Skyrim => "Skyrim",
            GameId::Fallout4 => "Fallout 4",
            GameId::Fallout76 => "Fallout 76",
            GameId::Starfield => "Starfield",
        }
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error reading a game name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownGame(pub String);

impl fmt::Display for UnknownGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown target `{}` (expected: skyrim, fallout4, fallout76, starfield)",
            self.0
        )
    }
}

impl std::error::Error for UnknownGame {}

impl FromStr for GameId {
    type Err = UnknownGame;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // The aliases cover the ways people refer to these games in the
        // modding community.
        match s
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "")
            .as_str()
        {
            "skyrim" | "sse" | "skyrimse" | "skyrimle" | "skyrimae" | "tesv" => Ok(GameId::Skyrim),
            "fallout4" | "fo4" | "f4" => Ok(GameId::Fallout4),
            "fallout76" | "fo76" | "f76" => Ok(GameId::Fallout76),
            "starfield" | "sf" | "sf1" => Ok(GameId::Starfield),
            _ => Err(UnknownGame(s.to_owned())),
        }
    }
}

/// Syntactic construct whose availability depends on the target game.
///
/// Serves the dialect's second intervention point: constructs that do not
/// start with a demotable keyword are always parsed, and a diagnostic — not a
/// syntax error — reports that they do not exist in the target. The tree thus
/// stays complete and the editor stays usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Qualified script name, `Foo:Bar:Script`.
    Namespace,
    /// `Struct` / `EndStruct` declaration.
    Struct,
    /// `Group` / `EndGroup` block.
    Group,
    /// `Is` operator.
    IsOperator,
    /// `new StructName`, without brackets.
    NewStruct,
    /// `Property` declared in a `State` or a `Group`.
    NestedProperty,
    /// Immutable local, `Type x = expr Const`.
    ConstLocal,
    /// Locks: `Guard`, `LockGuard`, `TryLockGuard`.
    Guard,
    /// `Private`, `Protected`, `SelfOnly` access modifiers.
    AccessModifier,
    /// Remote event, `Event ObjectReference.OnActivate(…)`.
    ///
    /// Absent from the 14,324 Skyrim scripts, present in 695 of Fallout 4.
    RemoteEvent,
}

impl Feature {
    /// Game from which the construct exists.
    pub const fn since(self) -> GameId {
        match self {
            Feature::Namespace
            | Feature::Struct
            | Feature::Group
            | Feature::IsOperator
            | Feature::NewStruct
            | Feature::NestedProperty
            | Feature::ConstLocal
            | Feature::RemoteEvent => GameId::Fallout4,
            Feature::Guard | Feature::AccessModifier => GameId::Starfield,
        }
    }

    /// Short description, for diagnostic messages.
    pub const fn describe(self) -> &'static str {
        match self {
            Feature::Namespace => "namespace syntax",
            Feature::Struct => "structs",
            Feature::Group => "property groups",
            Feature::IsOperator => "the `Is` operator",
            Feature::NewStruct => "struct instantiation",
            Feature::NestedProperty => "nested properties",
            Feature::ConstLocal => "`Const` local variables",
            Feature::Guard => "guards (`Guard`)",
            Feature::AccessModifier => "access modifiers",
            Feature::RemoteEvent => "remote events (`Event Type.OnX`)",
        }
    }
}

/// Full target: the game, plus the enabled unofficial extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dialect {
    /// Target game.
    pub game: GameId,
    /// Third-party compiler extensions, all disabled by default.
    pub extensions: Extensions,
}

impl Dialect {
    /// Target without any extension.
    pub const fn new(game: GameId) -> Self {
        Dialect {
            game,
            extensions: Extensions::NONE,
        }
    }

    /// True if the construct exists in this target.
    pub const fn supports(self, feature: Feature) -> bool {
        self.game.accepts(feature.since())
    }
}

impl From<GameId> for Dialect {
    fn from(game: GameId) -> Self {
        Dialect::new(game)
    }
}

/// Syntax extensions missing from the official compilers.
///
/// Caprica accepts control flow Bethesda never implemented (`For`, `ForEach`,
/// `Switch`, `Break`, `Continue`, `Do`…). A project compiled with Caprica is
/// legitimate, but its code will not compile with the Creation Kit: accepting
/// it must therefore be an explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extensions {
    /// Control flow added by Caprica.
    pub caprica_control_flow: bool,
}

impl Extensions {
    /// No extension: the official compiler's behavior.
    pub const NONE: Self = Extensions {
        caprica_control_flow: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallout76_est_un_alias_de_fallout4() {
        assert_eq!(GameId::Fallout76.rank(), GameId::Fallout4.rank());
        assert!(GameId::Fallout76.accepts(GameId::Fallout4));
        assert!(GameId::Fallout4.accepts(GameId::Fallout76));
    }

    #[test]
    fn les_jeux_forment_une_suite_de_sur_ensembles() {
        assert!(GameId::Starfield.accepts(GameId::Fallout4));
        assert!(GameId::Starfield.accepts(GameId::Skyrim));
        assert!(GameId::Fallout4.accepts(GameId::Skyrim));

        assert!(!GameId::Skyrim.accepts(GameId::Fallout4));
        assert!(!GameId::Fallout4.accepts(GameId::Starfield));
    }

    #[test]
    fn la_cible_par_defaut_est_le_sur_ensemble_le_plus_large() {
        assert_eq!(GameId::default(), GameId::Starfield);
    }

    #[test]
    fn les_features_sont_rattachees_au_bon_jeu() {
        let skyrim = Dialect::new(GameId::Skyrim);
        let fo4 = Dialect::new(GameId::Fallout4);
        let sf = Dialect::new(GameId::Starfield);

        // `Is` does not exist in Skyrim: checked by compiling `x is Actor`
        // with Skyrim SE's PapyrusCompiler.exe, which rejects it.
        assert!(!skyrim.supports(Feature::IsOperator));
        assert!(fo4.supports(Feature::IsOperator));

        // Locks are specific to Starfield.
        assert!(!fo4.supports(Feature::Guard));
        assert!(sf.supports(Feature::Guard));
    }

    #[test]
    fn les_noms_de_jeu_acceptent_les_alias_de_la_communaute() {
        assert_eq!("skyrim".parse(), Ok(GameId::Skyrim));
        assert_eq!("Skyrim SE".parse(), Ok(GameId::Skyrim));
        assert_eq!("skyrim-le".parse(), Ok(GameId::Skyrim));
        assert_eq!("FO4".parse(), Ok(GameId::Fallout4));
        assert_eq!("fallout_76".parse(), Ok(GameId::Fallout76));
        assert_eq!("sf".parse(), Ok(GameId::Starfield));

        assert!("morrowind".parse::<GameId>().is_err());
    }

    #[test]
    fn le_nom_canonique_se_relit() {
        for game in [
            GameId::Skyrim,
            GameId::Fallout4,
            GameId::Fallout76,
            GameId::Starfield,
        ] {
            assert_eq!(game.as_str().parse(), Ok(game));
        }
    }
}
