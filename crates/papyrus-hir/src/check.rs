//! Semantic diagnostics: calls, types, members, returns.
//!
//! Each one is certain. A call is only judged when its callee is resolved,
//! a type when its whole `extends` chain is known, a missing member when
//! every script it could come from is known. Whatever is unknown stays
//! quiet: missing information is never an error.

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange};

use crate::SourceFile;
use crate::calls::{Arg, arg_list, args, callee};
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, is_callee, locals_in_scope};
use crate::ty::Ty;
use crate::types::Object;

/// What is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Problem {
    /// Too few or too many arguments.
    Arity,
    /// A named argument the callee does not declare, or one already given.
    NamedArg,
    /// A value of a type the compiler does not cast implicitly.
    Mismatch,
    /// A function, a variable, a property or a field that does not exist.
    UnknownMember,
    /// A typed function that can reach its end without `Return`.
    MissingReturn,
}

/// A replacement to suggest; never applied automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// What is replaced.
    pub range: TextRange,
    /// The new text.
    pub text: String,
}

/// A problem found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What is wrong.
    pub problem: Problem,
    /// Where.
    pub range: TextRange,
    /// What, and how to fix it.
    pub message: String,
    /// The likely intended name, for a typo.
    pub replacement: Option<Replacement>,
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{count} {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// The candidate closest to `name`, if close enough to be a typo.
fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let budget = match name.len() {
        0..=4 => 1,
        5..=8 => 2,
        _ => 3,
    };
    candidates
        .into_iter()
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| (1..=budget).contains(d))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// Edit distance, case-insensitive.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<u8> = a.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let b: Vec<u8> = b.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

/// True if a statement list cannot run to its end.
fn always_returns(list: &SyntaxNode) -> bool {
    list.children().any(|stmt| stmt_returns(&stmt))
}

fn stmt_returns(stmt: &SyntaxNode) -> bool {
    let body =
        |node: &SyntaxNode| child(node, SyntaxKind::STMT_LIST).is_some_and(|l| always_returns(&l));
    match stmt.kind() {
        SyntaxKind::RETURN_STMT => true,
        SyntaxKind::IF_STMT => {
            body(stmt)
                && stmt
                    .children()
                    .filter(|c| c.kind() == SyntaxKind::ELSEIF_CLAUSE)
                    .all(|c| body(&c))
                && child(stmt, SyntaxKind::ELSE_CLAUSE).is_some_and(|c| body(&c))
        }
        // `While True` only ends through `Return`: Papyrus has no `break`.
        SyntaxKind::WHILE_STMT => stmt.children().next().is_some_and(|condition| {
            let mut condition = condition;
            while condition.kind() == SyntaxKind::PAREN_EXPR {
                match condition.children().next() {
                    Some(inner) => condition = inner,
                    None => return false,
                }
            }
            condition.kind() == SyntaxKind::LITERAL
                && condition
                    .first_token()
                    .is_some_and(|t| t.kind() == SyntaxKind::TRUE_KW)
        }),
        SyntaxKind::LOCK_GUARD_STMT => body(stmt),
        SyntaxKind::TRY_LOCK_GUARD_STMT => {
            body(stmt)
                && child(stmt, SyntaxKind::ELSE_TRY_LOCK_GUARD_CLAUSE).is_some_and(|c| body(&c))
        }
        _ => false,
    }
}

/// The name of a function or an event declaration.
fn decl_name(decl: &SyntaxNode) -> Option<SyntaxNode> {
    child(decl, SyntaxKind::NAME)
}

/// The explicit cast that would turn `from` into `to`, when there is one.
fn cast_hint(from: &Ty, to: &Ty) -> String {
    let castable = matches!(
        (from, to),
        (Ty::Float | Ty::String | Ty::Bool, Ty::Int)
            | (Ty::String | Ty::Bool, Ty::Float)
            | (Ty::Object(_), Ty::Object(_))
    );
    if castable {
        format!(" — cast it with `as {to}`")
    } else {
        String::new()
    }
}

impl Semantics<'_> {
    /// The semantic problems of `file`. Nothing without a target: the
    /// native scripts depend on it.
    pub fn check(&self, file: SourceFile) -> Vec<Diagnostic> {
        let Some(game) = self.game() else {
            return Vec::new();
        };
        let root = self.root(file);
        let errors: Vec<TextRange> = crate::parse(self.db(), file, Some(game))
            .errors()
            .iter()
            .map(|e| e.range())
            .collect();
        // A declaration the parser could not read whole is left alone.
        let broken = |node: &SyntaxNode| {
            let range = node.text_range();
            errors
                .iter()
                .any(|e| range.start() <= e.start() && e.start() < range.end())
                || node
                    .descendants()
                    .any(|n| n.kind() == SyntaxKind::ERROR_TREE)
        };

        let mut out = Vec::new();
        for node in root.descendants() {
            match node.kind() {
                SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
                    let Some(body) = child(&node, SyntaxKind::STMT_LIST) else {
                        continue;
                    };
                    if broken(&node) {
                        continue;
                    }
                    self.check_returns(&node, &body, &mut out);
                    for inner in body.descendants() {
                        match inner.kind() {
                            SyntaxKind::CALL_EXPR => self.check_call(file, &inner, &mut out),
                            SyntaxKind::ASSIGN_STMT => self.check_assign(file, &inner, &mut out),
                            SyntaxKind::VARIABLE_DECL => self.check_init(file, &inner, &mut out),
                            SyntaxKind::RETURN_STMT => {
                                self.check_return_value(file, &node, &inner, &mut out);
                            }
                            SyntaxKind::NAME_REF => {
                                self.check_value(file, &node, &inner, &mut out);
                            }
                            _ => {}
                        }
                    }
                }
                // Script variables: their initial value.
                SyntaxKind::VARIABLE_DECL
                    if node
                        .parent()
                        .is_some_and(|p| p.kind() == SyntaxKind::SOURCE_FILE)
                        && !broken(&node) =>
                {
                    self.check_init(file, &node, &mut out);
                }
                _ => {}
            }
        }
        out.sort_by_key(|d| d.range.start());
        out
    }

    // ── Returns ───────────────────────────────────────────────────────────

    /// A typed function that can reach `EndFunction`. An empty body is a
    /// deliberate stub, which returns the default value.
    fn check_returns(&self, decl: &SyntaxNode, body: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if decl.kind() != SyntaxKind::FUNCTION_DECL || body.children().next().is_none() {
            return;
        }
        let (Some(returns), Some(name)) = (child(decl, SyntaxKind::TYPE_REF), decl_name(decl))
        else {
            return;
        };
        if always_returns(body) {
            return;
        }
        out.push(Diagnostic {
            problem: Problem::MissingReturn,
            range: trimmed(&name),
            message: format!(
                "`{}` returns `{}`, but can reach `EndFunction` without `Return`: it then \
                 returns the default value",
                compact(&name),
                compact(&returns)
            ),
            replacement: None,
        });
    }

    /// `Return value`: of the declared type, and only in a typed function.
    fn check_return_value(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        ret: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(value) = ret.children().next() else {
            return;
        };
        // Without a return type, the compiler accepts `Return Foo()` when
        // `Foo` returns nothing either: left alone.
        let Some(returns) =
            child(decl, SyntaxKind::TYPE_REF).filter(|_| decl.kind() == SyntaxKind::FUNCTION_DECL)
        else {
            return;
        };
        let name = decl_name(decl).map(|n| compact(&n)).unwrap_or_default();
        let expected = self.qualify(file, Ty::parse(&compact(&returns)));
        self.mismatch(file, &value, &expected, out, |found, to| {
            format!("`{name}` returns `{to}`, found `{found}`")
        });
    }

    // ── Types ─────────────────────────────────────────────────────────────

    /// Reports `value` if it cannot go where `expected` is expected.
    fn mismatch(
        &self,
        file: SourceFile,
        value: &SyntaxNode,
        expected: &Ty,
        out: &mut Vec<Diagnostic>,
        message: impl FnOnce(&Ty, &Ty) -> String,
    ) {
        let found = self.type_of(file, value);
        if self.coerces(&found, expected) == Some(false) {
            out.push(Diagnostic {
                problem: Problem::Mismatch,
                range: trimmed(value),
                message: format!(
                    "{}{}",
                    message(&found, expected),
                    cast_hint(&found, expected)
                ),
                replacement: None,
            });
        }
    }

    /// `x = value`.
    fn check_assign(&self, file: SourceFile, stmt: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let plain = stmt
            .children_with_tokens()
            .any(|e| e.kind() == SyntaxKind::EQ);
        let mut nodes = stmt.children();
        let (Some(target), Some(value)) = (nodes.next(), nodes.next()) else {
            return;
        };
        if !plain {
            return;
        }
        let expected = self.type_of(file, &target);
        let name = compact(&target);
        self.mismatch(file, &value, &expected, out, |found, to| {
            format!("`{name}` is `{to}`: a `{found}` cannot be assigned to it")
        });
    }

    /// `int x = value`.
    fn check_init(&self, file: SourceFile, decl: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let (Some(ty), Some(name)) = (
            child(decl, SyntaxKind::TYPE_REF),
            child(decl, SyntaxKind::NAME),
        ) else {
            return;
        };
        let Some(value) = decl
            .children_with_tokens()
            .skip_while(|e| e.kind() != SyntaxKind::EQ)
            .find_map(|e| e.into_node())
        else {
            return;
        };
        let expected = self.qualify(file, Ty::parse(&compact(&ty)));
        let name = compact(&name);
        self.mismatch(file, &value, &expected, out, |found, to| {
            format!("`{name}` is `{to}`: it cannot be initialized with a `{found}`")
        });
    }

    // ── Calls ─────────────────────────────────────────────────────────────

    fn check_call(&self, file: SourceFile, call: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let (Some(callee), Some(list)) = (callee(call), arg_list(call)) else {
            return;
        };
        let closed = list
            .children_with_tokens()
            .any(|e| e.kind() == SyntaxKind::R_PAREN);
        if !closed {
            return;
        }
        let Some(def) = self.resolve_name_ref(file, &callee) else {
            self.unknown_function(file, &callee, out);
            return;
        };
        let Some(signature) = self.signature(&def) else {
            return;
        };
        let args = args(&list);
        if args.iter().any(|a| a.value.is_none()) {
            return;
        }
        let positional: Vec<&Arg> = args.iter().take_while(|a| a.name.is_none()).collect();
        let named: Vec<&Arg> = args[positional.len()..].iter().collect();
        // A positional argument after a named one: the compiler says it.
        if named.iter().any(|a| a.name.is_none()) {
            return;
        }
        let name = &signature.name;
        let params = &signature.params;

        if positional.len() > params.len() {
            let extra = &positional[params.len()..];
            let range = trimmed(&extra[0].node).cover(trimmed(&extra[extra.len() - 1].node));
            out.push(Diagnostic {
                problem: Problem::Arity,
                range,
                message: format!(
                    "`{name}` takes {}, {} given",
                    plural(params.len(), "argument"),
                    positional.len()
                ),
                replacement: None,
            });
            return;
        }

        let mut given: Vec<bool> = (0..params.len()).map(|i| i < positional.len()).collect();
        let mut unknown = false;
        for arg in &named {
            let Some(arg_name) = &arg.name else {
                continue;
            };
            let written = compact(arg_name);
            match signature.param(&written) {
                Some(index) if given[index] => out.push(Diagnostic {
                    problem: Problem::NamedArg,
                    range: trimmed(arg_name),
                    message: format!("`{}` of `{name}` is already given", params[index].name),
                    replacement: None,
                }),
                Some(index) => given[index] = true,
                None => {
                    unknown = true;
                    let suggestion = closest(&written, params.iter().map(|p| p.name.as_str()));
                    let mut message = format!("`{name}` has no parameter `{written}`");
                    if let Some(s) = suggestion {
                        message.push_str(&format!(" — did you mean `{s}`?"));
                    }
                    out.push(Diagnostic {
                        problem: Problem::NamedArg,
                        range: trimmed(arg_name),
                        message,
                        replacement: suggestion.map(|s| Replacement {
                            range: trimmed(arg_name),
                            text: s.to_owned(),
                        }),
                    });
                }
            }
        }

        let missing: Vec<String> = params
            .iter()
            .zip(&given)
            .filter(|(p, given)| !**given && p.default.is_none())
            .map(|(p, _)| format!("`{}`", p.name))
            .collect();
        // A misspelled name may be the missing one: one error is enough.
        if !missing.is_empty() && !unknown {
            out.push(Diagnostic {
                problem: Problem::Arity,
                range: trimmed(&list),
                message: format!(
                    "`{name}` is missing {} {}",
                    if missing.len() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                    missing.join(", ")
                ),
                replacement: None,
            });
        }

        let pairs =
            positional
                .iter()
                .enumerate()
                .map(|(i, a)| (i, *a))
                .chain(named.iter().filter_map(|a| {
                    let index = signature.param(&compact(a.name.as_ref()?))?;
                    Some((index, *a))
                }));
        for (index, arg) in pairs {
            let (Some(value), Some(param)) = (&arg.value, params.get(index)) else {
                continue;
            };
            self.mismatch(file, value, &param.ty, out, |found, to| {
                format!(
                    "`{name}` expects `{to}` for `{}`, found `{found}`",
                    param.name
                )
            });
        }
    }

    // ── Members ───────────────────────────────────────────────────────────

    /// The functions and events of `script` and its ancestors.
    fn callables(&self, script: ScriptRef) -> Vec<&str> {
        let mut out = Vec::new();
        for script in self.chain(script) {
            match script {
                ScriptRef::Source(file) => out.extend(
                    self.items(file)
                        .items
                        .iter()
                        .filter(|i| i.kind.is_callable() && i.remote.is_none())
                        .map(|i| i.name.as_str()),
                ),
                ScriptRef::Native(script) => {
                    out.extend(script.functions.iter().chain(script.events).map(|f| f.name))
                }
            }
        }
        if self.skyrim() {
            let builtins = &crate::builtins::SKYRIM_SCRIPT;
            out.extend(
                builtins
                    .functions
                    .iter()
                    .chain(builtins.events)
                    .map(|f| f.name),
            );
        }
        out
    }

    /// The variables and properties of `script` and its ancestors.
    fn values(&self, script: ScriptRef) -> Vec<&str> {
        let mut out = Vec::new();
        for script in self.chain(script) {
            match script {
                ScriptRef::Source(file) => out.extend(
                    self.items(file)
                        .items
                        .iter()
                        .filter(|i| {
                            matches!(
                                i.kind,
                                ItemKind::Variable | ItemKind::Property | ItemKind::Guard
                            )
                        })
                        .map(|i| i.name.as_str()),
                ),
                ScriptRef::Native(script) => {
                    out.extend(script.properties.iter().map(|p| p.name));
                }
            }
        }
        out
    }

    fn unknown(
        &self,
        node: &SyntaxNode,
        message: String,
        candidates: Vec<&str>,
        out: &mut Vec<Diagnostic>,
    ) {
        let name = compact(node);
        let suggestion = closest(&name, candidates);
        let mut message = message;
        if let Some(s) = suggestion {
            message.push_str(&format!(" — did you mean `{s}`?"));
        }
        out.push(Diagnostic {
            problem: Problem::UnknownMember,
            range: trimmed(node),
            message,
            replacement: suggestion.map(|s| Replacement {
                range: trimmed(node),
                text: s.to_owned(),
            }),
        });
    }

    /// The script a receiver designates: itself for `Debug` or `self`,
    /// else the script of its value.
    fn receiver_script(&self, file: SourceFile, receiver: &SyntaxNode) -> Option<ScriptRef> {
        if receiver.kind() == SyntaxKind::NAME_REF {
            match self.resolve_name_ref(file, receiver) {
                Some(Def::Script { file }) => return Some(ScriptRef::Source(file)),
                Some(Def::NativeScript(script)) => return Some(ScriptRef::Native(script)),
                _ => {}
            }
        }
        match self.type_of(file, receiver).object() {
            Some(name) => match self.object(name)? {
                Object::Script(script) => Some(script),
                Object::Struct(..) => None,
            },
            None => None,
        }
    }

    /// A call that resolves to nothing, though everything it could reach is
    /// known.
    fn unknown_function(&self, file: SourceFile, callee: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let name = compact(callee);
        let receiver = callee
            .parent()
            .filter(|p| p.kind() == SyntaxKind::FIELD_EXPR)
            .and_then(|p| p.children().next());
        if let Some(receiver) = receiver {
            if let Ty::Array(_) = self.type_of(file, &receiver) {
                let functions: Vec<&str> = self.array_functions().iter().map(|f| f.name).collect();
                let message = if self.skyrim() {
                    format!(
                        "arrays have no function `{name}`: Skyrim only knows `Find` and `RFind`"
                    )
                } else {
                    format!("arrays have no function `{name}`")
                };
                self.unknown(callee, message, functions, out);
                return;
            }
            let Some(script) = self.receiver_script(file, &receiver) else {
                return;
            };
            if !self.chain_complete(script) {
                return;
            }
            let message = format!(
                "`{}` and its parents have no function `{name}`",
                self.script_name(script)
            );
            self.unknown(callee, message, self.callables(script), out);
            return;
        }

        if name.contains(':') {
            return;
        }
        let this = ScriptRef::Source(file);
        if !self.chain_complete(this) {
            return;
        }
        // The global functions of the imported scripts.
        let mut candidates = self.callables(this);
        for import in &self.items(file).imports {
            let Some(script) = self.script_in(file, import) else {
                return;
            };
            match script {
                ScriptRef::Source(f) => candidates.extend(
                    self.items(f)
                        .items
                        .iter()
                        .filter(|i| i.kind == ItemKind::Function && i.global)
                        .map(|i| i.name.as_str()),
                ),
                ScriptRef::Native(s) => {
                    candidates.extend(s.functions.iter().filter(|f| f.global).map(|f| f.name));
                }
            }
        }
        let message = format!(
            "`{}`, its parents and its imports have no function `{name}`",
            self.script_name(this)
        );
        self.unknown(callee, message, candidates, out);
    }

    /// A name read or written that resolves to nothing, though everything
    /// it could designate is known.
    fn check_value(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        name_ref: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let Some(token) = name_ref.first_token() else {
            return;
        };
        if matches!(
            token.kind(),
            SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW | SyntaxKind::LENGTH_KW
        ) {
            return;
        }
        let Some(parent) = name_ref.parent() else {
            return;
        };
        // Named arguments and calls have their own checks.
        if parent.kind() == SyntaxKind::ARG
            && name_ref
                .siblings_with_tokens(rowan::Direction::Next)
                .any(|e| e.kind() == SyntaxKind::EQ)
        {
            return;
        }
        if is_callee(name_ref) {
            return;
        }
        let name = compact(name_ref);

        if parent.kind() == SyntaxKind::FIELD_EXPR {
            let receiver = parent.children().next();
            // `a.b`: `a` may be a script that is not known.
            if receiver.as_ref() == Some(name_ref) || is_callee(&parent) {
                return;
            }
            if self.resolve_name_ref(file, name_ref).is_some() {
                return;
            }
            let Some(receiver) = receiver else {
                return;
            };
            match self.type_of(file, &receiver) {
                Ty::Array(_) => self.unknown(
                    name_ref,
                    format!("arrays have no member `{name}`, only `Length`"),
                    vec!["Length"],
                    out,
                ),
                Ty::Object(object) => match self.object(&object) {
                    Some(Object::Script(script)) if self.chain_complete(script) => {
                        let message = format!(
                            "`{}` and its parents have no property `{name}`",
                            self.script_name(script)
                        );
                        self.unknown(name_ref, message, self.values(script), out);
                    }
                    Some(Object::Struct(file, index)) => {
                        let items = self.items(file);
                        let structure = &items.items[index].name;
                        let fields = items
                            .items
                            .iter()
                            .filter(|i| {
                                i.kind == ItemKind::Field && i.container.as_ref() == Some(structure)
                            })
                            .map(|i| i.name.as_str())
                            .collect();
                        let message = format!("the struct `{structure}` has no field `{name}`");
                        self.unknown(name_ref, message, fields, out);
                    }
                    _ => {}
                },
                _ => {}
            }
            return;
        }

        // `__state`: the compiler's own variable, which `ScriptObject` reads.
        if name.contains(':')
            || name.eq_ignore_ascii_case("__state")
            || self.resolve_name_ref(file, name_ref).is_some()
        {
            return;
        }
        // A variable of another block of the function, which the compiler
        // may still see.
        let declared_here = decl.descendants().any(|n| {
            matches!(n.kind(), SyntaxKind::VARIABLE_DECL | SyntaxKind::PARAM)
                && child(&n, SyntaxKind::NAME)
                    .is_some_and(|d| compact(&d).eq_ignore_ascii_case(&name))
        });
        if declared_here {
            return;
        }
        let this = ScriptRef::Source(file);
        if !self.chain_complete(this) {
            return;
        }
        // A function, a state or a struct used as a value: another error,
        // which the compiler words better.
        let elsewhere = self
            .callables(this)
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&name))
            || self.chain(this).any(|script| match script {
                ScriptRef::Source(f) => self.items(f).named(&name).next().is_some(),
                ScriptRef::Native(_) => false,
            });
        if elsewhere {
            return;
        }
        let locals = locals_in_scope(file, name_ref, name_ref.text_range().start(), true);
        let mut candidates: Vec<&str> = locals
            .iter()
            .filter_map(|d| match d {
                Def::Local { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        candidates.extend(self.values(this));
        self.unknown(
            name_ref,
            format!("`{name}` is not declared"),
            candidates,
            out,
        );
    }
}
