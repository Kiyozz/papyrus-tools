//! Ce que les scripts du projet déclarent, pour suivre `extends` au-delà des
//! scripts natifs.
//!
//! Un mod dérive souvent ses scripts d'un script à lui, et non directement de
//! `ObjectReference` : sans ce relais, la chaîne s'arrêterait au premier
//! script inconnu, et les lints qui en dépendent se tairaient.

use std::collections::HashMap;

use papyrus_syntax::{SyntaxKind, SyntaxNode};

/// Ce qu'un script transmet à ses descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptSummary {
    /// Le nom, `MyMod:Base` pour un script en namespace.
    pub name: String,
    /// Le script parent, tel qu'écrit après `extends`.
    pub parent: Option<String>,
    /// Les événements qu'il déclare, `CustomEvent` compris, avec leur casse.
    pub events: Vec<String>,
}

/// Le texte significatif d'un nœud : tokens hors trivia, accolés.
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
    /// Le résumé d'un script, `None` sans en-tête lisible.
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
                // `Event Actor.OnDeath` écoute un autre script.
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

/// Les scripts du projet, par nom, sans tenir compte de la casse.
#[derive(Debug, Default, Clone)]
pub struct Workspace {
    scripts: HashMap<String, ScriptSummary>,
}

impl Workspace {
    /// Ajoute ou remplace un script ; rend l'ancienne version.
    pub fn insert(&mut self, summary: ScriptSummary) -> Option<ScriptSummary> {
        self.scripts
            .insert(summary.name.to_ascii_lowercase(), summary)
    }

    /// Retire le script `name`.
    pub fn remove(&mut self, name: &str) -> Option<ScriptSummary> {
        self.scripts.remove(&name.to_ascii_lowercase())
    }

    /// Le script `name`.
    pub fn get(&self, name: &str) -> Option<&ScriptSummary> {
        self.scripts.get(&name.to_ascii_lowercase())
    }

    /// Le nombre de scripts connus.
    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// Vrai si aucun script n'est connu.
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }
}
