//! Les signatures des scripts natifs de chaque jeu, embarquées.
//!
//! De quoi raisonner sur `extends ObjectReference` sans jeu installé : quels
//! événements un script peut recevoir, quelles fonctions il hérite. Les
//! tables sont générées depuis les sources vanilla par
//! `cargo xtask stdlib`, et ne contiennent que des **signatures** — ni corps
//! de fonction, ni docstring.
//!
//! Seuls les scripts *natifs* y figurent : ceux que le moteur implémente, qui
//! portent le flag `Native` ou déclarent des fonctions natives, et ceux qui ne
//! font que déclarer — les types sans fonction native, comme `Activator` en
//! Skyrim, dont le `.psc` tient en une ligne. Ce sont eux qui déclarent les
//! types et les événements du moteur.

#![forbid(unsafe_code)]

mod generated;

use std::cmp::Ordering;

use papyrus_dialect::GameId;

/// Un paramètre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    /// Le nom, `akActionRef`.
    pub name: &'static str,
    /// Le type, tel qu'écrit : `ObjectReference`, `int[]`.
    pub ty: &'static str,
    /// La valeur par défaut, telle qu'écrite : `None`, `-1`.
    pub default: Option<&'static str>,
}

/// Une fonction ou un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Function {
    /// Le nom, avec la casse du vanilla.
    pub name: &'static str,
    /// Le type de retour, `None` pour aucun.
    pub returns: Option<&'static str>,
    /// Les paramètres.
    pub params: &'static [Param],
    /// `Global` : s'appelle sur le script, pas sur une instance.
    pub global: bool,
    /// `Native` : implémentée par le moteur.
    pub native: bool,
}

/// Un script natif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Script {
    /// Le nom, avec la casse du vanilla.
    pub name: &'static str,
    /// Le script parent, `None` pour la racine.
    pub parent: Option<&'static str>,
    /// Les fonctions, triées par nom sans tenir compte de la casse.
    pub functions: &'static [Function],
    /// Les événements, triés de même.
    pub events: &'static [Function],
}

/// Compare deux noms comme Papyrus : sans tenir compte de la casse.
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

/// Les scripts natifs de `game`, triés par nom.
pub fn scripts(game: GameId) -> &'static [Script] {
    match game {
        GameId::Skyrim => generated::skyrim::SCRIPTS,
        // Fallout 76 n'a ni Creation Kit public ni sources : ses scripts de
        // base sont ceux de Fallout 4.
        GameId::Fallout4 | GameId::Fallout76 => generated::fallout4::SCRIPTS,
        GameId::Starfield => generated::starfield::SCRIPTS,
    }
}

/// Le script natif `name`, sans tenir compte de la casse.
pub fn script(game: GameId, name: &str) -> Option<&'static Script> {
    find(scripts(game), name, |s| s.name)
}

/// `name` et ses ancêtres natifs, du plus proche au plus lointain.
pub fn ancestors(game: GameId, name: &str) -> impl Iterator<Item = &'static Script> {
    let mut next = script(game, name);
    // Garde-fou contre un cycle, que des tables mal générées pourraient créer.
    let mut steps = 0;
    std::iter::from_fn(move || {
        let current = next?;
        steps += 1;
        next = if steps < 64 {
            current.parent.and_then(|p| script(game, p))
        } else {
            None
        };
        Some(current)
    })
}

/// L'événement `event` que reçoit un script dérivé de `name`, en remontant
/// la hiérarchie.
pub fn find_event(game: GameId, name: &str, event: &str) -> Option<&'static Function> {
    ancestors(game, name).find_map(|s| find(s.events, event, |f| f.name))
}

/// La fonction `function` héritée de `name`.
pub fn find_function(game: GameId, name: &str, function: &str) -> Option<&'static Function> {
    ancestors(game, name).find_map(|s| find(s.functions, function, |f| f.name))
}

/// Les noms connus d'un jeu, en minuscules, pour les recherches en masse.
#[derive(Debug, Default)]
struct Names {
    functions: std::collections::HashSet<String>,
    events: std::collections::HashSet<String>,
}

fn names(game: GameId) -> &'static Names {
    use std::sync::OnceLock;
    static TABLES: [OnceLock<Names>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let slot = match game {
        GameId::Skyrim => 0,
        GameId::Fallout4 | GameId::Fallout76 => 1,
        GameId::Starfield => 2,
    };
    TABLES[slot].get_or_init(|| {
        let mut names = Names::default();
        for script in scripts(game) {
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

/// Vrai si un script natif de `game` déclare une fonction `name`.
pub fn has_function(game: GameId, name: &str) -> bool {
    names(game).functions.contains(&name.to_ascii_lowercase())
}

/// Vrai si un script natif de `game` déclare un événement `name`.
pub fn has_event(game: GameId, name: &str) -> bool {
    names(game).events.contains(&name.to_ascii_lowercase())
}
