//! Navigation along the hierarchy of a symbol: its uses in the file being
//! edited, the overrides of a function, who calls it and whom it calls.

use std::collections::HashMap;

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use crate::SourceFile;
use crate::calls::callee;
use crate::ide::NavTarget;
use crate::items::{ItemKind, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, is_callee};

/// A use of a symbol in the file being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    /// The name.
    pub range: TextRange,
    /// A declaration or an assignment, rather than a read.
    pub write: bool,
}

/// A function, an event or a property of the project, in the call
/// hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallItem {
    /// What it is.
    pub def: Def,
    /// Its name.
    pub name: String,
    /// Function, event, or the property whose `Get` or `Set` calls.
    pub kind: ItemKind,
    /// Its declaration line.
    pub detail: String,
    /// Where it is declared.
    pub target: NavTarget,
}

/// Calls between two items of the call hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The caller, for incoming calls; the callee, for outgoing ones.
    pub item: CallItem,
    /// The names called, in the caller's file.
    pub ranges: Vec<TextRange>,
}

/// True for the target of an assignment: `x = 1`, `x += 1`, `a.x = 1`.
fn is_assigned(name_ref: &SyntaxNode) -> bool {
    let target = match name_ref.parent() {
        Some(field) if field.kind() == SyntaxKind::FIELD_EXPR => field,
        _ => name_ref.clone(),
    };
    target.parent().is_some_and(|stmt| {
        stmt.kind() == SyntaxKind::ASSIGN_STMT && stmt.children().next().as_ref() == Some(&target)
    })
}

/// The `NAME` or `NAME_REF` node exactly covering `range`.
fn name_node(root: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    root.covering_element(range)
        .ancestors()
        .find(|n| matches!(n.kind(), SyntaxKind::NAME | SyntaxKind::NAME_REF))
}

/// True if `name_ref` is the name of a call: `Foo()`, `a.Foo()`.
fn is_called(name_ref: &SyntaxNode) -> bool {
    is_callee(name_ref)
        || name_ref.parent().is_some_and(|p| {
            p.kind() == SyntaxKind::FIELD_EXPR
                && p.children().next().as_ref() != Some(name_ref)
                && is_callee(&p)
        })
}

impl Semantics<'_> {
    /// The uses of the symbol at `offset` in `file`, declarations included.
    pub fn highlights(&self, file: SourceFile, offset: TextSize) -> Option<Vec<Highlight>> {
        let def = self.def_at(file, offset)?;
        let root = self.root(file);
        Some(
            self.references_in(&def, &[file])
                .into_iter()
                .map(|r| Highlight {
                    range: r.range,
                    write: r.declaration
                        || name_node(&root, r.range).is_some_and(|n| is_assigned(&n)),
                })
                .collect(),
        )
    }

    // ── Overrides ─────────────────────────────────────────────────────────

    /// The declarations overriding the function or event at `offset`: in
    /// the states of its script and in the scripts deriving from it. For a
    /// native one, every declaration of the project.
    pub fn implementations(&self, file: SourceFile, offset: TextSize) -> Option<Vec<NavTarget>> {
        let def = self.def_at(file, offset)?;
        let origin = match &def {
            Def::Item { file, index } if self.item(*file, *index).kind.is_callable() => {
                Some(ScriptRef::Source(*file))
            }
            Def::NativeFunction { .. } => None,
            _ => return None,
        };
        let own = self.nav_target(&def).map(|t| (t.file, t.focus_range));
        let mut out = Vec::new();
        for reference in self.references_of(&def) {
            if !reference.declaration || Some((reference.file, reference.range)) == own {
                continue;
            }
            if let Some(origin) = origin {
                let derives = self
                    .chain(ScriptRef::Source(reference.file))
                    .any(|s| s == origin);
                if !derives {
                    continue;
                }
            }
            let index = self
                .items(reference.file)
                .items
                .iter()
                .position(|i| i.name_range == reference.range);
            if let Some(target) = index.and_then(|index| {
                self.nav_target(&Def::Item {
                    file: reference.file,
                    index,
                })
            }) {
                out.push(target);
            }
        }
        Some(out)
    }

    // ── Types ─────────────────────────────────────────────────────────────

    /// The script at `offset`, to start a type hierarchy: the one named
    /// there, else the file's own.
    pub fn script_at(&self, file: SourceFile, offset: TextSize) -> Option<SourceFile> {
        match self.def_at(file, offset) {
            Some(Def::Script { file }) => Some(file),
            Some(Def::NativeScript(_)) => None,
            _ => (!self.items(file).name.is_empty()).then_some(file),
        }
    }

    /// The parent of a script, when it has a source.
    pub fn supertypes(&self, file: SourceFile) -> Vec<SourceFile> {
        match self.chain(ScriptRef::Source(file)).nth(1) {
            Some(ScriptRef::Source(parent)) => vec![parent],
            _ => Vec::new(),
        }
    }

    /// The scripts extending `file` directly.
    pub fn subtypes(&self, file: SourceFile) -> Vec<SourceFile> {
        self.index()
            .files()
            .filter(|&other| {
                other != file
                    && self.chain(ScriptRef::Source(other)).nth(1) == Some(ScriptRef::Source(file))
            })
            .collect()
    }

    // ── Calls ─────────────────────────────────────────────────────────────

    /// `def` in the call hierarchy: a function, an event or a property
    /// declared in source.
    pub fn call_item(&self, def: &Def) -> Option<CallItem> {
        let Def::Item { file, index } = def else {
            return None;
        };
        let item = self.item(*file, *index);
        if !matches!(
            item.kind,
            ItemKind::Function | ItemKind::Event | ItemKind::Property
        ) {
            return None;
        }
        Some(CallItem {
            def: def.clone(),
            name: item.name.clone(),
            kind: item.kind,
            detail: item.signature.clone(),
            target: self.nav_target(def)?,
        })
    }

    /// The function or event at `offset`, to start a call hierarchy.
    pub fn prepare_call_hierarchy(&self, file: SourceFile, offset: TextSize) -> Option<CallItem> {
        let item = self.call_item(&self.def_at(file, offset)?)?;
        (item.kind != ItemKind::Property).then_some(item)
    }

    /// The item declaring the body around `node`: its function or event,
    /// or the property of a `Get` or `Set`.
    fn enclosing_item(&self, file: SourceFile, node: &SyntaxNode) -> Option<Def> {
        let items = self.items(file);
        node.ancestors()
            .filter(|n| {
                matches!(
                    n.kind(),
                    SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL | SyntaxKind::PROPERTY_DECL
                )
            })
            .find_map(|decl| {
                let name = decl.children().find(|c| c.kind() == SyntaxKind::NAME)?;
                let range = trimmed(&name);
                let index = items.items.iter().position(|i| i.name_range == range)?;
                Some(Def::Item { file, index })
            })
    }

    /// Who calls `def`, grouped by caller.
    pub fn incoming_calls(&self, def: &Def) -> Vec<Call> {
        let mut calls: Vec<Call> = Vec::new();
        let mut roots: HashMap<SourceFile, SyntaxNode> = HashMap::new();
        for reference in self.references_of(def) {
            if reference.declaration {
                continue;
            }
            let root = roots
                .entry(reference.file)
                .or_insert_with(|| self.root(reference.file));
            let Some(name_ref) = name_node(root, reference.range).filter(is_called) else {
                continue;
            };
            let Some(caller) = self
                .enclosing_item(reference.file, &name_ref)
                .and_then(|d| self.call_item(&d))
            else {
                continue;
            };
            push_call(&mut calls, caller, reference.range);
        }
        calls
    }

    /// Whom `def` calls, grouped by callee: the functions and events of the
    /// project only, the native ones having no source.
    pub fn outgoing_calls(&self, def: &Def) -> Vec<Call> {
        let Some(target) = self.nav_target(def) else {
            return Vec::new();
        };
        let root = self.root(target.file);
        let Some(decl) = root
            .covering_element(target.full_range)
            .ancestors()
            .find(|n| {
                matches!(
                    n.kind(),
                    SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL | SyntaxKind::PROPERTY_DECL
                )
            })
        else {
            return Vec::new();
        };
        let mut calls: Vec<Call> = Vec::new();
        for call in decl
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::CALL_EXPR)
        {
            let Some(name) = callee(&call) else {
                continue;
            };
            let Some(callee) = self
                .resolve_name_ref(target.file, &name)
                .and_then(|d| self.call_item(&d))
            else {
                continue;
            };
            push_call(&mut calls, callee, trimmed(&name));
        }
        calls
    }
}

/// Adds a call to `item`, merging with the calls already found.
fn push_call(calls: &mut Vec<Call>, item: CallItem, range: TextRange) {
    match calls.iter_mut().find(|c| c.item.def == item.def) {
        Some(call) => call.ranges.push(range),
        None => calls.push(Call {
            item,
            ranges: vec![range],
        }),
    }
}
