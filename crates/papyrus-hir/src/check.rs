//! Semantic diagnostics: calls, types, members, returns, states, events,
//! read-only properties, `Global` functions and dead code.
//!
//! Each one is certain. A call is only judged when its callee is resolved,
//! a type when its whole `extends` chain is known, a missing member when
//! every script it could come from is known. Whatever is unknown stays
//! quiet: missing information is never an error.

use papyrus_dialect::GameId;
use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use crate::SourceFile;
use crate::calls::{Arg, SignatureParam, arg_list, args, callee};
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, is_callee, locals_in_scope};
use crate::ty::Ty;
use crate::types::Object;

mod case;
mod decls;
mod exprs;
mod habits;
mod misuse;
mod usage;

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
    /// `GotoState` to a state the script does not declare.
    UnknownState,
    /// An event whose parameters differ from the one it implements.
    EventSignature,
    /// An assignment to an `AutoReadOnly` or `Const` property, or one
    /// without `Set`.
    ReadOnly,
    /// An instance member reached without an instance: from a `Global`
    /// function, or on the script itself.
    NotGlobal,
    /// Statements after one that always returns.
    Unreachable,
    /// A local variable never read.
    UnusedLocal,
    /// A parameter never read, after the last one read.
    UnusedParam,
    /// A member only SKSE declares, in a Skyrim project without SKSE.
    SkseMember,
    /// A script neither the project nor its imports declare, once they
    /// hold the game's sources.
    UnknownScript,
    /// A local declared twice in a block, named like a parameter or a
    /// variable of the script, or hiding one of an enclosing block.
    DuplicateLocal,
    /// A name written with another case than its declaration.
    NameCase,
    /// A `ScriptName` other than the file's name.
    ScriptName,
    /// A script among its own ancestors.
    ExtendsCycle,
    /// A member declared twice: a function or event in the same state, a
    /// variable, a property, a second `Auto` state.
    DuplicateMember,
    /// `Return value` in a function without a return type or in an event,
    /// `Return` alone in a typed function.
    ReturnValue,
    /// A function or event whose signature differs from the one it
    /// overrides: the parent's, or the empty state's.
    OverrideSignature,
    /// A function in a state but in no empty state.
    StateFunction,
    /// A property's `Get` or `Set` of the wrong shape, another function in
    /// it, an initial value where none is allowed or missing.
    PropertyAccessor,
    /// `new T[n]` of a size the compiler refuses.
    ArraySize,
    /// An assignment to something that cannot be assigned: a call,
    /// `.Length`.
    NotAssignable,
    /// An initial or default value that is not a literal.
    NotLiteral,
    /// A parameter without default after one with a default.
    DefaultOrder,
    /// A `Native` function in a script that is not `Native`.
    NativeFunction,
    /// An integer literal that does not fit 32 bits.
    IntRange,
    /// `RegisterForUpdate`, which keeps running after the mod is removed.
    RepeatingUpdate,
    /// `Game.GetPlayer()` called on each iteration of a loop.
    GetPlayerInLoop,
    /// A comparison with `True` or `False`.
    BoolComparison,
    /// `x = x`.
    SelfAssign,
    /// `x == x`, `x < x`: always the same answer.
    SelfCompare,
    /// A script variable never read.
    UnusedVariable,
    /// An `Import` nothing uses.
    UnusedImport,
}

/// A text replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    /// What is replaced.
    pub range: TextRange,
    /// The new text.
    pub text: String,
}

/// A fix to suggest; never applied automatically: it is likely, not
/// certain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    /// What it does: "Replace with `OnActivate`".
    pub title: String,
    /// The replacements, in the file of the diagnostic.
    pub edits: Vec<Replacement>,
}

impl Fix {
    /// Replaces `range` with `name`, the likely intended one.
    fn replace(range: TextRange, name: &str) -> Fix {
        Fix {
            title: format!("Replace with `{name}`"),
            edits: vec![Replacement {
                range,
                text: name.to_owned(),
            }],
        }
    }

    /// Removes the lines of `range`, in `text`, their line end included.
    fn remove_lines(title: &str, text: &str, range: TextRange) -> Fix {
        let (start, end) = (usize::from(range.start()), usize::from(range.end()));
        let start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
        Fix {
            title: title.to_owned(),
            edits: vec![Replacement {
                range: TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32)),
                text: String::new(),
            }],
        }
    }
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
    /// Likely fixes: the intended name of a typo, a cast, a missing
    /// argument, a function to create.
    pub fixes: Vec<Fix>,
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

/// `value as ty`, parenthesized unless `value` binds tighter than `as`.
fn cast_fix(value: &SyntaxNode, ty: &Ty) -> Fix {
    let title = format!("Cast with `as {ty}`");
    // `x as Form`: the cast changes its type.
    if let Some(target) = (value.kind() == SyntaxKind::CAST_EXPR)
        .then(|| child(value, SyntaxKind::TYPE_REF))
        .flatten()
    {
        return Fix {
            title,
            edits: vec![Replacement {
                range: trimmed(&target),
                text: ty.to_string(),
            }],
        };
    }
    let range = trimmed(value);
    let atomic = matches!(
        value.kind(),
        SyntaxKind::LITERAL
            | SyntaxKind::NAME_REF
            | SyntaxKind::PAREN_EXPR
            | SyntaxKind::CALL_EXPR
            | SyntaxKind::FIELD_EXPR
            | SyntaxKind::INDEX_EXPR
    );
    let mut edits = Vec::new();
    if !atomic {
        edits.push(Replacement {
            range: TextRange::empty(range.start()),
            text: "(".to_owned(),
        });
    }
    edits.push(Replacement {
        range: TextRange::empty(range.end()),
        text: format!("{} as {ty}", if atomic { "" } else { ")" }),
    });
    Fix { title, edits }
}

/// The value a parameter of type `ty` takes when nothing better is known.
fn placeholder(ty: &Ty) -> &'static str {
    match ty {
        Ty::Int => "0",
        Ty::Float => "0.0",
        Ty::Bool => "false",
        Ty::String => "\"\"",
        _ => "None",
    }
}

/// Adds the missing arguments of a call before its `)`: positionally when
/// the call names none, by name otherwise.
fn missing_args_fix(
    list: &SyntaxNode,
    params: &[SignatureParam],
    given: &[bool],
    positional: bool,
) -> Option<Fix> {
    let close = list
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == SyntaxKind::R_PAREN)?;
    let missing: Vec<usize> = (0..params.len())
        .filter(|&i| !given[i] && params[i].default.is_none())
        .collect();
    let last = *missing.last()?;
    let value = |p: &SignatureParam| placeholder(&p.ty).to_owned();
    let values: Vec<String> = if positional {
        // The optional parameters before the last missing one are passed
        // their default value.
        let first = given.iter().position(|g| !g).unwrap_or(params.len());
        params[first..=last]
            .iter()
            .map(|p| p.default.clone().unwrap_or_else(|| value(p)))
            .collect()
    } else {
        missing
            .iter()
            .map(|&i| format!("{} = {}", params[i].name, value(&params[i])))
            .collect()
    };
    let empty = !list.children().any(|c| c.kind() == SyntaxKind::ARG);
    let names: Vec<String> = missing
        .iter()
        .map(|&i| format!("`{}`", params[i].name))
        .collect();
    Some(Fix {
        title: format!(
            "Add the missing {} {}",
            if missing.len() == 1 {
                "argument"
            } else {
                "arguments"
            },
            names.join(", ")
        ),
        edits: vec![Replacement {
            range: TextRange::empty(close.text_range().start()),
            text: format!("{}{}", if empty { "" } else { ", " }, values.join(", ")),
        }],
    })
}

impl Semantics<'_> {
    /// True if `from` can be cast to `to` with `as`: a number from a string
    /// or a bool, an integer from a float, an object to one of its
    /// descendants.
    fn castable(&self, from: &Ty, to: &Ty) -> bool {
        match (from, to) {
            (Ty::Float | Ty::String | Ty::Bool, Ty::Int) | (Ty::String | Ty::Bool, Ty::Float) => {
                true
            }
            (Ty::Object(_), Ty::Object(_)) => self.coerces(to, from) == Some(true),
            _ => false,
        }
    }

    /// The semantic problems of `file`.
    ///
    /// The native scripts depend on the target. Without one, the file is
    /// checked against each game's, and only what all of them report is
    /// kept: wrong whatever the game.
    pub fn check(&self, file: SourceFile) -> Vec<Diagnostic> {
        if self.game().is_some() {
            return self.check_for_target(file);
        }
        let [skyrim, fallout4, starfield] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield]
            .map(|game| self.assuming(game).check_for_target(file));
        starfield
            .into_iter()
            .filter(|d| {
                [&skyrim, &fallout4].iter().all(|others| {
                    others
                        .iter()
                        .any(|o| o.problem == d.problem && o.range == d.range)
                })
            })
            .collect()
    }

    /// [`Semantics::check_vanilla`], and the scripts `file` names: unknown
    /// ones once the game's sources are known, SKSE's in a Skyrim project
    /// without SKSE.
    fn check_for_target(&self, file: SourceFile) -> Vec<Diagnostic> {
        let mut found = self.check_vanilla(file);
        let without_skse = self.skyrim() && !self.library().skse;
        let sources = self.has_game_sources();
        if !without_skse && !sources {
            return found;
        }
        let text = file.text(self.db());
        let invite = |range: TextRange, what: &str| Diagnostic {
            problem: Problem::SkseMember,
            range,
            message: format!(
                "`{}` {what} SKSE, which this project does not use: install SKSE, then set \
                 `skse = true` in papyrus.toml or add its sources to `imports`",
                &text[range]
            ),
            fixes: Vec::new(),
        };
        let unknown = |d: &Diagnostic| d.problem == Problem::UnknownMember;
        if without_skse && found.iter().any(unknown) {
            // Known with SKSE: the member is SKSE's.
            let extended = self.with_skse().check_vanilla(file);
            for diagnostic in found.iter_mut().filter(|d| unknown(d)) {
                if !extended
                    .iter()
                    .any(|e| unknown(e) && e.range == diagnostic.range)
                {
                    *diagnostic = invite(diagnostic.range, "comes from");
                }
            }
        }
        let skse = self.with_skse();
        for (name, range, receiver) in self.unknown_scripts(file) {
            if without_skse && skse.resolve_type(file, &name).is_some() {
                found.push(invite(range, "is a script of"));
            } else if sources && receiver {
                found.push(Diagnostic {
                    problem: Problem::UnknownScript,
                    range,
                    message: format!("`{name}` is neither a variable nor a script"),
                    fixes: Vec::new(),
                });
            } else if sources {
                let suggestion = closest(&name, self.script_names());
                let mut message = format!(
                    "unknown script `{name}`: neither the project nor its imports declare it"
                );
                if let Some(s) = suggestion {
                    message.push_str(&format!(" — did you mean `{s}`?"));
                }
                found.push(Diagnostic {
                    problem: Problem::UnknownScript,
                    range,
                    message,
                    fixes: suggestion
                        .map(|s| Fix::replace(range, s))
                        .into_iter()
                        .collect(),
                });
            }
        }
        found
    }

    /// True once the project or its imports hold the game's sources: then a
    /// script they do not declare does not exist.
    fn has_game_sources(&self) -> bool {
        ["ObjectReference", "Actor", "Form"]
            .iter()
            .all(|name| matches!(self.script(name), Some(ScriptRef::Source(_))))
    }

    /// Every script the project knows: its own, imported, native.
    fn script_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .index()
            .files()
            .map(|f| self.items(f).name.as_str())
            .collect();
        names.extend(
            papyrus_stdlib::scripts(self.library())
                .iter()
                .map(|s| s.name),
        );
        names
    }

    /// The scripts `file` names that resolve to nothing: `(name, range,
    /// receiver)`. Types — `extends`, `import`, declarations, casts — and the
    /// receivers of calls, which may also be misspelled variables.
    fn unknown_scripts(&self, file: SourceFile) -> Vec<(String, TextRange, bool)> {
        // Under an unknown ancestor, a receiver may be an inherited property.
        let complete = self.chain_complete(ScriptRef::Source(file));
        let mut out = Vec::new();
        for node in self.root(file).descendants() {
            match node.kind() {
                SyntaxKind::TYPE_REF => {
                    let name = compact(&node).trim_end_matches("[]").to_owned();
                    // `CustomEventName`, `ScriptEventName`: the compiler's
                    // own types, keywords like `int`.
                    let keyword = node
                        .descendants_with_tokens()
                        .any(|t| t.kind().is_keyword());
                    if !keyword
                        && matches!(Ty::parse(&name), Ty::Object(_))
                        && self.resolve_type(file, &name).is_none()
                    {
                        out.push((name, crate::semantics::type_name_range(&node), false));
                    }
                }
                SyntaxKind::NAME_REF
                    if complete
                        && node.prev_sibling().is_none()
                        && node
                            .parent()
                            .is_some_and(|p| p.kind() == SyntaxKind::FIELD_EXPR)
                        && self.resolve_name_ref(file, &node).is_none() =>
                {
                    let name = compact(&node);
                    if !name.contains(':') {
                        out.push((name, trimmed(&node), true));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The problems of `file` against the native scripts in effect.
    fn check_vanilla(&self, file: SourceFile) -> Vec<Diagnostic> {
        let root = self.root(file);
        let errors: Vec<TextRange> = crate::parse(self.db(), file, self.syntax())
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
        self.check_decls(file, &root, &broken, &mut out);
        self.check_values(&root, &broken, &mut out);
        self.check_unused_script_level(file, &root, &mut out);
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
                    if node.kind() == SyntaxKind::EVENT_DECL {
                        self.check_event_signature(file, &node, &mut out);
                    }
                    self.check_global_body(file, &node, &body, &mut out);
                    usage::check_unreachable(&body, &mut out);
                    let duplicated = usage::check_duplicates(&root, &node, &body, &mut out);
                    self.check_unused(file, &node, &body, &duplicated, &mut out);
                    for inner in body.descendants() {
                        self.check_expr(file, &inner, &mut out);
                        self.check_habits(file, &inner, &mut out);
                        match inner.kind() {
                            SyntaxKind::CALL_EXPR => {
                                self.check_call(file, &inner, &mut out);
                                if let Some(callee) = callee(&inner) {
                                    self.check_state(file, &inner, &callee, &mut out);
                                    self.check_static_call(file, &callee, &mut out);
                                }
                            }
                            SyntaxKind::ASSIGN_STMT => {
                                self.check_assign(file, &inner, &mut out);
                                self.check_readonly(file, &inner, &mut out);
                            }
                            SyntaxKind::VARIABLE_DECL => self.check_init(file, &inner, &mut out),
                            SyntaxKind::RETURN_STMT => {
                                self.check_return_value(file, &node, &inner, &mut out);
                            }
                            SyntaxKind::NAME_REF => {
                                self.check_value(file, &node, &inner, &mut out);
                                self.check_case(file, &inner, &mut out);
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
                SyntaxKind::TYPE_REF if !broken(&node) => {
                    self.check_type_case(file, &node, &mut out);
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
            fixes: Vec::new(),
        });
    }

    /// `Return value`: of the declared type, and only in a typed function;
    /// `Return` alone only in a function without return type or an event.
    fn check_return_value(
        &self,
        file: SourceFile,
        decl: &SyntaxNode,
        ret: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let returns =
            child(decl, SyntaxKind::TYPE_REF).filter(|_| decl.kind() == SyntaxKind::FUNCTION_DECL);
        let name = decl_name(decl).map(|n| compact(&n)).unwrap_or_default();
        let report = |message: String| Diagnostic {
            problem: Problem::ReturnValue,
            range: trimmed(ret),
            message,
            fixes: Vec::new(),
        };
        let (value, returns) = match (ret.children().next(), returns) {
            (None, None) => return,
            (None, Some(returns)) => {
                out.push(report(format!(
                    "`{name}` returns `{}`: `Return` needs a value",
                    compact(&returns)
                )));
                return;
            }
            (Some(value), None) => {
                // `Return None` is accepted: vanilla Starfield writes it.
                if self.type_of(file, &value) == Ty::None {
                    return;
                }
                // Skyrim's compiler accepts `Return Foo()` when `Foo`
                // returns nothing either; later ones do not.
                let maybe_void = value.kind() == SyntaxKind::CALL_EXPR
                    && self.returns_nothing(file, &value) != Some(false);
                if !maybe_void || self.game().is_some_and(|g| g.rank() >= 1) {
                    let what = if decl.kind() == SyntaxKind::EVENT_DECL {
                        "an event"
                    } else {
                        "a function without return type"
                    };
                    out.push(report(format!(
                        "`{name}` is {what}: it cannot return a value"
                    )));
                }
                return;
            }
            (Some(value), Some(returns)) => (value, returns),
        };
        let expected = self.qualify(file, Ty::parse(&compact(&returns)));
        self.mismatch(file, &value, &expected, out, |found, to| {
            format!("`{name}` returns `{to}`, found `{found}`")
        });
    }

    /// Whether a call returns nothing: `Some(true)` for a function without
    /// return type or an event, `None` for an unresolved callee or anything
    /// but a call.
    fn returns_nothing(&self, file: SourceFile, expr: &SyntaxNode) -> Option<bool> {
        if expr.kind() != SyntaxKind::CALL_EXPR {
            return None;
        }
        let callee = match expr.children().next()? {
            c if c.kind() == SyntaxKind::FIELD_EXPR => crate::semantics::member_name_ref(&c)?,
            c => c,
        };
        if callee.kind() != SyntaxKind::NAME_REF {
            return None;
        }
        match self.resolve_name_ref(file, &callee)? {
            Def::Item { file, index } => {
                let item = self.item(file, index);
                match item.kind {
                    ItemKind::Function => Some(item.ty.is_none()),
                    ItemKind::Event => Some(true),
                    _ => None,
                }
            }
            Def::NativeFunction {
                function, event, ..
            } => Some(event || function.returns.is_none()),
            _ => None,
        }
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
        // Nothing passes for `None`: vanilla Skyrim assigns such calls to
        // objects and passes them as booleans; never as numbers.
        if self.returns_nothing(file, value) == Some(true) {
            if self.coerces(&Ty::None, expected) == Some(false) {
                self.check_void(file, value, &format!("used as a `{expected}`"), out);
            }
            return;
        }
        let found = self.type_of(file, value);
        if self.coerces(&found, expected) == Some(false) {
            let castable = self.castable(&found, expected);
            let mut message = message(&found, expected);
            if castable {
                message.push_str(&format!(" — cast it with `as {expected}`"));
            }
            out.push(Diagnostic {
                problem: Problem::Mismatch,
                range: trimmed(value),
                message,
                fixes: castable
                    .then(|| cast_fix(value, expected))
                    .into_iter()
                    .collect(),
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
                fixes: Vec::new(),
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
                    fixes: Vec::new(),
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
                        fixes: suggestion
                            .map(|s| Fix::replace(trimmed(arg_name), s))
                            .into_iter()
                            .collect(),
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
                fixes: missing_args_fix(&list, params, &given, named.is_empty())
                    .into_iter()
                    .collect(),
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

    /// A function of `script` named as a value, `MyScript.Run` for
    /// `MyScript.Run()`: what it is, and the parentheses to call it.
    fn not_called(
        &self,
        script: ScriptRef,
        node: &SyntaxNode,
        owner: Option<&str>,
    ) -> Option<Diagnostic> {
        let name = compact(node);
        let declared = self
            .callables(script)
            .into_iter()
            .find(|c| c.eq_ignore_ascii_case(&name))?;
        let of = owner.map(|o| format!(" of `{o}`")).unwrap_or_default();
        let range = trimmed(node);
        Some(Diagnostic {
            problem: Problem::UnknownMember,
            range,
            // The declared name, whatever the case written.
            message: format!(
                "`{declared}` is a function{of}, not a variable: call it with parentheses, \
                 `{declared}()`"
            ),
            fixes: vec![Fix {
                title: format!("Call `{declared}()`"),
                edits: vec![Replacement {
                    range,
                    text: format!("{declared}()"),
                }],
            }],
        })
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
            fixes: suggestion
                .map(|s| Fix::replace(trimmed(node), s))
                .into_iter()
                .collect(),
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
            if script == ScriptRef::Source(file) {
                self.offer_creation(file, callee, out);
            }
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
        self.offer_creation(file, callee, out);
    }

    /// Adds "Create the function" to the diagnostic just reported.
    fn offer_creation(&self, file: SourceFile, callee: &SyntaxNode, out: &mut [Diagnostic]) {
        if let (Some(fix), Some(last)) = (self.create_function_fix(file, callee), out.last_mut()) {
            last.fixes.push(fix);
        }
    }

    /// `Function Name(...)` at the end of `file`, with a parameter per
    /// argument, of its type; `None` when one is unknown.
    fn create_function_fix(&self, file: SourceFile, callee: &SyntaxNode) -> Option<Fix> {
        let call = callee
            .ancestors()
            .find(|n| n.kind() == SyntaxKind::CALL_EXPR)?;
        let list = arg_list(&call)?;
        let mut params: Vec<(String, String)> = Vec::new();
        for (index, arg) in args(&list).iter().enumerate() {
            let value = arg.value.as_ref()?;
            let ty = self.type_of(file, value);
            if matches!(ty, Ty::Unknown | Ty::None) {
                return None;
            }
            let plain = value.kind() == SyntaxKind::NAME_REF
                && value
                    .first_token()
                    .is_some_and(|t| t.kind() == SyntaxKind::IDENT);
            let mut name = match &arg.name {
                Some(name) => compact(name),
                None if plain => compact(value),
                None => format!("param{}", index + 1),
            };
            if params.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) {
                name = format!("{name}{}", index + 1);
            }
            params.push((ty.to_string(), name));
        }
        let decl = call
            .ancestors()
            .find(|n| matches!(n.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL))?;
        let global = decl.kind() == SyntaxKind::FUNCTION_DECL
            && child(&decl, SyntaxKind::FLAG_LIST).is_some_and(|list| {
                list.children()
                    .filter_map(|f| f.first_token())
                    .any(|t| t.kind() == SyntaxKind::GLOBAL_KW)
            });
        let returns = self
            .expected_type(file, &call, &decl)
            .map(|ty| format!("{ty} "))
            .unwrap_or_default();
        let name = compact(callee);
        let params: Vec<String> = params.iter().map(|(t, n)| format!("{t} {n}")).collect();
        let text = file.text(self.db());
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let separator = if text.ends_with('\n') || text.is_empty() {
            eol.to_owned()
        } else {
            eol.repeat(2)
        };
        Some(Fix {
            title: format!("Create the function `{name}`"),
            edits: vec![Replacement {
                range: TextRange::empty(TextSize::of(text)),
                text: format!(
                    "{separator}{returns}Function {name}({}){}{eol}EndFunction{eol}",
                    params.join(", "),
                    if global { " Global" } else { "" }
                ),
            }],
        })
    }

    /// The type the value of `call` must have, from where it goes: a
    /// variable initialized, assigned, a value returned.
    fn expected_type(&self, file: SourceFile, call: &SyntaxNode, decl: &SyntaxNode) -> Option<Ty> {
        let parent = call.parent()?;
        let ty = match parent.kind() {
            SyntaxKind::VARIABLE_DECL => {
                Ty::parse(&compact(&child(&parent, SyntaxKind::TYPE_REF)?))
            }
            SyntaxKind::ASSIGN_STMT if parent.children().nth(1).as_ref() == Some(call) => {
                self.type_of(file, &parent.children().next()?)
            }
            SyntaxKind::RETURN_STMT if decl.kind() == SyntaxKind::FUNCTION_DECL => {
                Ty::parse(&compact(&child(decl, SyntaxKind::TYPE_REF)?))
            }
            _ => return None,
        };
        let ty = self.qualify(file, ty);
        (!matches!(ty, Ty::Unknown | Ty::None)).then_some(ty)
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
            // `MyScript.Run`: a script names only its global functions.
            let script = match self.resolve_name_ref(file, &receiver) {
                Some(Def::Script { file }) => Some(ScriptRef::Source(file)),
                Some(Def::NativeScript(script)) => Some(ScriptRef::Native(script)),
                _ => None,
            };
            if let Some(script) = script.filter(|_| receiver.kind() == SyntaxKind::NAME_REF) {
                let owner = self.script_name(script);
                out.extend(self.not_called(script, name_ref, Some(owner)));
                return;
            }
            match self.type_of(file, &receiver) {
                Ty::Array(_) => self.unknown(
                    name_ref,
                    format!("arrays have no member `{name}`, only `Length`"),
                    vec!["Length"],
                    out,
                ),
                Ty::Object(object) => match self.object(&object) {
                    Some(Object::Script(script)) if self.chain_complete(script) => {
                        let owner = self.script_name(script);
                        if let Some(d) = self.not_called(script, name_ref, Some(owner)) {
                            out.push(d);
                            return;
                        }
                        let message =
                            format!("`{owner}` and its parents have no property `{name}`");
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
        if let Some(d) = self.not_called(this, name_ref, None) {
            out.push(d);
            return;
        }
        // A state or a struct used as a value: another error, which the
        // compiler words better.
        let elsewhere = self.chain(this).any(|script| match script {
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
