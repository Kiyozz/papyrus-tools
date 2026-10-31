//! Functions that end up calling themselves, directly or through others,
//! across files.
//!
//! Which function a call runs depends on the object and on its state, so
//! calls are followed by what they name: an override, or a function in a
//! state, counts as the function it overrides. `Parent.F()` does not: it
//! runs the parent's. Events reached through the engine are not calls:
//! `RegisterForSingleUpdate` in `OnUpdate` recurses through nothing here.
//!
//! Recursion is fine. Only the kind that cannot end is reported: a function
//! that comes back on every path, with nothing a state or an override could
//! change.

use std::collections::HashSet;

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange};

use crate::SourceFile;
use crate::calls::callee;
use crate::check::{Diagnostic, Problem};
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, Semantics};

/// The most functions followed from one call: a call graph that large says
/// nothing an editor should wait for.
const MAX_FOLLOWED: usize = 2_000;

/// A call that comes back to the function holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recursion {
    /// The name of the function holding the call.
    pub function: TextRange,
    /// The call.
    pub call: TextRange,
    /// The function it goes through; `None` when it calls itself.
    pub through: Option<String>,
}

/// The kinds of statements holding other statements.
fn branches(stmt: &SyntaxNode) -> Vec<SyntaxNode> {
    let mut out: Vec<SyntaxNode> = stmt
        .children()
        .filter(|c| c.kind() == SyntaxKind::STMT_LIST)
        .collect();
    for clause in stmt.children().filter(|c| {
        matches!(
            c.kind(),
            SyntaxKind::ELSEIF_CLAUSE | SyntaxKind::ELSE_CLAUSE
        )
    }) {
        out.extend(
            clause
                .children()
                .filter(|c| c.kind() == SyntaxKind::STMT_LIST),
        );
    }
    out
}

/// True if `call` may be skipped: on the right of `&&` or `||`.
fn short_circuited(call: &SyntaxNode, top: &SyntaxNode) -> bool {
    let mut node = call.clone();
    while &node != top {
        let Some(parent) = node.parent() else {
            return false;
        };
        let lazy = parent.kind() == SyntaxKind::BIN_EXPR
            && parent
                .children_with_tokens()
                .any(|e| matches!(e.kind(), SyntaxKind::AMP2 | SyntaxKind::PIPE2))
            && parent.children().next().as_ref() != Some(&node);
        if lazy {
            return true;
        }
        node = parent;
    }
    false
}

impl Semantics<'_> {
    /// The declaration of a function or an event in source.
    fn callable_decl(&self, def: &Def) -> Option<(SourceFile, SyntaxNode)> {
        let Def::Item { file, index } = def else {
            return None;
        };
        if !matches!(
            self.item(*file, *index).kind,
            ItemKind::Function | ItemKind::Event
        ) {
            return None;
        }
        let target = self.nav_target(def)?;
        let decl = self
            .root(target.file)
            .covering_element(target.full_range)
            .ancestors()
            .find(|n| matches!(n.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL))?;
        Some((target.file, decl))
    }

    /// What a call runs, unless it is `Parent.F()`.
    fn called(&self, file: SourceFile, call: &SyntaxNode) -> Option<Def> {
        let name = callee(call)?;
        let on_parent = name
            .parent()
            .filter(|p| p.kind() == SyntaxKind::FIELD_EXPR)
            .and_then(|p| p.children().next())
            .and_then(|receiver| receiver.first_token())
            .is_some_and(|t| t.kind() == SyntaxKind::PARENT_KW);
        if on_parent {
            return None;
        }
        self.resolve_name_ref(file, &name)
    }

    /// The calls of a declaration, with what they run.
    fn calls_in(&self, file: SourceFile, decl: &SyntaxNode) -> Vec<(SyntaxNode, Def)> {
        decl.descendants()
            .filter(|n| n.kind() == SyntaxKind::CALL_EXPR)
            .filter_map(|call| Some((call.clone(), self.called(file, &call)?)))
            .collect()
    }

    /// True if `from` leads to `target`.
    fn reaches(&self, from: &Def, target: &Def, seen: &mut HashSet<(SourceFile, usize)>) -> bool {
        if self.canonical(from) == *target {
            return true;
        }
        let Def::Item { file, index } = from else {
            return false;
        };
        if seen.len() >= MAX_FOLLOWED || !seen.insert((*file, *index)) {
            return false;
        }
        let Some((file, decl)) = self.callable_decl(from) else {
            return false;
        };
        self.calls_in(file, &decl)
            .iter()
            .any(|(_, def)| self.reaches(def, target, seen))
    }

    /// The calls of `file` coming back to the function holding them.
    pub fn recursions(&self, file: SourceFile) -> Vec<Recursion> {
        let items = self.items(file);
        let root = self.root(file);
        let mut out = Vec::new();
        for decl in root
            .descendants()
            .filter(|n| matches!(n.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL))
        {
            let Some(name) = decl.children().find(|c| c.kind() == SyntaxKind::NAME) else {
                continue;
            };
            let function = trimmed(&name);
            let Some(index) = items.items.iter().position(|i| i.name_range == function) else {
                continue;
            };
            let target = self.canonical(&Def::Item { file, index });
            for (call, def) in self.calls_in(file, &decl) {
                let direct = self.canonical(&def) == target;
                if direct || self.reaches(&def, &target, &mut HashSet::new()) {
                    out.push(Recursion {
                        function,
                        call: trimmed(&call),
                        through: (!direct).then(|| self.def_name(&def).to_owned()),
                    });
                }
            }
        }
        out
    }

    /// `F` that comes back to itself on every path, with nothing to stop it.
    pub(crate) fn check_infinite_recursion(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        body: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(name) = decl.children().find(|c| c.kind() == SyntaxKind::NAME) else {
            return;
        };
        let range = trimmed(&name);
        let items = self.items(file);
        let Some(index) = items.items.iter().position(|i| i.name_range == range) else {
            return;
        };
        let def = Def::Item { file, index };
        let mut stack = vec![self.canonical(&def)];
        if !self.list_loops(file, body, &mut stack) {
            return;
        }
        // A state or an override may end it, and `GotoState` may reach them.
        for def in &stack {
            let Some((file, decl)) = self.callable_decl(def) else {
                return;
            };
            let Def::Item { index, .. } = def else {
                return;
            };
            let name_range = self.item(file, *index).name_range;
            let overridden = self
                .implementations(file, name_range.start())
                .is_none_or(|found| !found.is_empty());
            let in_state = self.item(file, *index).container.is_some();
            let moves = decl
                .descendants()
                .filter(|n| n.kind() == SyntaxKind::CALL_EXPR)
                .filter_map(|call| callee(&call))
                .any(|callee| compact(&callee).eq_ignore_ascii_case("GotoState"));
            if overridden || in_state || moves {
                return;
            }
        }
        let name = compact(&name);
        let message = match stack.get(1) {
            Some(through) => format!(
                "`{name}` comes back to itself through `{}` on every path: it never returns",
                self.def_name(through)
            ),
            None => format!("`{name}` calls itself on every path: it never returns"),
        };
        out.push(Diagnostic {
            problem: Problem::InfiniteRecursion,
            range,
            message,
            fixes: Vec::new(),
        });
    }

    /// True if running `list` always comes back to a function of `stack`.
    fn list_loops(&self, file: SourceFile, list: &SyntaxNode, stack: &mut Vec<Def>) -> bool {
        for stmt in list.children() {
            let inner = branches(&stmt);
            // What always runs: the condition, or the statement itself.
            let head = if inner.is_empty() {
                Some(stmt.clone())
            } else {
                stmt.children().next()
            };
            if head.is_some_and(|head| self.expr_loops(file, &head, stack)) {
                return true;
            }
            if stmt.kind() == SyntaxKind::RETURN_STMT {
                return false;
            }
            if stmt.kind() == SyntaxKind::IF_STMT {
                let has_else = stmt.children().any(|c| c.kind() == SyntaxKind::ELSE_CLAUSE);
                let depth = stack.len();
                if has_else && inner.iter().all(|b| self.list_loops(file, b, stack)) {
                    return true;
                }
                stack.truncate(depth);
            }
            // A way out, which something decides.
            let leaves = inner
                .iter()
                .any(|b| b.descendants().any(|n| n.kind() == SyntaxKind::RETURN_STMT));
            if leaves {
                return false;
            }
        }
        false
    }

    /// True if evaluating `top` always comes back to a function of `stack`.
    fn expr_loops(&self, file: SourceFile, top: &SyntaxNode, stack: &mut Vec<Def>) -> bool {
        for call in top
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::CALL_EXPR)
        {
            if short_circuited(&call, top) {
                continue;
            }
            // On this object: another one may stop anywhere.
            let own_object = call
                .children()
                .next()
                .filter(|first| first.kind() == SyntaxKind::FIELD_EXPR)
                .and_then(|field| field.children().next())
                .is_none_or(|receiver| {
                    receiver
                        .first_token()
                        .is_some_and(|t| t.kind() == SyntaxKind::SELF_KW)
                });
            if !own_object {
                continue;
            }
            let Some(def) = self.called(file, &call) else {
                continue;
            };
            let target = self.canonical(&def);
            if stack.contains(&target) {
                return true;
            }
            let Some((callee_file, decl)) = self.callable_decl(&target) else {
                continue;
            };
            let Some(body) = decl.children().find(|c| c.kind() == SyntaxKind::STMT_LIST) else {
                continue;
            };
            stack.push(target);
            if self.list_loops(callee_file, &body, stack) {
                return true;
            }
            stack.pop();
        }
        false
    }
}
