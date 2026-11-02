//! What the compiler refuses in declarations: the script's name and
//! ancestry, members declared twice, overrides that do not match, functions
//! only a state declares, the shape of properties.

use std::collections::HashMap;

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use super::misuse::type_key;
use super::{Diagnostic, Fix, Problem, Replacement, child, decl_name};
use crate::SourceFile;
use crate::builtins;
use crate::items::{ItemKind, compact, has_flag, trimmed};
use crate::semantics::{Def, Namespace, ScriptRef, Semantics};

/// The line of `offset` in `text`, from 1.
fn line_of(text: &str, offset: TextSize) -> usize {
    text[..usize::from(offset)].matches('\n').count() + 1
}

/// A number literal's value, for comparing defaults: `1.0` and `1.00`,
/// `16` and `0x10` are the same for the compiler.
fn number(text: &str) -> Option<f64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text),
    };
    let value = match digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        Some(hex) => i64::from_str_radix(hex, 16).ok()? as f64,
        None => digits.parse::<f64>().ok()?,
    };
    Some(if negative { -value } else { value })
}

/// Whether two defaults are the same for the compiler: numbers by value,
/// anything else — `True`, `None`, strings — as written, case included.
fn same_default(a: &str, b: &str) -> bool {
    match (number(a), number(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}

/// The value nodes of the parameters of `decl`, `None` without default.
fn default_nodes(decl: &SyntaxNode) -> Vec<Option<SyntaxNode>> {
    child(decl, SyntaxKind::PARAM_LIST)
        .map(|list| {
            list.children()
                .filter(|p| p.kind() == SyntaxKind::PARAM)
                .map(|p| {
                    p.children_with_tokens()
                        .skip_while(|e| e.kind() != SyntaxKind::EQ)
                        .skip(1)
                        .find_map(|e| e.into_node())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Where a declaration of the script lives: the empty state, a state.
fn state_of(decl: &SyntaxNode) -> Option<String> {
    decl.parent()
        .filter(|p| p.kind() == SyntaxKind::STATE_DECL)
        .and_then(|s| decl_name(&s))
        .map(|n| compact(&n))
}

impl Semantics<'_> {
    /// The declaration-level problems of `file`.
    pub(super) fn check_decls(
        &self,
        file: SourceFile,
        root: &SyntaxNode,
        broken: &dyn Fn(&SyntaxNode) -> bool,
        out: &mut Vec<Diagnostic>,
    ) {
        self.check_script_name(file, out);
        self.check_cycle(file, root, out);
        self.check_duplicate_members(file, root, broken, out);
        for node in root.children() {
            let members: Vec<SyntaxNode> = if node.kind() == SyntaxKind::STATE_DECL {
                node.children().collect()
            } else {
                vec![node.clone()]
            };
            for member in members {
                if broken(&member) {
                    continue;
                }
                match member.kind() {
                    SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
                        self.check_override(file, &member, out);
                        self.check_state_function(file, &member, out);
                        if self.since_fallout4() {
                            self.check_fallout4_function(root, &member, out);
                        }
                    }
                    SyntaxKind::PROPERTY_DECL => self.check_property(file, &member, out),
                    SyntaxKind::GROUP_DECL => {
                        for property in member
                            .children()
                            .filter(|c| c.kind() == SyntaxKind::PROPERTY_DECL)
                            .filter(|c| !broken(c))
                        {
                            self.check_property(file, &property, out);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// From Fallout 4 on.
    pub(super) fn since_fallout4(&self) -> bool {
        self.game().is_some_and(|g| g.rank() >= 1)
    }

    // ── The script ────────────────────────────────────────────────────────

    /// `ScriptName Foo` in `Bar.psc`.
    fn check_script_name(&self, file: SourceFile, out: &mut Vec<Diagnostic>) {
        let items = self.items(file);
        let path = file.path(self.db());
        let psc = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("psc"));
        let (Some(stem), true) = (path.file_stem().and_then(|s| s.to_str()), psc) else {
            return;
        };
        let short = items.name.rsplit(':').next().unwrap_or_default();
        if short.is_empty() || short.eq_ignore_ascii_case(stem) {
            // Skyrim's scripts have no namespace, whatever their folder.
            let known = self
                .game()
                .is_some_and(|g| g.rank() >= 1)
                .then(|| self.name_from_roots(file))
                .flatten();
            if let Some(expected) = known {
                out.push(Diagnostic {
                    problem: Problem::ScriptName,
                    range: items.name_range,
                    message: format!(
                        "the folders of this file make it `{expected}`: the compiler wants them \
                         to match"
                    ),
                    fixes: vec![Fix {
                        title: format!("Rename the script to `{expected}`"),
                        edits: vec![Replacement {
                            range: items.name_range,
                            text: expected,
                        }],
                    }],
                });
                return;
            }
            // Otherwise the root is unknown, but a namespace's folders are
            // not: only a name without any cannot be told from a file under
            // its root.
            if let Some((namespace, _)) = items.name.rsplit_once(':')
                && self.script_root(file).is_none()
            {
                out.push(Diagnostic {
                    problem: Problem::ScriptName,
                    range: items.name_range,
                    message: format!(
                        "the script is in the namespace `{namespace}`, but its file is not in \
                         that folder: the compiler wants them to match"
                    ),
                    fixes: vec![Fix {
                        title: format!("Rename the script to `{stem}`"),
                        edits: vec![Replacement {
                            range: items.name_range,
                            text: stem.to_owned(),
                        }],
                    }],
                });
            }
            return;
        }
        let range = TextRange::new(
            items.name_range.end() - TextSize::of(short),
            items.name_range.end(),
        );
        out.push(Diagnostic {
            problem: Problem::ScriptName,
            range,
            message: format!(
                "the script is named `{short}`, but its file `{stem}.psc`: the compiler wants the \
                 same name"
            ),
            fixes: vec![Fix {
                title: format!("Rename the script to `{stem}`"),
                edits: vec![Replacement {
                    range,
                    text: stem.to_owned(),
                }],
            }],
        });
    }

    /// `ScriptName A extends B` where `B` extends `A`.
    fn check_cycle(&self, file: SourceFile, root: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let me = ScriptRef::Source(file);
        let mut names = vec![self.script_name(me)];
        let mut looped = false;
        for script in self.chain(me).skip(1) {
            names.push(self.script_name(script));
            if script == me {
                looped = true;
                break;
            }
        }
        if !looped {
            return;
        }
        let Some(parent) = child(root, SyntaxKind::SCRIPT_HEADER)
            .and_then(|h| child(&h, SyntaxKind::EXTENDS_CLAUSE))
            .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
        else {
            return;
        };
        let path = names
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(" extends ");
        out.push(Diagnostic {
            problem: Problem::ExtendsCycle,
            range: trimmed(&parent),
            message: format!("the script is its own ancestor: {path}"),
            fixes: Vec::new(),
        });
    }

    // ── Members declared twice ────────────────────────────────────────────

    /// A function or event twice in a state, a variable or a property
    /// twice, a second `Auto` state; from Fallout 4, a property named like a
    /// variable.
    fn check_duplicate_members(
        &self,
        file: SourceFile,
        root: &SyntaxNode,
        broken: &dyn Fn(&SyntaxNode) -> bool,
        out: &mut Vec<Diagnostic>,
    ) {
        let text = file.text(self.db());
        let shared = self.since_fallout4();
        // Lowercase key → (kind, line of the first one).
        let mut values: HashMap<String, (SyntaxKind, usize)> = HashMap::new();
        let mut callables: HashMap<(String, String, String), usize> = HashMap::new();
        let mut auto: Option<String> = None;

        let mut value = |node: &SyntaxNode, out: &mut Vec<Diagnostic>| {
            let Some(name) = decl_name(node) else {
                return;
            };
            let written = compact(&name);
            let key = written.to_ascii_lowercase();
            let line = line_of(text, trimmed(&name).start());
            match values.get(&key) {
                Some(&(kind, first)) if kind == node.kind() || shared => {
                    let what = |k: SyntaxKind| {
                        if k == SyntaxKind::PROPERTY_DECL {
                            "property"
                        } else {
                            "variable"
                        }
                    };
                    let mut message = format!(
                        "`{written}` is already a {} of the script, line {first}",
                        what(kind)
                    );
                    if kind != node.kind() {
                        message.push_str(
                            ": from Fallout 4, a property cannot share its name with a variable",
                        );
                    }
                    out.push(Diagnostic {
                        problem: Problem::DuplicateMember,
                        range: trimmed(&name),
                        message,
                        fixes: Vec::new(),
                    });
                }
                Some(_) => {}
                None => {
                    values.insert(key, (node.kind(), line));
                }
            }
        };

        for node in root.children().filter(|n| !broken(n)) {
            match node.kind() {
                SyntaxKind::VARIABLE_DECL | SyntaxKind::PROPERTY_DECL => value(&node, out),
                SyntaxKind::GROUP_DECL => {
                    for property in node
                        .children()
                        .filter(|c| c.kind() == SyntaxKind::PROPERTY_DECL && !broken(c))
                    {
                        value(&property, out);
                    }
                }
                _ => {}
            }
        }

        let members = root.children().flat_map(|node| {
            if node.kind() == SyntaxKind::STATE_DECL {
                node.children().collect::<Vec<_>>()
            } else {
                vec![node]
            }
        });
        for node in members.filter(|n| !broken(n)) {
            if !matches!(
                node.kind(),
                SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL
            ) {
                continue;
            }
            let Some(name) = decl_name(&node) else {
                continue;
            };
            let state = state_of(&node);
            // `Event Actor.OnDeath`: one per script listened to.
            let remote = (node.kind() == SyntaxKind::EVENT_DECL)
                .then(|| child(&node, SyntaxKind::TYPE_REF))
                .flatten()
                .map(|t| compact(&t).to_ascii_lowercase())
                .unwrap_or_default();
            let written = compact(&name);
            let key = (
                state.clone().unwrap_or_default().to_ascii_lowercase(),
                remote,
                written.to_ascii_lowercase(),
            );
            let line = line_of(text, trimmed(&name).start());
            if let Some(first) = callables.get(&key) {
                let place = match &state {
                    Some(state) => format!("state `{state}`"),
                    None => "the empty state".to_owned(),
                };
                out.push(Diagnostic {
                    problem: Problem::DuplicateMember,
                    range: trimmed(&name),
                    message: format!("`{written}` is already declared in {place}, line {first}"),
                    fixes: Vec::new(),
                });
            } else {
                callables.insert(key, line);
            }
        }

        for state in root
            .children()
            .filter(|n| n.kind() == SyntaxKind::STATE_DECL)
        {
            let automatic = state
                .children_with_tokens()
                .any(|e| e.kind() == SyntaxKind::AUTO_KW);
            let Some(name) = decl_name(&state).filter(|_| automatic) else {
                continue;
            };
            let written = compact(&name);
            match &auto {
                Some(first) if !first.eq_ignore_ascii_case(&written) => {
                    out.push(Diagnostic {
                        problem: Problem::DuplicateMember,
                        range: trimmed(&name),
                        message: format!(
                            "`{first}` is already the `Auto` state: a script starts in one state \
                             only"
                        ),
                        fixes: Vec::new(),
                    });
                }
                Some(_) => {}
                None => auto = Some(written),
            }
        }
    }

    // ── Overrides ─────────────────────────────────────────────────────────

    /// What `item` overrides: the empty state's for one in a state, else
    /// the closest ancestor's; with where it comes from, for messages.
    fn overridden(&self, file: SourceFile, index: usize) -> Option<(Def, String, bool)> {
        let items = self.items(file);
        let item = &items.items[index];
        if item.container.is_some() {
            let empty = items
                .named(&item.name)
                .find(|(_, i)| i.kind.is_callable() && i.container.is_none() && i.remote.is_none());
            if let Some((index, _)) = empty {
                return Some((
                    Def::Item { file, index },
                    "the empty state".to_owned(),
                    false,
                ));
            }
        }
        self.chain(ScriptRef::Source(file))
            .skip(1)
            .find_map(|script| {
                self.own_member(script, &item.name, Namespace::Callable)
                    .map(|def| (def, format!("`{}`", self.script_name(script)), true))
            })
    }

    /// A function or event whose signature differs from the one it
    /// overrides.
    fn check_override(&self, file: SourceFile, decl: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(name) = decl_name(decl) else {
            return;
        };
        let range = trimmed(&name);
        let items = self.items(file);
        let Some((index, item)) = items
            .items
            .iter()
            .enumerate()
            .find(|(_, i)| i.kind.is_callable() && i.name_range == range)
        else {
            return;
        };
        if item.remote.is_some() {
            return;
        }
        let Some((origin, from, inherited)) = self.overridden(file, index) else {
            return;
        };
        let (origin_global, origin_native, origin_event) = match &origin {
            Def::Item { file, index } => {
                let o = self.item(*file, *index);
                (o.global, o.native, o.kind == ItemKind::Event)
            }
            Def::NativeFunction {
                function, event, ..
            } => (function.global, function.native, *event),
            _ => return,
        };
        let report = |message: String, fixes: Vec<Fix>| Diagnostic {
            problem: Problem::OverrideSignature,
            range,
            message,
            fixes,
        };
        if inherited && origin_global {
            if item.global {
                out.push(report(
                    format!(
                        "`{}` is already a `Global` function of {from}: a `Global` function \
                         cannot be redefined",
                        item.name
                    ),
                    Vec::new(),
                ));
            }
            return;
        }
        let event = item.kind == ItemKind::Event;
        if inherited && event != origin_event && self.since_fallout4() {
            let (this, that) = if event {
                ("an event", "function")
            } else {
                ("a function", "event")
            };
            out.push(report(
                format!(
                    "`{}` is {this}, but an {that} of {from} has this name: from Fallout 4, the \
                     compiler refuses it",
                    item.name
                ),
                Vec::new(),
            ));
            return;
        }
        let script_native = child(&self.root(file), SyntaxKind::SCRIPT_HEADER)
            .is_some_and(|h| has_flag(&h, SyntaxKind::NATIVE_KW));
        if inherited
            && origin_native
            && !script_native
            && self.game() == Some(papyrus_dialect::GameId::Starfield)
        {
            out.push(report(
                format!(
                    "`{}` overrides a native function of {from}: Starfield refuses it in a script \
                     that is not `Native`",
                    item.name
                ),
                Vec::new(),
            ));
            return;
        }
        let Some(expected) = self.signature(&origin) else {
            return;
        };
        let origin_file = match origin {
            Def::Item { file, .. } => Some(file),
            _ => None,
        };
        let (label, _) = expected.label();
        let keyword = if origin_event { "Event" } else { "Function" };
        // Events: their types are `events/signature`'s.
        let same_types = expected.params.len() == item.params.len()
            && expected.params.iter().zip(&item.params).all(|(e, p)| {
                type_key(self, origin_file, &e.written) == type_key(self, Some(file), &p.ty)
            });
        if !same_types {
            if !event {
                out.push(report(
                    format!(
                        "`{}` does not take the parameters of the function it overrides in \
                         {from}: `{keyword} {label}`",
                        item.name
                    ),
                    Vec::new(),
                ));
            }
            return;
        }
        if !event {
            let returns = |ty: Option<&str>, file: Option<SourceFile>| {
                ty.map(|t| type_key(self, file, t)).unwrap_or_default()
            };
            if returns(expected.returns.as_deref(), origin_file)
                != returns(item.ty.as_deref(), Some(file))
            {
                let theirs = expected.returns.as_deref().unwrap_or("nothing");
                out.push(report(
                    format!(
                        "`{}` must return what the function it overrides in {from} returns: \
                         `{theirs}`",
                        item.name
                    ),
                    Vec::new(),
                ));
                return;
            }
        }
        let nodes = default_nodes(decl);
        for ((e, p), node) in expected.params.iter().zip(&item.params).zip(&nodes) {
            let same = match (&e.default, &p.default) {
                (None, None) => true,
                (Some(a), Some(b)) => same_default(a, b),
                _ => false,
            };
            if same {
                continue;
            }
            let theirs = e
                .default
                .as_deref()
                .map_or_else(|| "no default".to_owned(), |d| format!("`{d}`"));
            let mut message = format!(
                "the default of `{}` must be written as in {from}: {theirs}",
                p.name
            );
            if e.default.is_some() && p.default.is_some() {
                message.push_str(
                    " — the compiler compares `True`, `False`, `None` and strings as written",
                );
            }
            let fixes = match (&e.default, node) {
                (Some(theirs), Some(node)) => vec![Fix {
                    title: format!("Write `{theirs}`"),
                    edits: vec![Replacement {
                        range: trimmed(node),
                        text: theirs.clone(),
                    }],
                }],
                _ => Vec::new(),
            };
            out.push(report(message, fixes));
        }
    }

    /// From Fallout 4: a parameter without default after one with a
    /// default, a `Native` function in a script that is not.
    fn check_fallout4_function(
        &self,
        root: &SyntaxNode,
        decl: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let nodes = default_nodes(decl);
        if let Some(first) = nodes.iter().position(Option::is_some) {
            let params: Vec<SyntaxNode> = child(decl, SyntaxKind::PARAM_LIST)
                .map(|l| {
                    l.children()
                        .filter(|p| p.kind() == SyntaxKind::PARAM)
                        .collect()
                })
                .unwrap_or_default();
            for (param, default) in params.iter().zip(&nodes).skip(first + 1) {
                if default.is_some() {
                    continue;
                }
                let name = decl_name(param).map(|n| compact(&n)).unwrap_or_default();
                out.push(Diagnostic {
                    problem: Problem::DefaultOrder,
                    range: trimmed(param),
                    message: format!(
                        "`{name}` needs a default value: it follows a parameter that has one"
                    ),
                    fixes: Vec::new(),
                });
            }
        }
        let native_script = child(root, SyntaxKind::SCRIPT_HEADER)
            .is_some_and(|h| has_flag(&h, SyntaxKind::NATIVE_KW));
        if !native_script && has_flag(decl, SyntaxKind::NATIVE_KW) {
            let name = decl_name(decl).map(|n| compact(&n)).unwrap_or_default();
            let range = decl
                .children()
                .find(|c| c.kind() == SyntaxKind::FLAG_LIST)
                .and_then(|list| {
                    list.children().find(|f| {
                        f.first_token()
                            .is_some_and(|t| t.kind() == SyntaxKind::NATIVE_KW)
                    })
                })
                .map(|f| trimmed(&f))
                .unwrap_or_else(|| trimmed(decl));
            out.push(Diagnostic {
                problem: Problem::NativeFunction,
                range,
                message: format!(
                    "`{name}` is `Native`, but the script is not: from Fallout 4, only a `Native` \
                     script declares native functions"
                ),
                fixes: Vec::new(),
            });
        }
    }

    // ── States ────────────────────────────────────────────────────────────

    /// A function in a state that neither the empty state nor an ancestor
    /// declares; in Starfield, a `Global` function in a state.
    fn check_state_function(&self, file: SourceFile, decl: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(state) = state_of(decl) else {
            return;
        };
        if decl.kind() != SyntaxKind::FUNCTION_DECL {
            return;
        }
        let Some(name) = decl_name(decl) else {
            return;
        };
        let written = compact(&name);
        if has_flag(decl, SyntaxKind::GLOBAL_KW) {
            out.push(Diagnostic {
                problem: Problem::StateFunction,
                range: trimmed(&name),
                message: format!(
                    "`{written}` is `Global`: a `Global` function belongs to no state, declare it \
                     in the empty state only"
                ),
                fixes: Vec::new(),
            });
            return;
        }
        let me = ScriptRef::Source(file);
        let own = self
            .items(file)
            .named(&written)
            .any(|(_, i)| i.kind.is_callable() && i.container.is_none() && i.remote.is_none());
        if own || !self.chain_complete(me) {
            return;
        }
        let inherited = match self.parent(me) {
            Some(parent) => self.member(parent, &written, Namespace::Callable).is_some(),
            None => {
                self.skyrim()
                    && self
                        .own_member(
                            ScriptRef::Native(&builtins::SKYRIM_SCRIPT),
                            &written,
                            Namespace::Callable,
                        )
                        .is_some()
            }
        };
        if inherited {
            return;
        }
        out.push(Diagnostic {
            problem: Problem::StateFunction,
            range: trimmed(&name),
            message: format!(
                "`{written}` is only declared in state `{state}`: the compiler wants it in the \
                 empty state too"
            ),
            fixes: Vec::new(),
        });
    }

    // ── Properties ────────────────────────────────────────────────────────

    /// The functions of a full property, and initial values.
    fn check_property(&self, file: SourceFile, property: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(name) = decl_name(property) else {
            return;
        };
        let written = compact(&name);
        let ty = child(property, SyntaxKind::TYPE_REF).map(|t| compact(&t));
        let auto = has_flag(property, SyntaxKind::AUTO_KW);
        let readonly = has_flag(property, SyntaxKind::AUTOREADONLY_KW);
        let value = property
            .children_with_tokens()
            .any(|e| e.kind() == SyntaxKind::EQ);
        let push = |out: &mut Vec<Diagnostic>, range: TextRange, message: String| {
            out.push(Diagnostic {
                problem: Problem::PropertyAccessor,
                range,
                message,
                fixes: Vec::new(),
            });
        };
        if readonly && !value {
            push(
                out,
                trimmed(&name),
                format!("`{written}` is `AutoReadOnly`: it needs an initial value"),
            );
        }
        if !auto && !readonly && value {
            push(
                out,
                trimmed(&name),
                format!(
                    "`{written}` has `Get` or `Set` functions: only `Auto` and `AutoReadOnly` \
                     properties take an initial value"
                ),
            );
        }
        let key = |t: &str| type_key(self, Some(file), t);
        for function in property
            .children()
            .filter(|c| matches!(c.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL))
        {
            let Some(fname) = decl_name(&function) else {
                continue;
            };
            let fwritten = compact(&fname);
            let returns = (function.kind() == SyntaxKind::FUNCTION_DECL)
                .then(|| child(&function, SyntaxKind::TYPE_REF))
                .flatten()
                .map(|t| compact(&t));
            let params: Vec<String> = child(&function, SyntaxKind::PARAM_LIST)
                .map(|l| {
                    l.children()
                        .filter(|p| p.kind() == SyntaxKind::PARAM)
                        .map(|p| {
                            child(&p, SyntaxKind::TYPE_REF)
                                .map(|t| compact(&t))
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let ty = ty.as_deref().unwrap_or_default();
            let problem = if function.kind() == SyntaxKind::EVENT_DECL
                || !(fwritten.eq_ignore_ascii_case("Get") || fwritten.eq_ignore_ascii_case("Set"))
            {
                Some(format!(
                    "a property only holds a `Get` and a `Set` function, not `{fwritten}`"
                ))
            } else if fwritten.eq_ignore_ascii_case("Get") {
                (!params.is_empty() || returns.as_deref().map(key) != Some(key(ty))).then(|| {
                    format!("the `Get` of `{written}` takes nothing and returns `{ty}`: `{ty} Function Get()`")
                })
            } else {
                (returns.is_some() || params.len() != 1 || key(&params[0]) != key(ty)).then(|| {
                    format!("the `Set` of `{written}` takes a `{ty}` and returns nothing: `Function Set({ty} value)`")
                })
            };
            if let Some(message) = problem {
                push(out, trimmed(&fname), message);
            }
        }
    }
}
