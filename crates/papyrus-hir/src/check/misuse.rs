//! What is declared but used the wrong way: a state that does not exist, an
//! event whose parameters are not the engine's, a read-only property
//! assigned, an instance reached from a `Global` function.

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use super::{Diagnostic, Problem, Replacement, child, closest, decl_name};
use crate::SourceFile;
use crate::calls::{arg_list, args};
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, member_name_ref};
use crate::ty::Ty;

/// The text of a string literal, quotes removed; `None` for anything else,
/// or for a string with escapes.
fn string_literal(node: &SyntaxNode) -> Option<(String, TextRange)> {
    if node.kind() != SyntaxKind::LITERAL {
        return None;
    }
    let token = node.first_token()?;
    if token.kind() != SyntaxKind::STRING {
        return None;
    }
    let text = token.text();
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    if inner.contains('\\') {
        return None;
    }
    let start = token.text_range().start() + TextSize::of('"');
    Some((inner.to_owned(), TextRange::at(start, TextSize::of(inner))))
}

/// The written type `ty` of `file`, as a comparable key: lowercase, scripts
/// by their full name.
fn type_key(sema: &Semantics<'_>, file: Option<SourceFile>, ty: &str) -> String {
    let ty = Ty::parse(ty);
    let ty = match file {
        Some(file) => sema.qualify(file, ty),
        None => ty,
    };
    ty.to_string().to_ascii_lowercase()
}

impl Semantics<'_> {
    // ── States ────────────────────────────────────────────────────────────

    /// The states of `script` and its ancestors; `None` when a link of the
    /// chain is unknown.
    fn states(&self, script: ScriptRef) -> Option<Vec<&str>> {
        if !self.chain_complete(script) {
            return None;
        }
        let mut out = Vec::new();
        for script in self.chain(script) {
            if let ScriptRef::Source(file) = script {
                out.extend(
                    self.items(file)
                        .items
                        .iter()
                        .filter(|i| i.kind == ItemKind::State)
                        .map(|i| i.name.as_str()),
                );
            }
        }
        Some(out)
    }

    /// True if a project script deriving from `script` declares the state
    /// `name`: the object may be one of them.
    fn derived_state(&self, script: SourceFile, name: &str) -> bool {
        self.index().files().any(|file| {
            file != script
                && self
                    .items(file)
                    .named(name)
                    .any(|(_, i)| i.kind == ItemKind::State)
                && self
                    .chain(ScriptRef::Source(file))
                    .any(|s| s == ScriptRef::Source(script))
        })
    }

    /// `GotoState("Busy")` to a state the script does not declare.
    pub(super) fn check_state(
        &self,
        file: SourceFile,
        call: &SyntaxNode,
        callee: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        if !compact(callee).eq_ignore_ascii_case("GotoState") {
            return;
        }
        // The engine's, not one the project declares.
        if !matches!(
            self.resolve_name_ref(file, callee),
            Some(Def::NativeFunction { .. })
        ) {
            return;
        }
        let Some(list) = arg_list(call) else {
            return;
        };
        let args = args(&list);
        let [arg] = args.as_slice() else {
            return;
        };
        let Some((name, range)) = arg.value.as_ref().and_then(string_literal) else {
            return;
        };
        // The empty state.
        if name.is_empty() {
            return;
        }
        let receiver = callee
            .parent()
            .filter(|p| p.kind() == SyntaxKind::FIELD_EXPR)
            .and_then(|p| p.children().next());
        let target = match &receiver {
            None => ScriptRef::Source(file),
            Some(receiver) => match self.receiver_script(file, receiver) {
                Some(script) => script,
                None => return,
            },
        };
        let ScriptRef::Source(target_file) = target else {
            return;
        };
        let Some(states) = self.states(target) else {
            return;
        };
        if states.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            return;
        }
        if self.derived_state(target_file, &name) {
            return;
        }
        let suggestion = closest(&name, states.iter().copied());
        let mut message = format!(
            "`{}` and its parents declare no state `{name}`",
            self.script_name(target)
        );
        if let Some(s) = suggestion {
            message.push_str(&format!(" — did you mean `{s}`?"));
        }
        out.push(Diagnostic {
            problem: Problem::UnknownState,
            range,
            message,
            replacement: suggestion.map(|s| Replacement {
                range,
                text: s.to_owned(),
            }),
        });
    }

    // ── Events ────────────────────────────────────────────────────────────

    /// An event whose parameters differ from the one it implements: the
    /// engine's, or the empty state's for an event in a state.
    pub(super) fn check_event_signature(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(name) = decl_name(decl) else {
            return;
        };
        let range = trimmed(&name);
        let items = self.items(file);
        let Some(item) = items
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Event && i.name_range == range)
        else {
            return;
        };
        // What it implements: the empty state's, for a state; else the
        // closest ancestor's, or for `Event Actor.OnDeath`, `Actor`'s.
        let mut origin: Option<(Def, String)> = None;
        if item.container.is_some() && item.remote.is_none() {
            origin = items
                .named(&item.name)
                .find(|(_, i)| {
                    i.kind == ItemKind::Event && i.container.is_none() && i.remote.is_none()
                })
                .map(|(index, _)| (Def::Item { file, index }, "the empty state".to_owned()));
        }
        if origin.is_none() {
            let chain: Vec<ScriptRef> = match &item.remote {
                Some(remote) => match self.script_in(file, remote) {
                    Some(script) => self.chain(script).collect(),
                    None => return,
                },
                None => self.chain(ScriptRef::Source(file)).skip(1).collect(),
            };
            origin = chain.into_iter().find_map(|script| {
                self.own_event(script, &item.name)
                    .map(|def| (def, format!("`{}`", self.script_name(script))))
            });
        }
        let Some((origin, from)) = origin else {
            return;
        };
        let Some(expected) = self.signature(&origin) else {
            return;
        };
        let origin_file = match origin {
            Def::Item { file, .. } => Some(file),
            _ => None,
        };
        // A remote event first receives the script that sent it.
        let params = match &item.remote {
            Some(_) => item.params.get(1..),
            None => Some(&item.params[..]),
        };
        let same = params.is_some_and(|params| {
            expected.params.len() == params.len()
                && expected.params.iter().zip(params).all(|(e, p)| {
                    type_key(self, origin_file, &e.written) == type_key(self, Some(file), &p.ty)
                })
        });
        if same {
            return;
        }
        let (label, _) = expected.label();
        let label = match &item.remote {
            Some(remote) => {
                let sender = if expected.params.is_empty() {
                    format!("{remote} akSender")
                } else {
                    format!("{remote} akSender, ")
                };
                format!("{remote}.{}", label.replacen('(', &format!("({sender}"), 1))
            }
            None => label,
        };
        out.push(Diagnostic {
            problem: Problem::EventSignature,
            range,
            message: format!(
                "`{}` does not take the parameters of the event of {from}: `Event {label}`",
                item.name
            ),
            replacement: None,
        });
    }

    /// The event `name` declared by `script` itself.
    fn own_event(&self, script: ScriptRef, name: &str) -> Option<Def> {
        match script {
            ScriptRef::Source(file) => self
                .items(file)
                .named(name)
                .find(|(_, i)| {
                    i.kind == ItemKind::Event && i.container.is_none() && i.remote.is_none()
                })
                .map(|(index, _)| Def::Item { file, index }),
            ScriptRef::Native(script) => script
                .events
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                .map(|function| Def::NativeFunction {
                    script,
                    function,
                    event: true,
                }),
        }
    }

    // ── Read-only properties ──────────────────────────────────────────────

    /// `x = value` or `x += value` on a property that cannot be assigned.
    pub(super) fn check_readonly(
        &self,
        file: SourceFile,
        stmt: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(target) = stmt.children().next() else {
            return;
        };
        let name_ref = match target.kind() {
            SyntaxKind::NAME_REF => target.clone(),
            SyntaxKind::FIELD_EXPR => match member_name_ref(&target) {
                Some(name_ref) => name_ref,
                None => return,
            },
            _ => return,
        };
        let why = match self.resolve_name_ref(file, &name_ref) {
            Some(Def::Item { file, index }) => {
                let item = self.item(file, index);
                if item.kind != ItemKind::Property || !item.readonly {
                    return;
                }
                let flags = item.signature.to_ascii_lowercase();
                let words: Vec<&str> = flags.split_whitespace().collect();
                if words.contains(&"autoreadonly") {
                    "is `AutoReadOnly`"
                } else if words.contains(&"const") {
                    "is `Const`"
                } else {
                    "has no `Set` function"
                }
            }
            Some(Def::NativeProperty { property, .. }) if property.readonly => "is read-only",
            _ => return,
        };
        out.push(Diagnostic {
            problem: Problem::ReadOnly,
            range: trimmed(&name_ref),
            message: format!(
                "the property `{}` {why}: it cannot be assigned",
                compact(&name_ref)
            ),
            replacement: None,
        });
    }

    // ── Global functions ──────────────────────────────────────────────────

    /// True for a function or an event called on an instance.
    fn is_instance_callable(&self, def: &Def) -> bool {
        match def {
            Def::Item { file, index } => {
                let item = self.item(*file, *index);
                item.kind.is_callable() && !item.global
            }
            Def::NativeFunction { function, .. } => !function.global,
            _ => false,
        }
    }

    /// `Script.Foo()`, where `Foo` is not `Global`.
    pub(super) fn check_static_call(
        &self,
        file: SourceFile,
        callee: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(receiver) = callee
            .parent()
            .filter(|p| p.kind() == SyntaxKind::FIELD_EXPR)
            .and_then(|p| p.children().next())
            .filter(|r| r.kind() == SyntaxKind::NAME_REF)
        else {
            return;
        };
        if receiver
            .first_token()
            .is_some_and(|t| matches!(t.kind(), SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW))
        {
            return;
        }
        let script = match self.resolve_name_ref(file, &receiver) {
            Some(Def::Script { file }) => ScriptRef::Source(file),
            Some(Def::NativeScript(script)) => ScriptRef::Native(script),
            _ => return,
        };
        let Some(def) = self.resolve_name_ref(file, callee) else {
            return;
        };
        if !self.is_instance_callable(&def) {
            return;
        }
        out.push(Diagnostic {
            problem: Problem::NotGlobal,
            range: trimmed(callee),
            message: format!(
                "`{}` is not `Global`: it is called on an instance of `{}`, not on the script",
                self.def_name(&def),
                self.script_name(script)
            ),
            replacement: None,
        });
    }

    /// `self`, a member function, variable or property used in a `Global`
    /// function, which runs without an instance.
    pub(super) fn check_global_body(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        body: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let global = child(decl, SyntaxKind::FLAG_LIST).is_some_and(|list| {
            list.children()
                .filter_map(|f| f.first_token())
                .any(|t| t.kind() == SyntaxKind::GLOBAL_KW)
        });
        if decl.kind() != SyntaxKind::FUNCTION_DECL || !global {
            return;
        }
        let function = decl_name(decl).map(|n| compact(&n)).unwrap_or_default();
        let this = ScriptRef::Source(file);
        for name_ref in body
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::NAME_REF)
        {
            let Some(token) = name_ref.first_token() else {
                continue;
            };
            if matches!(token.kind(), SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW) {
                out.push(Diagnostic {
                    problem: Problem::NotGlobal,
                    range: trimmed(&name_ref),
                    message: format!(
                        "`{}` does not exist in `{function}`, which is `Global`",
                        token.text()
                    ),
                    replacement: None,
                });
                continue;
            }
            // `a.b`: only `a` is looked up in the function.
            let qualified = name_ref.parent().is_some_and(|p| {
                p.kind() == SyntaxKind::FIELD_EXPR
                    && p.children().next().as_ref() != Some(&name_ref)
            });
            if qualified {
                continue;
            }
            let Some(def) = self.resolve_name_ref(file, &name_ref) else {
                continue;
            };
            let what = match &def {
                Def::Item { file: owner, index } => {
                    let item = self.item(*owner, *index);
                    let member = self.chain(this).any(|s| s == ScriptRef::Source(*owner));
                    match item.kind {
                        _ if !member => continue,
                        ItemKind::Function | ItemKind::Event if !item.global => {
                            item.kind.describe()
                        }
                        ItemKind::Variable | ItemKind::Property => item.kind.describe(),
                        _ => continue,
                    }
                }
                Def::NativeFunction {
                    function, event, ..
                } if !function.global => {
                    if *event {
                        "event"
                    } else {
                        "function"
                    }
                }
                Def::NativeProperty { .. } => "property",
                _ => continue,
            };
            // An imported script's global function of the same name: the
            // compiler may pick it.
            let name = compact(&name_ref);
            let imported = self.items(file).imports.iter().any(|import| {
                self.script_in(file, import)
                    .is_some_and(|script| match script {
                        ScriptRef::Source(f) => self
                            .items(f)
                            .named(&name)
                            .any(|(_, i)| i.kind == ItemKind::Function && i.global),
                        ScriptRef::Native(s) => s
                            .functions
                            .iter()
                            .any(|f| f.global && f.name.eq_ignore_ascii_case(&name)),
                    })
            });
            if imported {
                continue;
            }
            out.push(Diagnostic {
                problem: Problem::NotGlobal,
                range: trimmed(&name_ref),
                message: format!(
                    "`{function}` is `Global`: it has no instance, and cannot use the {what} `{}`",
                    self.def_name(&def)
                ),
                replacement: None,
            });
        }
    }
}
