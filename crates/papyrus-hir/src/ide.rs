//! What an editor asks of the model: where a name is defined, what it is,
//! what can be typed here, where it is used, how to rename it, which symbols
//! the workspace declares.
//!
//! Everything is expressed in files and byte ranges; turning them into
//! editor positions is the language server's job.

use papyrus_dialect::GameId;
use papyrus_stdlib::{Function, Script};
use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize};

use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, ScriptRef, Semantics, locals_in_scope, name_node_at, type_name_range};
use crate::ty::Ty;
use crate::{Project, SourceFile};

/// Where a definition lives in source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavTarget {
    /// The file declaring it.
    pub file: SourceFile,
    /// The whole declaration.
    pub full_range: TextRange,
    /// Its name.
    pub focus_range: TextRange,
}

/// What a name is, for a tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// The name hovered.
    pub range: TextRange,
    /// The declaration line: `Actor Function GetActorRef()`.
    pub signature: String,
    /// The docstring.
    pub doc: Option<String>,
    /// The script declaring it, or what it is when local: `parameter`.
    pub origin: Option<String>,
}

/// What a completion proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A local variable or a parameter.
    Local,
    /// A script variable.
    Variable,
    /// A property.
    Property,
    /// A function.
    Function,
    /// An event.
    Event,
    /// A struct field.
    Field,
    /// A struct.
    Struct,
    /// A script.
    Script,
    /// A custom event.
    CustomEvent,
    /// A built-in type: `int`, `string`.
    Type,
    /// A keyword: `self`, `None`.
    Keyword,
    /// A parameter of the call around the cursor, to pass by name:
    /// `value =`.
    NamedArg,
}

/// One proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// What is inserted.
    pub label: String,
    /// What it is.
    pub kind: CompletionKind,
    /// Its signature or type.
    pub detail: Option<String>,
}

/// A use or the declaration of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    /// The file.
    pub file: SourceFile,
    /// The name.
    pub range: TextRange,
    /// The declaration itself, not a use.
    pub declaration: bool,
}

/// The edits of a rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEdit {
    /// Every name to replace.
    pub edits: Vec<Reference>,
    /// The new name.
    pub new_name: String,
    /// A rename that compiles but may break something outside the scripts.
    pub warning: Option<String>,
    /// The script renamed: its file must take the new name too.
    pub script: Option<SourceFile>,
}

/// Why a name cannot be renamed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameError(pub String);

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RenameError {}

/// What a workspace symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// A script.
    Script,
    /// An item of a script.
    Item(ItemKind),
}

/// A symbol declared in a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// Its name.
    pub name: String,
    /// What it is.
    pub kind: SymbolKind,
    /// The script, state, struct or group holding it.
    pub container: Option<String>,
    /// Its file.
    pub file: SourceFile,
    /// Its name's range.
    pub range: TextRange,
}

/// The parameters of a native function, as written.
fn native_params(function: &Function) -> String {
    function
        .params
        .iter()
        .map(|p| match p.default {
            Some(default) => format!("{} {} = {default}", p.ty, p.name),
            None => format!("{} {}", p.ty, p.name),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The declaration line of a native function or event.
fn native_signature(function: &Function, event: bool) -> String {
    let params = native_params(function);
    if event {
        return format!("Event {}({params})", function.name);
    }
    let mut out = String::new();
    if let Some(returns) = function.returns {
        out.push_str(returns);
        out.push(' ');
    }
    out.push_str(&format!("Function {}({params})", function.name));
    if function.global {
        out.push_str(" Global");
    }
    if function.native {
        out.push_str(" Native");
    }
    out
}

fn native_script_signature(script: &Script) -> String {
    match script.parent {
        Some(parent) => format!("ScriptName {} extends {parent}", script.name),
        None => format!("ScriptName {}", script.name),
    }
}

/// The range of a name node, `[]` excluded.
fn name_range(node: &SyntaxNode) -> TextRange {
    if node.kind() == SyntaxKind::TYPE_REF {
        type_name_range(node)
    } else {
        trimmed(node)
    }
}

/// The name a node holds, `[]` excluded.
fn node_name(node: &SyntaxNode) -> String {
    let text = compact(node);
    match node.kind() {
        SyntaxKind::TYPE_REF => text.strip_suffix("[]").unwrap_or(&text).to_owned(),
        _ => text,
    }
}

/// True if `name` is an identifier the target does not reserve.
fn is_identifier(name: &str, game: Option<GameId>) -> bool {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    let game = game.unwrap_or(GameId::Starfield);
    valid && !papyrus_syntax::keywords::lookup(name).is_some_and(|(_, since)| game.accepts(since))
}

/// The previous significant token.
fn previous(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut current = token.prev_token();
    while let Some(t) = current {
        if !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE {
            return Some(t);
        }
        current = t.prev_token();
    }
    None
}

/// Accumulates completions, first one of a name wins.
#[derive(Default)]
struct Proposals {
    seen: std::collections::HashSet<String>,
    out: Vec<Completion>,
}

impl Proposals {
    fn push(&mut self, label: &str, kind: CompletionKind, detail: Option<String>) {
        if !label.is_empty() && self.seen.insert(label.to_ascii_lowercase()) {
            self.out.push(Completion {
                label: label.to_owned(),
                kind,
                detail,
            });
        }
    }
}

impl Semantics<'_> {
    // ── Definition ────────────────────────────────────────────────────────

    /// Where `def` is declared; `None` for native ones, which have no
    /// source.
    pub fn nav_target(&self, def: &Def) -> Option<NavTarget> {
        match def {
            Def::Local {
                file, name_range, ..
            } => {
                let root = self.root(*file);
                let decl = root
                    .covering_element(*name_range)
                    .ancestors()
                    .find(|n| matches!(n.kind(), SyntaxKind::PARAM | SyntaxKind::VARIABLE_DECL));
                Some(NavTarget {
                    file: *file,
                    full_range: decl.map_or(*name_range, |d| trimmed(&d)),
                    focus_range: *name_range,
                })
            }
            Def::Item { file, index } => {
                let item = self.item(*file, *index);
                Some(NavTarget {
                    file: *file,
                    full_range: item.range,
                    focus_range: item.name_range,
                })
            }
            Def::Script { file } => {
                let items = self.items(*file);
                Some(NavTarget {
                    file: *file,
                    full_range: items.range,
                    focus_range: items.name_range,
                })
            }
            Def::NativeScript(_) | Def::NativeFunction { .. } | Def::NativeProperty { .. } => None,
        }
    }

    /// Where the name at `offset` is declared.
    pub fn definition(&self, file: SourceFile, offset: TextSize) -> Option<NavTarget> {
        self.nav_target(&self.def_at(file, offset)?)
    }

    // ── Hover ─────────────────────────────────────────────────────────────

    /// What the name at `offset` is.
    pub fn hover(&self, file: SourceFile, offset: TextSize) -> Option<Hover> {
        let root = self.root(file);
        let node = name_node_at(&root, offset)?;
        let def = self.def_of_node(file, &node)?;
        let range = name_range(&node);
        Some(match &def {
            Def::Local {
                name, ty, param, ..
            } => Hover {
                range,
                signature: format!("{ty} {name}"),
                doc: None,
                origin: Some(
                    if *param {
                        "parameter"
                    } else {
                        "local variable"
                    }
                    .to_owned(),
                ),
            },
            Def::Item { file, index } => {
                let item = self.item(*file, *index);
                let script = &self.items(*file).name;
                Hover {
                    range,
                    signature: item.signature.clone(),
                    doc: item.doc.clone(),
                    origin: Some(match &item.container {
                        Some(container) => format!("{script}, {container}"),
                        None => script.clone(),
                    }),
                }
            }
            Def::Script { file } => {
                let items = self.items(*file);
                Hover {
                    range,
                    signature: items.signature.clone(),
                    doc: items.doc.clone(),
                    origin: self
                        .index()
                        .is_imported(*file)
                        .then(|| "imported".to_owned()),
                }
            }
            Def::NativeScript(script) => Hover {
                range,
                signature: native_script_signature(script),
                doc: None,
                origin: Some("native".to_owned()),
            },
            Def::NativeFunction {
                script,
                function,
                event,
            } => Hover {
                range,
                signature: native_signature(function, *event),
                doc: None,
                origin: Some(script.name.to_owned()),
            },
            Def::NativeProperty { script, property } => Hover {
                range,
                signature: format!("{} Property {}", property.ty, property.name),
                doc: None,
                origin: Some(script.name.to_owned()),
            },
        })
    }

    // ── Completion ────────────────────────────────────────────────────────

    /// What can be typed at `offset`.
    pub fn completions(&self, file: SourceFile, offset: TextSize) -> Vec<Completion> {
        let root = self.root(file);
        let Some(token) = root.token_at_offset(offset).left_biased() else {
            return Vec::new();
        };
        let typing = token.kind() == SyntaxKind::IDENT || token.kind().is_keyword();
        let before = if typing || token.kind().is_trivia() || token.kind() == SyntaxKind::NEWLINE {
            previous(&token)
        } else {
            Some(token.clone())
        };
        let mut proposals = Proposals::default();

        // `a.|`: the members of `a`.
        if let Some(dot) = before.clone().filter(|t| t.kind() == SyntaxKind::DOT) {
            if let Some(receiver) = dot
                .parent()
                .filter(|p| p.kind() == SyntaxKind::FIELD_EXPR)
                .and_then(|p| p.children().next())
            {
                self.complete_members(file, &receiver, &mut proposals);
            }
            return proposals.out;
        }

        let anchor = token.parent().unwrap_or_else(|| root.clone());
        let in_type = anchor.ancestors().any(|n| n.kind() == SyntaxKind::TYPE_REF)
            || previous_kind(&token, typing).is_some_and(|k| {
                matches!(
                    k,
                    SyntaxKind::EXTENDS_KW
                        | SyntaxKind::AS_KW
                        | SyntaxKind::IS_KW
                        | SyntaxKind::NEW_KW
                        | SyntaxKind::IMPORT_KW
                )
            });
        if in_type {
            self.complete_types(file, &mut proposals);
            return proposals.out;
        }

        // `Foo(|` or `Foo(1, |`: the parameters not given yet, by name.
        if before
            .as_ref()
            .is_some_and(|t| matches!(t.kind(), SyntaxKind::L_PAREN | SyntaxKind::COMMA))
        {
            self.complete_named_args(file, offset, &mut proposals);
        }

        for def in locals_in_scope(file, &anchor, offset, false) {
            if let Def::Local { name, ty, .. } = &def {
                proposals.push(name, CompletionKind::Local, Some(ty.clone()));
            }
        }
        let this = ScriptRef::Source(file);
        for (depth, script) in self.chain(this).enumerate() {
            // A script's variables are private to it.
            self.complete_script(script, depth == 0, false, &mut proposals);
        }
        if self.game().is_some_and(|g| g.rank() == 0) {
            self.complete_script(
                ScriptRef::Native(&crate::builtins::SKYRIM_SCRIPT),
                false,
                false,
                &mut proposals,
            );
        }
        for import in &self.items(file).imports {
            if let Some(script) = self.script_in(file, import) {
                self.complete_script(script, false, true, &mut proposals);
            }
        }
        for keyword in ["self", "parent", "None", "true", "false", "new"] {
            proposals.push(keyword, CompletionKind::Keyword, None);
        }
        self.complete_types(file, &mut proposals);
        proposals.out
    }

    /// The parameters of the call around `offset` that no argument gives
    /// yet: those after the positional arguments, not already named.
    fn complete_named_args(&self, file: SourceFile, offset: TextSize, proposals: &mut Proposals) {
        let Some((list, signature)) = self.call_at(file, offset) else {
            return;
        };
        let index = crate::hints::arg_index(&list, offset);
        let given: Vec<usize> = crate::calls::args(&list)
            .iter()
            .enumerate()
            .filter_map(|(i, arg)| match &arg.name {
                Some(name) => signature.param(&compact(name)),
                None => (i < index).then_some(i),
            })
            .collect();
        for (i, param) in signature.params.iter().enumerate() {
            if given.contains(&i) {
                continue;
            }
            let detail = match &param.default {
                Some(default) => format!("{} = {default}", param.written),
                None => param.written.clone(),
            };
            // Apart from the names: a local may share the parameter's.
            proposals.out.push(Completion {
                label: format!("{} =", param.name),
                kind: CompletionKind::NamedArg,
                detail: Some(detail),
            });
        }
    }

    /// The members of one script.
    fn complete_script(
        &self,
        script: ScriptRef,
        own: bool,
        global_only: bool,
        proposals: &mut Proposals,
    ) {
        match script {
            ScriptRef::Source(file) => {
                for item in &self.items(file).items {
                    if global_only && !item.global {
                        continue;
                    }
                    let kind = match item.kind {
                        ItemKind::Variable | ItemKind::Guard if own => CompletionKind::Variable,
                        ItemKind::Property => CompletionKind::Property,
                        ItemKind::Function => CompletionKind::Function,
                        ItemKind::Event if item.remote.is_none() => CompletionKind::Event,
                        ItemKind::CustomEvent => CompletionKind::CustomEvent,
                        _ => continue,
                    };
                    proposals.push(&item.name, kind, Some(item.signature.clone()));
                }
            }
            ScriptRef::Native(script) => {
                for function in script.functions {
                    if !global_only || function.global {
                        proposals.push(
                            function.name,
                            CompletionKind::Function,
                            Some(native_signature(function, false)),
                        );
                    }
                }
                if !global_only {
                    for property in script.properties {
                        proposals.push(
                            property.name,
                            CompletionKind::Property,
                            Some(format!("{} Property {}", property.ty, property.name)),
                        );
                    }
                    for event in script.events {
                        proposals.push(
                            event.name,
                            CompletionKind::Event,
                            Some(native_signature(event, true)),
                        );
                    }
                }
            }
        }
    }

    /// The members of what `receiver` designates.
    fn complete_members(&self, file: SourceFile, receiver: &SyntaxNode, proposals: &mut Proposals) {
        if receiver.kind() == SyntaxKind::NAME_REF
            && !matches!(
                receiver.first_token().map(|t| t.kind()),
                Some(SyntaxKind::SELF_KW | SyntaxKind::PARENT_KW)
            )
        {
            // `Debug.`: the script's global functions.
            let static_script = match self.resolve_name_ref(file, receiver) {
                Some(Def::Script { file }) => Some(ScriptRef::Source(file)),
                Some(Def::NativeScript(script)) => Some(ScriptRef::Native(script)),
                _ => None,
            };
            if let Some(script) = static_script {
                for script in self.chain(script) {
                    self.complete_script(script, false, true, proposals);
                }
                return;
            }
        }
        let ty = self.type_of(file, receiver);
        match &ty {
            Ty::Array(_) => {
                proposals.push("Length", CompletionKind::Property, Some("int".to_owned()));
                for function in self.array_functions() {
                    proposals.push(
                        function.name,
                        CompletionKind::Function,
                        Some(native_signature(function, false)),
                    );
                }
            }
            Ty::Object(name) => {
                if let Some(script) = self.script(name) {
                    let own = script == ScriptRef::Source(file);
                    for (depth, script) in self.chain(script).enumerate() {
                        self.complete_script(script, own && depth == 0, false, proposals);
                    }
                } else if let Some(Def::Item { file, index }) = self.resolve_type(file, name) {
                    let items = self.items(file);
                    let container = &items.items[index].name;
                    for item in &items.items {
                        if item.kind == ItemKind::Field
                            && item.container.as_ref() == Some(container)
                        {
                            proposals.push(&item.name, CompletionKind::Field, item.ty.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The types: built-in, scripts, the structs of the script's chain.
    fn complete_types(&self, file: SourceFile, proposals: &mut Proposals) {
        for builtin in ["bool", "int", "float", "string"] {
            proposals.push(builtin, CompletionKind::Type, None);
        }
        if self.game().is_none_or(|g| g.rank() >= 1) {
            proposals.push("var", CompletionKind::Type, None);
        }
        for script in self.chain(ScriptRef::Source(file)) {
            if let ScriptRef::Source(source) = script {
                for item in &self.items(source).items {
                    if item.kind == ItemKind::Struct {
                        proposals.push(&item.name, CompletionKind::Struct, None);
                    }
                }
            }
        }
        let mut scripts: Vec<SourceFile> = self.index().files().collect();
        scripts.sort_by_key(|f| self.items(*f).name.to_ascii_lowercase());
        for script in scripts {
            let items = self.items(script);
            proposals.push(
                &items.name,
                CompletionKind::Script,
                items.parent.as_ref().map(|p| format!("extends {p}")),
            );
        }
        for script in papyrus_stdlib::scripts(self.library()) {
            proposals.push(
                script.name,
                CompletionKind::Script,
                script.parent.map(|p| format!("extends {p}")),
            );
        }
    }

    // ── References ────────────────────────────────────────────────────────

    /// The uses of the symbol at `offset`, its declarations included or
    /// not. An override and the function it overrides are one symbol.
    pub fn references(
        &self,
        file: SourceFile,
        offset: TextSize,
        include_declaration: bool,
    ) -> Option<Vec<Reference>> {
        let def = self.def_at(file, offset)?;
        let mut found = self.references_of(&def);
        if !include_declaration {
            found.retain(|r| !r.declaration);
        }
        Some(found)
    }

    /// Every use and declaration of `def`.
    pub fn references_of(&self, def: &Def) -> Vec<Reference> {
        let files: Vec<SourceFile> = match def {
            Def::Local { file, .. } => vec![*file],
            _ => {
                let project = self.project();
                let db = self.db();
                project
                    .files(db)
                    .iter()
                    .chain(project.imports(db))
                    .copied()
                    .collect()
            }
        };
        self.references_in(def, &files)
    }

    /// The uses and declarations of `def` in `files`.
    pub fn references_in(&self, def: &Def, files: &[SourceFile]) -> Vec<Reference> {
        let target = self.canonical(def);
        let name = self.def_name(def).to_owned();
        let mut out = Vec::new();
        for &file in files {
            if !crate::mentions::mentions(self.db(), file, self.syntax()).may_mention(&name) {
                continue;
            }
            for node in self.root(file).descendants() {
                if !matches!(
                    node.kind(),
                    SyntaxKind::NAME | SyntaxKind::NAME_REF | SyntaxKind::TYPE_REF
                ) || !node_name(&node).eq_ignore_ascii_case(&name)
                {
                    continue;
                }
                let Some(found) = self.def_of_node(file, &node) else {
                    continue;
                };
                if self.canonical(&found) == target {
                    out.push(Reference {
                        file,
                        range: name_range(&node),
                        declaration: node.kind() == SyntaxKind::NAME,
                    });
                }
            }
        }
        out
    }

    // ── Rename ────────────────────────────────────────────────────────────

    /// The name at `offset`, if it can be renamed.
    pub fn prepare_rename(
        &self,
        file: SourceFile,
        offset: TextSize,
    ) -> Result<(TextRange, Def), RenameError> {
        let root = self.root(file);
        let node = name_node_at(&root, offset)
            .ok_or_else(|| RenameError("no name to rename here".to_owned()))?;
        let def = self
            .def_of_node(file, &node)
            .ok_or_else(|| RenameError(format!("`{}` is not resolved", node_name(&node))))?;
        self.renamable(&def)?;
        Ok((name_range(&node), def))
    }

    /// Checks that `def` can be renamed; a warning when it compiles but may
    /// break what binds it by name.
    pub(crate) fn renamable(&self, def: &Def) -> Result<Option<String>, RenameError> {
        let name = self.def_name(def).to_owned();
        match self.canonical(def) {
            Def::NativeScript(_) | Def::NativeFunction { .. } | Def::NativeProperty { .. } => Err(
                RenameError(format!("`{name}` comes from the game's native scripts")),
            ),
            Def::Script { file } if self.index().is_imported(file) => Err(RenameError(format!(
                "`{name}` is an imported script, which papyrus never changes"
            ))),
            Def::Script { .. } if name.contains(':') => Err(RenameError(format!(
                "`{name}` has a namespace: rename its file by hand"
            ))),
            Def::Script { .. } => Ok(Some(format!(
                "`{name}` is a script: plugins (.esp, .esm) attach scripts by name, attach it \
                 again in the Creation Kit"
            ))),
            Def::Item { file, index } => {
                if self.index().is_imported(file) {
                    return Err(RenameError(format!(
                        "`{name}` is declared in an imported script, which papyrus never changes"
                    )));
                }
                let kind = self.item(file, index).kind;
                if kind == ItemKind::Event {
                    return Err(RenameError(format!(
                        "`{name}` is an event: the engine calls events by name"
                    )));
                }
                Ok((kind == ItemKind::Property).then(|| {
                    format!(
                        "`{name}` is a property: plugins (.esp, .esm) bind properties by name, \
                         fill it again in the Creation Kit"
                    )
                }))
            }
            Def::Local { .. } => Ok(None),
        }
    }

    /// Renames the symbol at `offset` to `new_name`.
    pub fn rename(
        &self,
        file: SourceFile,
        offset: TextSize,
        new_name: &str,
    ) -> Result<RenameEdit, RenameError> {
        if !is_identifier(new_name, self.game()) {
            return Err(RenameError(format!("`{new_name}` is not a valid name")));
        }
        let (_, def) = self.prepare_rename(file, offset)?;
        let warning = self.renamable(&def)?;
        let index = self.index();
        let script = match self.canonical(&def) {
            Def::Script { file } => Some(file),
            _ => None,
        };
        if let Some(script) = script {
            let taken = index.files().any(|other| {
                other != script && self.items(other).name.eq_ignore_ascii_case(new_name)
            });
            if taken {
                return Err(RenameError(format!("a script `{new_name}` already exists")));
            }
        }
        let edits = self
            .references_of(&def)
            .into_iter()
            .filter(|r| !index.is_imported(r.file))
            .collect();
        Ok(RenameEdit {
            edits,
            new_name: new_name.to_owned(),
            warning,
            script,
        })
    }
}

/// The kind of the token before the one being typed.
fn previous_kind(token: &SyntaxToken, typing: bool) -> Option<SyntaxKind> {
    let before = if typing || token.kind().is_trivia() || token.kind() == SyntaxKind::NEWLINE {
        previous(token)
    } else {
        Some(token.clone())
    };
    before.map(|t| t.kind())
}

/// True if `name` matches `query`: its characters appear in order,
/// case-insensitively.
fn fuzzy(name: &str, query: &str) -> bool {
    let mut name = name.chars().map(|c| c.to_ascii_lowercase());
    query
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .all(|q| name.any(|c| c == q))
}

/// The most symbols a query returns.
const MAX_SYMBOLS: usize = 512;

/// The scripts and declarations of `projects` matching `query`, imports
/// excluded.
pub fn workspace_symbols(
    db: &dyn salsa::Database,
    projects: &[Project],
    query: &str,
) -> Vec<Symbol> {
    let mut out = Vec::new();
    for &project in projects {
        let sema = Semantics::new(db, project);
        for &file in project.files(db) {
            let items = sema.items(file);
            if items.name.is_empty() {
                continue;
            }
            if fuzzy(&items.name, query) {
                out.push(Symbol {
                    name: items.name.clone(),
                    kind: SymbolKind::Script,
                    container: None,
                    file,
                    range: items.name_range,
                });
            }
            for item in &items.items {
                if fuzzy(&item.name, query) {
                    out.push(Symbol {
                        name: item.name.clone(),
                        kind: SymbolKind::Item(item.kind),
                        container: Some(match &item.container {
                            Some(container) => format!("{}.{container}", items.name),
                            None => items.name.clone(),
                        }),
                        file,
                        range: item.name_range,
                    });
                }
            }
            if out.len() >= MAX_SYMBOLS {
                out.truncate(MAX_SYMBOLS);
                return out;
            }
        }
    }
    out
}
