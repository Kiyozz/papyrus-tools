//! `PAP004 events/unknown`: an event the engine will never call.
//!
//! The compiler accepts `Event OnActivatee()` without a word: it is a
//! declaration like any other, simply never called. Nothing reports it,
//! neither at compile time nor in game.
//!
//! The engine sends an event, by name, to every script on the object that
//! receives it: an `extends ObjectReference` script attached to an actor
//! receives `OnDeath`, which only `Actor` declares. A script therefore
//! receives the events of its `extends` chain, those of the types deriving
//! from it, and those relayed by aliases and magic effects: a
//! `ReferenceAlias` receives its reference's, an `ActiveMagicEffect` its
//! actor's.
//!
//! The lint only speaks when it is sure:
//!
//! - the `extends` chain is known down to its root, through the game's
//!   native scripts or the project's own;
//! - the name looks like an event the script receives — a typo —, or it is
//!   an engine event no object carrying this script will receive, such as
//!   `OnCombatStateChanged` on a quest.
//!
//! A name unrelated to any known event may be a `RegisterForModEvent`
//! callback, or an event the script sends itself: it is left alone. So is a
//! name written in a string of the script.

use std::collections::HashSet;

use papyrus_dialect::GameId;
use papyrus_syntax::{SyntaxKind, SyntaxNode};

use crate::workspace::compact;
use crate::{Context, EVENTS_UNKNOWN, Edit, Finding, Suggestion, distance};

/// The events every script receives, declared nowhere.
const UNIVERSAL: &[&str] = &["OnInit", "OnBeginState", "OnEndState"];

/// The scripts that receive the events of another object.
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

/// The events of `native`, of its ancestors and of the scripts deriving from
/// it.
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

/// The events that can reach a script deriving from `start`, or `None` if a
/// link of the chain is unknown.
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
        // The project may provide its own version of a native script, a
        // script extender's for instance: both count.
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
        // Since Fallout 4, every script derives from `ScriptObject`.
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

/// The known event closest to `name`, if close enough to be a typo.
fn closest<'a>(name: &str, known: &'a [String]) -> Option<&'a str> {
    known
        .iter()
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(d, _)| *d <= 2 && *d * 4 <= name.len())
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate.as_str())
}

/// The native scripts that declare the event `name` without inheriting it,
/// as an enumeration: « `Actor`, `ActiveMagicEffect` et `ReferenceAlias` ».
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

/// The contents of the script's strings, lowercased.
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
    // An engine script declares its events; it does not receive them.
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

        // `Event Actor.OnDeath`: the events of another script.
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
