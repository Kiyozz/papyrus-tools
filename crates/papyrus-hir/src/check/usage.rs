//! Locals: declared twice, never read; statements after `Return`;
//! parameters never read.

use std::collections::HashMap;

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange};

use super::{Diagnostic, Fix, Problem, Replacement, child, decl_name, stmt_returns};
use crate::SourceFile;
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics};

/// True for the target of a plain assignment, `x = value`: written, not
/// read.
fn is_written(name_ref: &SyntaxNode) -> bool {
    name_ref.parent().is_some_and(|stmt| {
        stmt.kind() == SyntaxKind::ASSIGN_STMT
            && stmt.children().next().as_ref() == Some(name_ref)
            && stmt
                .children_with_tokens()
                .any(|e| e.kind() == SyntaxKind::EQ)
    })
}

/// The statements of a list that follow one that always returns.
pub(super) fn check_unreachable(body: &SyntaxNode, out: &mut Vec<Diagnostic>) {
    let mut reported: Vec<TextRange> = Vec::new();
    for list in body
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::STMT_LIST)
    {
        let range = list.text_range();
        if reported.iter().any(|r| r.contains_range(range)) {
            continue;
        }
        let stmts: Vec<SyntaxNode> = list.children().collect();
        let Some(end) = stmts.iter().position(stmt_returns) else {
            continue;
        };
        let (Some(first), Some(last)) = (stmts.get(end + 1), stmts.last()) else {
            continue;
        };
        let range = trimmed(first).cover(trimmed(last));
        reported.push(range);
        let why = if stmts[end].kind() == SyntaxKind::RETURN_STMT {
            "`Return` comes before it"
        } else {
            "the statement before it always returns"
        };
        out.push(Diagnostic {
            problem: Problem::Unreachable,
            range,
            message: format!("this code never runs: {why}"),
            fixes: Vec::new(),
        });
    }
}

/// Locals the compiler refuses: declared twice in a block, named like a
/// parameter or a variable of the script, or shadowing one of an enclosing
/// block. Sibling blocks may reuse a name.
pub(super) fn check_duplicates(
    root: &SyntaxNode,
    decl: &SyntaxNode,
    body: &SyntaxNode,
    out: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let lower = |n: &SyntaxNode| compact(n).to_ascii_lowercase();
    let script: Vec<String> = root
        .children()
        .filter(|n| n.kind() == SyntaxKind::VARIABLE_DECL)
        .filter_map(|n| child(&n, SyntaxKind::NAME))
        .map(|n| lower(&n))
        .collect();
    let params: Vec<String> = child(decl, SyntaxKind::PARAM_LIST)
        .into_iter()
        .flat_map(|list| list.children().filter(|p| p.kind() == SyntaxKind::PARAM))
        .filter_map(|p| child(&p, SyntaxKind::NAME))
        .map(|n| lower(&n))
        .collect();
    let mut scopes = Scopes {
        script,
        params,
        blocks: Vec::new(),
        duplicated: Vec::new(),
        out,
    };
    scopes.block(body);
    scopes.duplicated
}

/// The names declared so far, block by block.
struct Scopes<'a> {
    script: Vec<String>,
    /// The parameters, in the body's own block.
    params: Vec<String>,
    blocks: Vec<Vec<String>>,
    /// The names reported, lowercase.
    duplicated: Vec<String>,
    out: &'a mut Vec<Diagnostic>,
}

impl Scopes<'_> {
    fn block(&mut self, list: &SyntaxNode) {
        self.blocks.push(Vec::new());
        for stmt in list.children() {
            if stmt.kind() == SyntaxKind::VARIABLE_DECL {
                self.declare(&stmt);
            } else {
                self.nested(&stmt);
            }
        }
        self.blocks.pop();
    }

    /// The blocks of a statement: `If`, `ElseIf`, `Else`, `While`…
    fn nested(&mut self, node: &SyntaxNode) {
        for inner in node.children() {
            if inner.kind() == SyntaxKind::STMT_LIST {
                self.block(&inner);
            } else {
                self.nested(&inner);
            }
        }
    }

    fn declare(&mut self, stmt: &SyntaxNode) {
        let Some(name) = child(stmt, SyntaxKind::NAME) else {
            return;
        };
        let key = compact(&name).to_ascii_lowercase();
        let outermost = self.blocks.len() == 1;
        let current = self.blocks.last().is_some_and(|b| b.contains(&key));
        let param = self.params.contains(&key);
        let why = if current || (outermost && param) {
            Some(if param {
                "is already a parameter of this function"
            } else {
                "is already declared in this block"
            })
        } else if param || self.blocks.iter().any(|b| b.contains(&key)) {
            Some("is already declared in an enclosing block, which Papyrus forbids hiding")
        } else if self.script.contains(&key) {
            Some("is already a variable of the script")
        } else {
            None
        };
        if let Some(why) = why {
            // Redeclared with a value in the same block: assigning was
            // likely meant.
            let fixes = match (child(stmt, SyntaxKind::TYPE_REF), current) {
                (Some(ty), true)
                    if stmt
                        .children_with_tokens()
                        .any(|e| e.kind() == SyntaxKind::EQ) =>
                {
                    vec![Fix {
                        title: format!("Assign `{}` instead", compact(&name)),
                        edits: vec![Replacement {
                            range: TextRange::new(trimmed(&ty).start(), trimmed(&name).start()),
                            text: String::new(),
                        }],
                    }]
                }
                _ => Vec::new(),
            };
            self.duplicated.push(key.clone());
            self.out.push(Diagnostic {
                problem: Problem::DuplicateLocal,
                range: trimmed(&name),
                message: format!("the variable `{}` {why}", compact(&name)),
                fixes,
            });
        }
        if let Some(block) = self.blocks.last_mut() {
            block.push(key);
        }
    }
}

/// Whether a local or a parameter is read, written, or neither.
#[derive(Default, Clone, Copy)]
struct Use {
    read: bool,
    written: bool,
}

impl Semantics<'_> {
    /// Locals never read; parameters never read, after the last one read.
    pub(super) fn check_unused(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        body: &SyntaxNode,
        duplicated: &[String],
        out: &mut Vec<Diagnostic>,
    ) {
        // A name `locals/duplicate` reports: one problem is enough.
        let reported = |n: &SyntaxNode| duplicated.contains(&compact(n).to_ascii_lowercase());
        let Some(name) = decl_name(decl) else {
            return;
        };
        let function = compact(&name);
        // The Creation Kit writes the fragments, with the parameters and
        // locals it wants.
        if function
            .get(..9)
            .is_some_and(|p| p.eq_ignore_ascii_case("Fragment_"))
        {
            return;
        }
        let locals: Vec<SyntaxNode> = body
            .descendants()
            .filter(|n| {
                n.kind() == SyntaxKind::VARIABLE_DECL
                    && n.parent()
                        .is_some_and(|p| p.kind() == SyntaxKind::STMT_LIST)
            })
            .filter_map(|n| child(&n, SyntaxKind::NAME))
            .collect();
        let params: Vec<SyntaxNode> = child(decl, SyntaxKind::PARAM_LIST)
            .into_iter()
            .flat_map(|list| list.children().filter(|p| p.kind() == SyntaxKind::PARAM))
            .filter_map(|p| child(&p, SyntaxKind::NAME))
            .collect();
        if locals.is_empty() && params.is_empty() {
            return;
        }
        let names: Vec<String> = locals
            .iter()
            .chain(&params)
            .map(|n| compact(n).to_ascii_lowercase())
            .collect();

        let mut uses: HashMap<TextRange, Use> = HashMap::new();
        for name_ref in body
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::NAME_REF)
        {
            if !names.contains(&compact(&name_ref).to_ascii_lowercase()) {
                continue;
            }
            if let Some(Def::Local { name_range, .. }) = self.resolve_name_ref(file, &name_ref) {
                let entry = uses.entry(name_range).or_default();
                if is_written(&name_ref) {
                    entry.written = true;
                } else {
                    entry.read = true;
                }
            }
        }
        let use_of = |name: &SyntaxNode| uses.get(&trimmed(name)).copied().unwrap_or_default();

        for local in &locals {
            let found = use_of(local);
            if found.read || reported(local) {
                continue;
            }
            let what = if found.written {
                "is assigned but never read"
            } else {
                "is never used"
            };
            out.push(Diagnostic {
                problem: Problem::UnusedLocal,
                range: trimmed(local),
                message: format!("the variable `{}` {what}", compact(local)),
                fixes: Vec::new(),
            });
        }

        // A parameter before one that is read stays: the caller passes it
        // to reach the next.
        let first_unused = params
            .iter()
            .rposition(|p| use_of(p).read)
            .map_or(0, |i| i + 1);
        // An empty body is a stub, whose parameters await an implementation.
        if first_unused == params.len()
            || body.children().next().is_none()
            || !self.own_signature(file, decl, &name)
        {
            return;
        }
        for param in params[first_unused..].iter().filter(|p| !reported(p)) {
            out.push(Diagnostic {
                problem: Problem::UnusedParam,
                range: trimmed(param),
                message: format!("the parameter `{}` is never used", compact(param)),
                fixes: Vec::new(),
            });
        }
    }

    /// True if the parameters of a function are its own choice: not an
    /// event, not native, neither overriding nor overridden.
    fn own_signature(&self, file: SourceFile, decl: &SyntaxNode, name: &SyntaxNode) -> bool {
        if decl.kind() != SyntaxKind::FUNCTION_DECL {
            return false;
        }
        let range = trimmed(name);
        let items = self.items(file);
        let Some(index) = items
            .items
            .iter()
            .position(|i| i.kind == ItemKind::Function && i.name_range == range)
        else {
            return false;
        };
        let item = &items.items[index];
        if item.native {
            return false;
        }
        let def = Def::Item { file, index };
        if self.canonical(&def) != def {
            return false;
        }
        // Redefined by a state of the script.
        if items
            .named(&item.name)
            .any(|(i, other)| i != index && other.kind == ItemKind::Function)
        {
            return false;
        }
        // Redefined by a script deriving from this one.
        let this = ScriptRef::Source(file);
        !self.index().files().any(|other| {
            other != file
                && self
                    .items(other)
                    .named(&item.name)
                    .any(|(_, i)| i.kind == ItemKind::Function)
                && self.chain(ScriptRef::Source(other)).any(|s| s == this)
        })
    }
}
