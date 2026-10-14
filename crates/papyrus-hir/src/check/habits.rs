//! What compiles but should not stay: modders' habits. Repeating updates,
//! `Game.GetPlayer()` in a loop, booleans compared with `True`, a value
//! assigned or compared to itself, script variables never read, imports
//! nothing uses.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::{Diagnostic, Fix, Problem, Replacement, child};
use crate::SourceFile;
use crate::items::{ItemKind, compact, has_flag, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, member_name_ref};
use crate::ty::Ty;

fn operator(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE)
}

/// Whether evaluating `node` cannot do anything: no call in it.
fn pure(node: &SyntaxNode) -> bool {
    !node
        .descendants()
        .any(|n| matches!(n.kind(), SyntaxKind::CALL_EXPR | SyntaxKind::NEW_EXPR))
}

/// The callee of a call, `Foo` in `x.Foo()`.
fn callee_name(call: &SyntaxNode) -> Option<SyntaxNode> {
    match call.children().next()? {
        c if c.kind() == SyntaxKind::FIELD_EXPR => member_name_ref(&c),
        c if c.kind() == SyntaxKind::NAME_REF => Some(c),
        _ => None,
    }
}

impl Semantics<'_> {
    /// The habits of a node of a body.
    pub(super) fn check_habits(
        &self,
        file: SourceFile,
        node: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        match node.kind() {
            SyntaxKind::CALL_EXPR => self.check_call_habits(file, node, out),
            SyntaxKind::BIN_EXPR => self.check_comparison(file, node, out),
            SyntaxKind::ASSIGN_STMT => {
                let plain = node
                    .children_with_tokens()
                    .any(|e| e.kind() == SyntaxKind::EQ);
                let mut nodes = node.children();
                let (Some(target), Some(value)) = (nodes.next(), nodes.next()) else {
                    return;
                };
                if plain && pure(&target) && compact(&target).eq_ignore_ascii_case(&compact(&value))
                {
                    out.push(Diagnostic {
                        problem: Problem::SelfAssign,
                        range: trimmed(node),
                        message: format!(
                            "`{}` is assigned to itself: this does nothing",
                            compact(&target)
                        ),
                        fixes: Vec::new(),
                    });
                }
            }
            _ => {}
        }
    }

    /// `RegisterForUpdate`, `Game.GetPlayer()` in a loop.
    fn check_call_habits(&self, file: SourceFile, call: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(callee) = callee_name(call) else {
            return;
        };
        // The engine's functions: embedded, or from the game's sources.
        let (script, name) = match self.resolve_name_ref(file, &callee) {
            Some(Def::NativeFunction {
                script, function, ..
            }) => (script.name, function.name),
            Some(Def::Item { file, index }) if self.item(file, index).native => (
                self.items(file).name.as_str(),
                self.item(file, index).name.as_str(),
            ),
            _ => return,
        };
        let single = if name.eq_ignore_ascii_case("RegisterForUpdate") {
            Some("RegisterForSingleUpdate")
        } else if name.eq_ignore_ascii_case("RegisterForUpdateGameTime") {
            Some("RegisterForSingleUpdateGameTime")
        } else {
            None
        };
        if let Some(single) = single.filter(|_| self.skyrim()) {
            out.push(Diagnostic {
                problem: Problem::RepeatingUpdate,
                range: trimmed(&callee),
                message: format!(
                    "`{name}` keeps firing, even in saves the mod was removed from: prefer \
                     `{single}`, registered again at the end of the event"
                ),
                fixes: vec![Fix {
                    title: format!("Use `{single}`"),
                    edits: vec![Replacement {
                        range: trimmed(&callee),
                        text: single.to_owned(),
                    }],
                }],
            });
        }
        if script.eq_ignore_ascii_case("Game")
            && name.eq_ignore_ascii_case("GetPlayer")
            && call
                .ancestors()
                .take_while(|n| {
                    !matches!(n.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL)
                })
                .any(|n| n.kind() == SyntaxKind::WHILE_STMT)
        {
            out.push(Diagnostic {
                problem: Problem::GetPlayerInLoop,
                range: trimmed(call),
                message: "`Game.GetPlayer()` is called on each iteration: keep the player in a \
                          local before the loop, or in a `PlayerRef` property"
                    .to_owned(),
                fixes: Vec::new(),
            });
        }
    }

    /// `b == True`, `x == x`.
    fn check_comparison(&self, file: SourceFile, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(op) = operator(node).map(|t| t.kind()) else {
            return;
        };
        let operands: Vec<SyntaxNode> = node.children().collect();
        let [left, right] = &operands[..] else {
            return;
        };
        let comparison = matches!(
            op,
            SyntaxKind::EQ2
                | SyntaxKind::NEQ
                | SyntaxKind::LT
                | SyntaxKind::GT
                | SyntaxKind::LTEQ
                | SyntaxKind::GTEQ
        );
        if !comparison {
            return;
        }
        if pure(left) && compact(left).eq_ignore_ascii_case(&compact(right)) {
            let answer = match op {
                SyntaxKind::EQ2 | SyntaxKind::LTEQ | SyntaxKind::GTEQ => "`True`",
                _ => "`False`",
            };
            out.push(Diagnostic {
                problem: Problem::SelfCompare,
                range: trimmed(node),
                message: format!(
                    "`{}` is compared with itself: always {answer}",
                    compact(left)
                ),
                fixes: Vec::new(),
            });
            return;
        }
        if !matches!(op, SyntaxKind::EQ2 | SyntaxKind::NEQ) {
            return;
        }
        let literal = |n: &SyntaxNode| {
            (n.kind() == SyntaxKind::LITERAL)
                .then(|| n.first_token())
                .flatten()
                .and_then(|t| match t.kind() {
                    SyntaxKind::TRUE_KW => Some(true),
                    SyntaxKind::FALSE_KW => Some(false),
                    _ => None,
                })
        };
        let (value, constant) = match (literal(left), literal(right)) {
            (None, Some(c)) => (left, c),
            (Some(c), None) => (right, c),
            _ => return,
        };
        if self.type_of(file, value) != Ty::Bool {
            return;
        }
        let keep = constant == (op == SyntaxKind::EQ2);
        let text = compact(value);
        let simple = matches!(
            value.kind(),
            SyntaxKind::NAME_REF
                | SyntaxKind::CALL_EXPR
                | SyntaxKind::FIELD_EXPR
                | SyntaxKind::INDEX_EXPR
                | SyntaxKind::PAREN_EXPR
                | SyntaxKind::LITERAL
        );
        let source = value.text().to_string();
        let replacement = match (keep, simple) {
            (true, _) => source.trim().to_owned(),
            (false, true) => format!("!{}", source.trim()),
            (false, false) => format!("!({})", source.trim()),
        };
        out.push(Diagnostic {
            problem: Problem::BoolComparison,
            range: trimmed(node),
            message: format!("`{text}` is already a `bool`: write `{replacement}`"),
            fixes: vec![Fix {
                title: format!("Write `{replacement}`"),
                edits: vec![Replacement {
                    range: trimmed(node),
                    text: replacement,
                }],
            }],
        });
    }

    /// Script variables never read, and imports nothing uses.
    pub(super) fn check_unused_script_level(
        &self,
        file: SourceFile,
        root: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let items = self.items(file);
        let variables: Vec<(usize, SyntaxNode)> = root
            .children()
            .filter(|n| n.kind() == SyntaxKind::VARIABLE_DECL)
            // The game's conditions read `Conditional` variables.
            .filter(|n| !has_flag(n, SyntaxKind::CONDITIONAL_KW))
            .filter_map(|n| {
                let name = trimmed(&child(&n, SyntaxKind::NAME)?);
                let index = items
                    .items
                    .iter()
                    .position(|i| i.kind == ItemKind::Variable && i.name_range == name)?;
                Some((index, n))
            })
            .collect();
        let name_refs: Vec<SyntaxNode> = root
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::NAME_REF)
            .collect();
        for (index, decl) in variables {
            let name = &items.items[index].name;
            let read = name_refs.iter().any(|r| {
                compact(r).eq_ignore_ascii_case(name)
                    && !Self::written_only(r)
                    && self.resolve_name_ref(file, r) == Some(Def::Item { file, index })
            });
            if !read {
                let range =
                    child(&decl, SyntaxKind::NAME).map_or_else(|| trimmed(&decl), |n| trimmed(&n));
                out.push(Diagnostic {
                    problem: Problem::UnusedVariable,
                    range,
                    message: format!("the script variable `{name}` is never read"),
                    fixes: Vec::new(),
                });
            }
        }

        for import in root.children().filter(|n| n.kind() == SyntaxKind::IMPORT) {
            let Some(type_ref) = child(&import, SyntaxKind::TYPE_REF) else {
                continue;
            };
            let written = compact(&type_ref);
            let Some(script) = self.script_in(file, &written) else {
                continue;
            };
            if !self.import_used(file, root, script, &written) {
                out.push(Diagnostic {
                    problem: Problem::UnusedImport,
                    range: trimmed(&import),
                    message: format!(
                        "nothing of `{written}` is used unqualified: the `Import` can go"
                    ),
                    fixes: Vec::new(),
                });
            }
        }
    }

    /// `x = value`: `x` written, not read.
    fn written_only(name_ref: &SyntaxNode) -> bool {
        name_ref.parent().is_some_and(|p| {
            p.kind() == SyntaxKind::ASSIGN_STMT
                && p.children().next().as_ref() == Some(name_ref)
                && p.children_with_tokens().any(|e| e.kind() == SyntaxKind::EQ)
        })
    }

    /// Whether `file` uses what `Import` brings: a global function called
    /// unqualified, the script's short name, one of its structs.
    fn import_used(
        &self,
        file: SourceFile,
        root: &SyntaxNode,
        script: ScriptRef,
        written: &str,
    ) -> bool {
        // `Import MyNs:Helper` lets `Helper` name the script; `Game.Foo()`
        // needs no `Import Game`.
        let short = written.rsplit_once(':').map_or("", |(_, short)| short);
        let structs: Vec<&str> = match script {
            ScriptRef::Source(f) => self
                .items(f)
                .items
                .iter()
                .filter(|i| i.kind == ItemKind::Struct)
                .map(|i| i.name.as_str())
                .collect(),
            ScriptRef::Native(_) => Vec::new(),
        };
        let names_type = |text: &str| {
            let last = text
                .trim_end_matches("[]")
                .rsplit(':')
                .next()
                .unwrap_or_default();
            (!short.is_empty() && last.eq_ignore_ascii_case(short))
                || structs.iter().any(|s| s.eq_ignore_ascii_case(last))
        };
        for node in root.descendants() {
            match node.kind() {
                SyntaxKind::TYPE_REF
                    if node
                        .parent()
                        .is_some_and(|p| p.kind() != SyntaxKind::IMPORT) =>
                {
                    if names_type(&compact(&node)) {
                        return true;
                    }
                }
                SyntaxKind::NAME_REF => {
                    // A receiver: `Helper.Run()`.
                    if !short.is_empty() && compact(&node).eq_ignore_ascii_case(short) {
                        return true;
                    }
                    let unqualified = node
                        .parent()
                        .is_some_and(|p| p.kind() == SyntaxKind::CALL_EXPR);
                    if !unqualified {
                        continue;
                    }
                    let from = match self.resolve_name_ref(file, &node) {
                        Some(Def::Item { file: f, .. }) => Some(ScriptRef::Source(f)),
                        Some(Def::NativeFunction { script, .. }) => Some(ScriptRef::Native(script)),
                        _ => None,
                    };
                    if from == Some(script) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
}
