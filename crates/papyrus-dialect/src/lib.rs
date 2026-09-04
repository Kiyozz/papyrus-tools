//! Cibles de compilation Papyrus.
//!
//! Papyrus n'a pas de spécification : chaque jeu embarque sa propre version du
//! compilateur, qui ajoute des mots-clés et des constructions sans jamais en
//! retirer. Le langage est donc une suite de sur-ensembles, ce que modélise
//! [`GameId::rank`].
//!
//! Ce crate ne connaît ni la syntaxe ni les tokens : il ne dit que « à partir de
//! quel jeu telle chose existe ». La table qui associe les mots-clés à leur jeu
//! d'apparition vit dans `papyrus-syntax`, qui dépend de ce crate.

#![forbid(unsafe_code)]

use std::fmt;
use std::str::FromStr;

/// Jeu ciblé, donc version du compilateur Papyrus dont on imite le comportement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameId {
    /// Skyrim, toutes éditions confondues — LE, Special Edition, Anniversary.
    ///
    /// Le langage y est identique ; seule la disposition des sources sur le
    /// disque diffère, ce qui relève du layout de projet et non du dialecte.
    Skyrim,
    /// Fallout 4. Ajoute les namespaces, `Struct`, `Group`, `Var`, `Is`,
    /// `Const`, `DebugOnly` / `BetaOnly` et les `CustomEvent`.
    Fallout4,
    /// Fallout 76.
    ///
    /// Jeu de mots-clés strictement identique à celui de Fallout 4 — dans
    /// Caprica, `FALLOUT76_MAX_KEYWORD == FALLOUT4_MAX_KEYWORD`. Aucun Creation
    /// Kit public ni source `.psc` extractible n'existe, donc rien n'est
    /// vérifiable : la valeur est acceptée en configuration mais ne promet rien
    /// de plus que [`GameId::Fallout4`].
    Fallout76,
    /// Starfield. Sur-ensemble de Fallout 4 : `Guard`, `LockGuard`,
    /// `TryLockGuard`, `RequiresGuard`, `ProtectsFunctionLogic` et les
    /// modificateurs d'accès `Private` / `Protected` / `SelfOnly`.
    ///
    /// Par défaut, faute de configuration : c'est le sur-ensemble le plus
    /// large, donc celui qui produit le moins de faux positifs.
    #[default]
    Starfield,
}

impl GameId {
    /// Position dans la suite des sur-ensembles.
    ///
    /// Fallout 76 partage le rang de Fallout 4 : les deux acceptent exactement
    /// le même langage.
    pub const fn rank(self) -> u8 {
        match self {
            GameId::Skyrim => 0,
            GameId::Fallout4 | GameId::Fallout76 => 1,
            GameId::Starfield => 2,
        }
    }

    /// Vrai si une construction apparue dans `since` existe dans ce jeu.
    pub const fn accepts(self, since: GameId) -> bool {
        self.rank() >= since.rank()
    }

    /// Nom canonique, tel qu'il s'écrit dans `papyrus.toml`.
    pub const fn as_str(self) -> &'static str {
        match self {
            GameId::Skyrim => "skyrim",
            GameId::Fallout4 => "fallout4",
            GameId::Fallout76 => "fallout76",
            GameId::Starfield => "starfield",
        }
    }

    /// Nom lisible, pour les messages de diagnostic.
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

/// Erreur de lecture d'un nom de jeu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownGame(pub String);

impl fmt::Display for UnknownGame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cible inconnue `{}` (attendu : skyrim, fallout4, fallout76, starfield)",
            self.0
        )
    }
}

impl std::error::Error for UnknownGame {}

impl FromStr for GameId {
    type Err = UnknownGame;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Les alias couvrent les manières dont les gens désignent ces jeux dans
        // la communauté du modding.
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

/// Construction syntaxique dont la disponibilité dépend du jeu ciblé.
///
/// Sert au second point d'intervention du dialecte : les constructions qui
/// n'ouvrent pas sur un mot-clé rétrogradable sont toujours parsées, et c'est un
/// diagnostic — pas une erreur de syntaxe — qui signale qu'elles n'existent pas
/// dans la cible. L'arbre reste donc complet et l'éditeur reste utilisable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Nom de script qualifié, `Foo:Bar:Script`.
    Namespace,
    /// Déclaration `Struct` / `EndStruct`.
    Struct,
    /// Bloc `Group` / `EndGroup`.
    Group,
    /// Opérateur `Is`.
    IsOperator,
    /// `new NomDeStruct`, sans crochets.
    NewStruct,
    /// `Property` déclarée dans un `State` ou un `Group`.
    NestedProperty,
    /// Locale immuable, `Type x = expr Const`.
    ConstLocal,
    /// Verrous : `Guard`, `LockGuard`, `TryLockGuard`.
    Guard,
    /// Modificateurs d'accès `Private`, `Protected`, `SelfOnly`.
    AccessModifier,
}

impl Feature {
    /// Jeu à partir duquel la construction existe.
    pub const fn since(self) -> GameId {
        match self {
            Feature::Namespace
            | Feature::Struct
            | Feature::Group
            | Feature::IsOperator
            | Feature::NewStruct
            | Feature::NestedProperty
            | Feature::ConstLocal => GameId::Fallout4,
            Feature::Guard | Feature::AccessModifier => GameId::Starfield,
        }
    }

    /// Description courte, destinée aux messages de diagnostic.
    pub const fn describe(self) -> &'static str {
        match self {
            Feature::Namespace => "la syntaxe de namespace",
            Feature::Struct => "les structures",
            Feature::Group => "les groupes de propriétés",
            Feature::IsOperator => "l'opérateur `Is`",
            Feature::NewStruct => "l'instanciation de structure",
            Feature::NestedProperty => "les propriétés imbriquées",
            Feature::ConstLocal => "les variables locales `Const`",
            Feature::Guard => "les verrous (`Guard`)",
            Feature::AccessModifier => "les modificateurs d'accès",
        }
    }
}

/// Cible complète : le jeu, plus les extensions non officielles activées.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dialect {
    /// Jeu ciblé.
    pub game: GameId,
    /// Extensions de compilateurs tiers, toutes désactivées par défaut.
    pub extensions: Extensions,
}

impl Dialect {
    /// Cible sans aucune extension.
    pub const fn new(game: GameId) -> Self {
        Dialect {
            game,
            extensions: Extensions::NONE,
        }
    }

    /// Vrai si la construction existe dans cette cible.
    pub const fn supports(self, feature: Feature) -> bool {
        self.game.accepts(feature.since())
    }
}

impl From<GameId> for Dialect {
    fn from(game: GameId) -> Self {
        Dialect::new(game)
    }
}

/// Extensions syntaxiques absentes des compilateurs officiels.
///
/// Caprica accepte du contrôle de flux que Bethesda n'a jamais implémenté
/// (`For`, `ForEach`, `Switch`, `Break`, `Continue`, `Do`…). Un projet compilé
/// avec Caprica est légitime, mais son code ne compilera pas avec le Creation
/// Kit : l'acceptation doit donc être un choix explicite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extensions {
    /// Contrôle de flux ajouté par Caprica.
    pub caprica_control_flow: bool,
}

impl Extensions {
    /// Aucune extension : le comportement du compilateur officiel.
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

        // `Is` n'existe pas en Skyrim : vérifié en compilant `x is Actor`
        // avec le PapyrusCompiler.exe de Skyrim SE, qui le rejette.
        assert!(!skyrim.supports(Feature::IsOperator));
        assert!(fo4.supports(Feature::IsOperator));

        // Les verrous sont propres à Starfield.
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
