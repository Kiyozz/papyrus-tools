//! `PAP004 events/unknown` : un événement que le moteur n'appellera jamais.
//!
//! Le compilateur accepte `Event OnActivatee()` sans broncher : c'est une
//! déclaration comme une autre, simplement jamais appelée. Rien ne le signale,
//! ni à la compilation ni en jeu.
//!
//! Le moteur envoie un événement, par son nom, à tous les scripts de l'objet
//! qui le reçoit : un script `extends ObjectReference` posé sur un acteur
//! reçoit `OnDeath`, que seul `Actor` déclare. Un script reçoit donc les
//! événements de sa chaîne `extends`, ceux des types qui en dérivent, et ceux
//! que relaient les alias et les effets magiques : un `ReferenceAlias` reçoit
//! ceux de sa référence, un `ActiveMagicEffect` ceux de son acteur.
//!
//! Le lint ne parle que lorsqu'il est sûr de lui :
//!
//! - la chaîne `extends` est connue jusqu'à sa racine, par les scripts natifs
//!   du jeu ou par ceux du projet ;
//! - le nom ressemble à un événement que le script reçoit — une faute de
//!   frappe —, ou c'est un événement du moteur qu'aucun objet portant ce
//!   script ne recevra, comme `OnCombatStateChanged` sur une quête.
//!
//! Un nom sans rapport avec aucun événement connu peut être le callback d'un
//! `RegisterForModEvent`, ou un événement que le script s'envoie lui-même : il
//! est laissé tranquille. Un nom écrit dans une chaîne du script l'est aussi.

use std::collections::HashSet;

use papyrus_dialect::GameId;
use papyrus_syntax::{SyntaxKind, SyntaxNode};

use crate::workspace::compact;
use crate::{Context, EVENTS_UNKNOWN, Edit, Finding, Suggestion, distance};

/// Les événements que tout script reçoit, déclarés nulle part.
const UNIVERSAL: &[&str] = &["OnInit", "OnBeginState", "OnEndState"];

/// Les scripts qui reçoivent les événements d'un autre objet.
const RELAYS: &[(&str, &str)] = &[
    ("ActiveMagicEffect", "Actor"),
    ("ReferenceAlias", "ObjectReference"),
    ("RefCollectionAlias", "ObjectReference"),
];

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

fn is_a(game: GameId, script: &str, ancestor: &str) -> bool {
    papyrus_stdlib::ancestors(game, script).any(|a| a.name.eq_ignore_ascii_case(ancestor))
}

/// Les événements de `native`, de ses ancêtres et des scripts qui en
/// dérivent.
fn family(game: GameId, native: &str, events: &mut Vec<String>) {
    for script in papyrus_stdlib::ancestors(game, native) {
        events.extend(script.events.iter().map(|e| e.name.to_owned()));
    }
    for script in papyrus_stdlib::scripts(game) {
        if is_a(game, script.name, native) {
            events.extend(script.events.iter().map(|e| e.name.to_owned()));
        }
    }
}

/// Les événements qui peuvent atteindre un script dérivé de `start`, ou
/// `None` si un maillon de la chaîne est inconnu.
fn reachable(context: &Context<'_>, game: GameId, start: &str) -> Option<Vec<String>> {
    let mut events = Vec::new();
    let mut first_native = None;
    let mut current = Some(start.to_owned());
    let mut steps = 0;
    while let Some(name) = current.take() {
        steps += 1;
        if steps > 64 {
            return None;
        }
        // Le projet peut fournir sa version d'un script natif, celle d'un
        // script extender par exemple : les deux comptent.
        let user = context.workspace.get(&name);
        let native = papyrus_stdlib::script(game, &name);
        if user.is_none() && native.is_none() {
            return None;
        }
        if let Some(user) = user {
            events.extend(user.events.iter().cloned());
        }
        if let Some(native) = native {
            first_native.get_or_insert(native.name);
        }
        current = user
            .and_then(|u| u.parent.clone())
            .or_else(|| native.and_then(|n| n.parent).map(str::to_owned));
        // Depuis Fallout 4, tout script dérive de `ScriptObject`.
        if current.is_none() && game.rank() >= 1 && !name.eq_ignore_ascii_case("ScriptObject") {
            current = Some("ScriptObject".to_owned());
        }
    }
    if let Some(native) = first_native {
        family(game, native, &mut events);
        for (relay, source) in RELAYS {
            if is_a(game, native, relay) {
                family(game, source, &mut events);
            }
        }
    }
    Some(events)
}

/// L'événement connu le plus proche de `name`, s'il l'est assez pour une
/// faute de frappe.
fn closest<'a>(name: &str, known: &'a [String]) -> Option<&'a str> {
    known
        .iter()
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(d, _)| *d <= 2 && *d * 4 <= name.len())
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate.as_str())
}

/// Les scripts natifs qui déclarent l'événement `name` sans l'hériter, à la
/// façon d'une énumération : « `Actor`, `ActiveMagicEffect` et
/// `ReferenceAlias` ».
fn owners(game: GameId, name: &str) -> Option<String> {
    let owners: Vec<String> = papyrus_stdlib::scripts(game)
        .iter()
        .filter(|s| s.events.iter().any(|e| e.name.eq_ignore_ascii_case(name)))
        .filter(|s| {
            s.parent
                .is_none_or(|p| papyrus_stdlib::find_event(game, p, name).is_none())
        })
        .map(|s| format!("`{}`", s.name))
        .collect();
    match owners.as_slice() {
        [] => None,
        [one] => Some(one.clone()),
        [init @ .., last] => Some(format!("{} et {last}", init.join(", "))),
    }
}

/// Le contenu des chaînes du script, en minuscules.
fn string_contents(root: &SyntaxNode) -> HashSet<String> {
    root.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == SyntaxKind::STRING)
        .map(|t| t.text().trim_matches('"').to_ascii_lowercase())
        .collect()
}

pub(super) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    let Some(game) = context.game else {
        return;
    };
    let root = context.root;
    let Some(header) = child(root, SyntaxKind::SCRIPT_HEADER) else {
        return;
    };
    // Un script du moteur déclare ses événements ; il ne les reçoit pas.
    let own_name = child(&header, SyntaxKind::NAME).map(|n| compact(&n));
    if own_name.is_some_and(|n| papyrus_stdlib::script(game, &n).is_some()) {
        return;
    }
    let parent = child(&header, SyntaxKind::EXTENDS_CLAUSE)
        .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
        .map(|t| compact(&t))
        .or_else(|| (game.rank() >= 1).then(|| "ScriptObject".to_owned()));

    let mut literals = None;
    let mut own = None;
    for event in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::EVENT_DECL)
    {
        let Some(name_node) = child(&event, SyntaxKind::NAME) else {
            continue;
        };
        let name = compact(&name_node);
        if name.is_empty() || UNIVERSAL.iter().any(|u| u.eq_ignore_ascii_case(&name)) {
            continue;
        }

        // `Event Actor.OnDeath` : les événements d'un autre script.
        let (target, known) = match child(&event, SyntaxKind::TYPE_REF) {
            Some(remote) => {
                let target = compact(&remote);
                let known = reachable(context, game, &target);
                (target, known)
            }
            None => {
                let Some(parent) = &parent else {
                    continue;
                };
                let known = own
                    .get_or_insert_with(|| reachable(context, game, parent))
                    .clone();
                (parent.clone(), known)
            }
        };
        let Some(known) = known else {
            continue;
        };
        if known.iter().any(|k| k.eq_ignore_ascii_case(&name)) {
            continue;
        }
        if literals
            .get_or_insert_with(|| string_contents(root))
            .contains(&name.to_ascii_lowercase())
        {
            continue;
        }

        let range = name_node.text_range();
        if let Some(best) = closest(&name, &known) {
            findings.push(Finding {
                lint: &EVENTS_UNKNOWN,
                range,
                message: format!(
                    "`{target}` ne reçoit aucun événement `{name}` : le moteur ne l'appellera \
                     jamais — vouliez-vous dire `{best}` ?"
                ),
                fix: None,
                suggestion: Some(Suggestion {
                    title: format!("Renommer en `{best}`"),
                    edits: vec![Edit {
                        range,
                        text: best.to_owned(),
                    }],
                }),
            });
        } else if let Some(owners) = owners(game, &name) {
            findings.push(Finding {
                lint: &EVENTS_UNKNOWN,
                range,
                message: format!(
                    "`{target}` ne reçoit pas `{name}`, qui n'est envoyé qu'à {owners} : le \
                     moteur ne l'appellera jamais sur ce script"
                ),
                fix: None,
                suggestion: None,
            });
        }
    }
}
