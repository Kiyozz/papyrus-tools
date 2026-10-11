//! What the project's scripts declare, to follow `extends` beyond the native
//! scripts.
//!
//! A mod often derives its scripts from one of its own rather than directly
//! from `ObjectReference`: without this relay, the chain would stop at the
//! first unknown script, and the lints that depend on it would stay quiet.

use std::collections::HashMap;

use papyrus_syntax::{SyntaxKind, SyntaxNode};

/// What a script passes on to its descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSummary {
    /// The name, `MyMod:Base` for a namespaced script.
    pub name: String,
    /// The parent script, as written after `extends`.
    pub parent: Option<String>,
    /// The events it declares, `CustomEvent` included, with their case.
    pub events: Vec<String>,
}

/// The significant text of a node: non-trivia tokens, joined.
pub(crate) fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_owned())
        .collect()
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

impl ScriptSummary {
    /// The summary of a script, `None` without a readable header.
    pub fn of(root: &SyntaxNode) -> Option<ScriptSummary> {
        let header = child(root, SyntaxKind::SCRIPT_HEADER)?;
        let name = compact(&child(&header, SyntaxKind::NAME)?);
        if name.is_empty() {
            return None;
        }
        let parent = child(&header, SyntaxKind::EXTENDS_CLAUSE)
            .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
            .map(|t| compact(&t));
        let events = root
            .descendants()
            .filter(|n| match n.kind() {
                // `Event Actor.OnDeath` listens to another script.
                SyntaxKind::EVENT_DECL => child(n, SyntaxKind::TYPE_REF).is_none(),
                SyntaxKind::CUSTOM_EVENT_DECL => true,
                _ => false,
            })
            .filter_map(|n| child(&n, SyntaxKind::NAME))
            .map(|n| compact(&n))
            .collect();
        Some(ScriptSummary {
            name,
            parent,
            events,
        })
    }
}

/// The project's scripts, by name, case-insensitively.
#[derive(Debug, Default, Clone)]
pub struct Workspace {
    scripts: HashMap<String, ScriptSummary>,
}

impl Workspace {
    /// Adds or replaces a script; returns the previous version.
    pub fn insert(&mut self, summary: ScriptSummary) -> Option<ScriptSummary> {
        self.scripts
            .insert(summary.name.to_ascii_lowercase(), summary)
    }

    /// Removes the script `name`.
    pub fn remove(&mut self, name: &str) -> Option<ScriptSummary> {
        self.scripts.remove(&name.to_ascii_lowercase())
    }

    /// The script `name`.
    pub fn get(&self, name: &str) -> Option<&ScriptSummary> {
        self.scripts.get(&name.to_ascii_lowercase())
    }

    /// The number of known scripts.
    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// True if no script is known.
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}
