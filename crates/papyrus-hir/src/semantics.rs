//! Name resolution and the types of expressions, from declarations alone.

use papyrus_dialect::GameId;
use papyrus_stdlib::{Function, Property, Script};
use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize};

use crate::builtins;
use crate::items::{Item, ItemKind, ScriptItems, compact, trimmed};
use crate::ty::Ty;
use crate::{Project, ScriptIndex, SourceFile, script_index, script_items};

/// What a name refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Def {
    /// A local variable or a parameter.
    Local {
        /// The file declaring it.
        file: SourceFile,
        /// Its name, as declared.
        name: String,
        /// Where its name is declared.
        name_range: TextRange,
        /// Its type, as written.
        ty: String,
        /// A parameter rather than a variable.
        param: bool,
    },
    /// An item of a project or imported script.
    Item {
        /// The script declaring it.
        file: SourceFile,
        /// Its position in [`ScriptItems::items`].
        index: usize,
    },
    /// A project or imported script.
    Script {
        /// Its file.
        file: SourceFile,
    },
    /// A native script, known by its embedded signatures.
    NativeScript(&'static Script),
    /// A function or an event of a native script.
    NativeFunction {
        /// The script declaring it.
        script: &'static Script,
        /// The function.
        function: &'static Function,
        /// An event rather than a function.
        event: bool,
    },
    /// A property of a native script.
    NativeProperty {
        /// The script declaring it.
        script: &'static Script,
        /// The property.
        property: &'static Property,
    },
}

/// A script, from source or native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptRef {
    /// A project or imported script.
    Source(SourceFile),
    /// A native script.
    Native(&'static Script),
}

/// What a name may designate, depending on where it is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Namespace {
    /// The callee of a call: functions and events.
    Callable,
    /// Anything else: variables, properties, fields.
    Value,
}

impl Namespace {
    fn accepts(self, kind: ItemKind) -> bool {
        match self {
            Namespace::Callable => kind.is_callable(),
            Namespace::Value => matches!(
                kind,
                ItemKind::Variable | ItemKind::Property | ItemKind::Field | ItemKind::Guard
            ),
        }
    }
}

/// The longest `extends` chain followed; deeper means a cycle.
pub(crate) const MAX_DEPTH: usize = 64;

/// Resolves names and types within one project.
#[derive(Clone, Copy)]
pub struct Semantics<'db> {
    db: &'db dyn salsa::Database,
    project: Project,
    /// The game whose native scripts apply.
    game: Option<GameId>,
    /// The game whose syntax the files are read with: the project's.
    syntax: Option<GameId>,
}

impl std::fmt::Debug for Semantics<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Semantics")
            .field("project", &self.project)
            .field("game", &self.game)
            .finish_non_exhaustive()
    }
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// The type name of a `TYPE_REF`, without its `[]`.
fn type_name(type_ref: &SyntaxNode) -> String {
    let text = compact(type_ref);
    text.strip_suffix("[]").unwrap_or(&text).to_owned()
}

/// The extent of a type name, without its `[]`.
pub(crate) fn type_name_range(type_ref: &SyntaxNode) -> TextRange {
    let tokens: Vec<SyntaxToken> = type_ref
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            matches!(t.kind(), SyntaxKind::IDENT | SyntaxKind::COLON) || t.kind().is_keyword()
        })
        .collect();
    match (tokens.first(), tokens.last()) {
        (Some(first), Some(last)) => first.text_range().cover(last.text_range()),
        _ => trimmed(type_ref),
    }
}

impl<'db> Semantics<'db> {
    /// The model of `project`.
    pub fn new(db: &'db dyn salsa::Database, project: Project) -> Semantics<'db> {
        Semantics {
            db,
            project,
            game: project.game(db),
            syntax: project.game(db),
        }
    }

    /// The same model, as if the project targeted `game`: its native
    /// scripts apply, the files are still read as the project says.
    pub(crate) fn assuming(&self, game: GameId) -> Semantics<'db> {
        Semantics {
            game: Some(game),
            ..*self
        }
    }

    /// The game the files are read with.
    pub(crate) fn syntax(&self) -> Option<GameId> {
        self.syntax
    }

    /// The database.
    pub fn db(&self) -> &'db dyn salsa::Database {
        self.db
    }

    /// The project.
    pub fn project(&self) -> Project {
        self.project
    }

    /// The project's target.
    pub fn game(&self) -> Option<GameId> {
        self.game
    }

    /// The index of the project's scripts.
    pub fn index(&self) -> &'db ScriptIndex {
        script_index(self.db, self.project)
    }

    /// The tree of `file`.
    pub fn root(&self, file: SourceFile) -> SyntaxNode {
        crate::parse(self.db, file, self.syntax).syntax()
    }

    /// What `file` declares.
    pub fn items(&self, file: SourceFile) -> &'db ScriptItems {
        script_items(self.db, file, self.syntax)
    }

    /// The item a [`Def::Item`] designates.
    pub fn item(&self, file: SourceFile, index: usize) -> &'db Item {
        &self.items(file).items[index]
    }

    // ── Scripts ───────────────────────────────────────────────────────────

    /// The script `name`: the project's, an imported one, else a native one.
    pub fn script(&self, name: &str) -> Option<ScriptRef> {
        if let Some(file) = self.index().get(name) {
            return Some(ScriptRef::Source(file));
        }
        let game = self.game.unwrap_or(GameId::Starfield);
        papyrus_stdlib::script(game, name).map(ScriptRef::Native)
    }

    /// The script `name` as written in `file`. A short name also resolves
    /// in the namespace of `file`'s script, first: `Bar` written in
    /// `Hardcore:Foo` may be `Hardcore:Bar`.
    pub fn script_in(&self, file: SourceFile, name: &str) -> Option<ScriptRef> {
        let mut namespace = self.items(file).name.as_str();
        while let Some((outer, _)) = namespace.rsplit_once(':') {
            if let Some(found) = self.script(&format!("{outer}:{name}")) {
                return Some(found);
            }
            namespace = outer;
        }
        self.script(name)
    }

    /// The name of a script.
    pub fn script_name(&self, script: ScriptRef) -> &'db str {
        match script {
            ScriptRef::Source(file) => &self.items(file).name,
            ScriptRef::Native(script) => script.name,
        }
    }

    /// The parent of a script. From Fallout 4 on, a script without
    /// `extends` derives from `ScriptObject`.
    pub fn parent(&self, script: ScriptRef) -> Option<ScriptRef> {
        let written = match script {
            ScriptRef::Source(file) => self.items(file).parent.as_deref(),
            ScriptRef::Native(script) => script.parent,
        };
        if let (ScriptRef::Source(file), Some(written)) = (script, written) {
            return self.script_in(file, written);
        }
        let implicit = || {
            let name = self.script_name(script);
            (self.game.is_some_and(|g| g.rank() >= 1) && !name.eq_ignore_ascii_case("ScriptObject"))
                .then_some("ScriptObject")
        };
        self.script(written.or_else(implicit)?)
    }

    /// `script` and its ancestors, from closest to farthest.
    pub fn chain(&self, script: ScriptRef) -> impl Iterator<Item = ScriptRef> + 'db {
        let this = *self;
        let mut next = Some(script);
        let mut depth = 0;
        std::iter::from_fn(move || {
            let current = next?;
            depth += 1;
            next = if depth < MAX_DEPTH {
                this.parent(current)
            } else {
                None
            };
            Some(current)
        })
    }

    /// The definition of a script.
    pub fn script_def(&self, script: ScriptRef) -> Def {
        match script {
            ScriptRef::Source(file) => Def::Script { file },
            ScriptRef::Native(script) => Def::NativeScript(script),
        }
    }

    /// The member `name` declared by `script` itself, not inherited.
    fn own_member(&self, script: ScriptRef, name: &str, ns: Namespace) -> Option<Def> {
        match script {
            ScriptRef::Source(file) => {
                let mut found: Option<(usize, &Item)> = None;
                for (index, item) in self.items(file).named(name) {
                    if !ns.accepts(item.kind) {
                        continue;
                    }
                    // The empty state's declaration first, then any state's.
                    let better = match found {
                        None => true,
                        Some((_, best)) => best.container.is_some() && item.container.is_none(),
                    };
                    // Struct fields are only reached through a struct value.
                    if better && item.kind != ItemKind::Field {
                        found = Some((index, item));
                    }
                }
                found.map(|(index, _)| Def::Item { file, index })
            }
            ScriptRef::Native(script) => {
                if ns != Namespace::Callable {
                    return script
                        .properties
                        .iter()
                        .find(|p| p.name.eq_ignore_ascii_case(name))
                        .map(|property| Def::NativeProperty { script, property });
                }
                let find = |list: &'static [Function]| {
                    list.iter().find(|f| f.name.eq_ignore_ascii_case(name))
                };
                if let Some(function) = find(script.functions) {
                    return Some(Def::NativeFunction {
                        script,
                        function,
                        event: false,
                    });
                }
                find(script.events).map(|function| Def::NativeFunction {
                    script,
                    function,
                    event: true,
                })
            }
        }
    }

    /// True for Skyrim, whose scripts have no common ancestor.
    pub(crate) fn skyrim(&self) -> bool {
        self.game.is_some_and(|g| g.rank() == 0)
    }

    /// The member `name` of `script`, inherited or not.
    fn member(&self, script: ScriptRef, name: &str, ns: Namespace) -> Option<Def> {
        self.chain(script)
            .find_map(|s| self.own_member(s, name, ns))
            .or_else(|| {
                // Skyrim's state functions, which no native script declares.
                self.skyrim()
                    .then(|| self.own_member(ScriptRef::Native(&builtins::SKYRIM_SCRIPT), name, ns))
                    .flatten()
            })
    }

    /// The array functions the target knows.
    pub(crate) fn array_functions(&self) -> &'static [Function] {
        let all = builtins::ARRAY.functions;
        if self.skyrim() {
            &all[..builtins::SKYRIM_ARRAY_FUNCTIONS]
        } else {
            all
        }
    }

    /// Gives a struct type the script declaring it, `Script:Struct`, so that
    /// it still resolves from another file.
    pub(crate) fn qualify(&self, file: SourceFile, ty: Ty) -> Ty {
        match ty {
            Ty::Array(element) => Ty::Array(Box::new(self.qualify(file, *element))),
            Ty::Object(name) => match self.script_in(file, &name) {
                Some(script) => Ty::Object(self.script_name(script).to_owned()),
                None => match self.find_struct(file, &name) {
                    Some(Def::Item { file, index }) => Ty::Object(format!(
                        "{}:{}",
                        self.items(file).name,
                        self.item(file, index).name
                    )),
                    _ => Ty::Object(name),
                },
            },
            other => other,
        }
    }

    /// The struct `name` seen from `file`: `Script:Struct`, or a struct of
    /// the script, of its ancestors, or of the scripts it imports.
    fn find_struct(&self, file: SourceFile, name: &str) -> Option<Def> {
        let in_script = |script: ScriptRef, name: &str| match script {
            ScriptRef::Source(file) => self
                .items(file)
                .named(name)
                .find(|(_, i)| i.kind == ItemKind::Struct)
                .map(|(index, _)| Def::Item { file, index }),
            ScriptRef::Native(_) => None,
        };
        if let Some((script, name)) = name.rsplit_once(':') {
            if let Some(script) = self.script_in(file, script) {
                if let Some(def) = in_script(script, name) {
                    return Some(def);
                }
            }
        }
        self.chain(ScriptRef::Source(file))
            .find_map(|script| in_script(script, name))
            .or_else(|| {
                // `import` brings the structs of a script into scope too.
                self.items(file)
                    .imports
                    .iter()
                    .filter_map(|import| self.script_in(file, import))
                    .find_map(|script| in_script(script, name))
            })
    }

    /// The type named `name` seen from `file`: a script, else a struct.
    pub fn resolve_type(&self, file: SourceFile, name: &str) -> Option<Def> {
        match self.script_in(file, name) {
            Some(script) => Some(self.script_def(script)),
            None => self.find_struct(file, name),
        }
    }

    // ── Definitions ───────────────────────────────────────────────────────

    /// What the name at `offset` in `file` refers to.
    pub fn def_at(&self, file: SourceFile, offset: TextSize) -> Option<Def> {
        let root = self.root(file);
        let node = name_node_at(&root, offset)?;
        self.def_of_node(file, &node)
    }

    /// What a `NAME`, `NAME_REF` or `TYPE_REF` node refers to.
    pub fn def_of_node(&self, file: SourceFile, node: &SyntaxNode) -> Option<Def> {
        match node.kind() {
            SyntaxKind::NAME => self.declared(file, node),
            SyntaxKind::NAME_REF => self.resolve_name_ref(file, node),
            SyntaxKind::TYPE_REF => self.resolve_type(file, &type_name(node)),
            _ => None,
        }
    }

    /// What a `NAME` declares.
    fn declared(&self, file: SourceFile, name: &SyntaxNode) -> Option<Def> {
        let decl = name.parent()?;
        match decl.kind() {
            SyntaxKind::SCRIPT_HEADER => Some(Def::Script { file }),
            SyntaxKind::PARAM => local(file, &decl, true),
            SyntaxKind::VARIABLE_DECL
                if decl
                    .parent()
                    .is_some_and(|p| p.kind() == SyntaxKind::STMT_LIST) =>
            {
                local(file, &decl, false)
            }
            _ => {
                let range = trimmed(name);
                self.items(file)
                    .items
                    .iter()
                    .position(|i| i.name_range == range)
                    .map(|index| Def::Item { file, index })
            }
        }
    }

    /// What a `NAME_REF` refers to.
    pub fn resolve_name_ref(&self, file: SourceFile, name_ref: &SyntaxNode) -> Option<Def> {
        let this = ScriptRef::Source(file);
        match name_ref.first_token()?.kind() {
            SyntaxKind::SELF_KW => return Some(Def::Script { file }),
            SyntaxKind::PARENT_KW => return self.parent(this).map(|p| self.script_def(p)),
            SyntaxKind::LENGTH_KW => return None,
            _ => {}
        }
        let name = compact(name_ref);
        let parent = name_ref.parent()?;

        // `a.b`: `b` is a member of what `a` is.
        let receiver = (parent.kind() == SyntaxKind::FIELD_EXPR)
            .then(|| parent.children().next())
            .flatten()
            .filter(|first| first != name_ref);
        let callee = if receiver.is_some() {
            parent.clone()
        } else {
            name_ref.clone()
        };
        let ns = if is_callee(&callee) {
            Namespace::Callable
        } else {
            Namespace::Value
        };

        if let Some(receiver) = receiver {
            return self.member_of_expr(file, &receiver, &name, ns);
        }
        // `Foo(aiCount = 2)`: a parameter of the callee.
        if parent.kind() == SyntaxKind::ARG
            && name_ref
                .siblings_with_tokens(rowan::Direction::Next)
                .any(|e| e.kind() == SyntaxKind::EQ)
        {
            return None;
        }

        if ns == Namespace::Value {
            if let Some(def) = local_in_scope(file, name_ref, &name) {
                return Some(def);
            }
        }
        if let Some(def) = self.member(this, &name, ns) {
            return Some(def);
        }
        if ns == Namespace::Callable {
            // The global functions of imported scripts.
            for import in &self.items(file).imports {
                if let Some(def) = self
                    .script(import)
                    .and_then(|s| self.own_member(s, &name, ns))
                    .filter(|d| self.is_global(d))
                {
                    return Some(def);
                }
            }
            return None;
        }
        self.resolve_type(file, &name)
    }

    /// True for a `Global` function.
    fn is_global(&self, def: &Def) -> bool {
        match def {
            Def::Item { file, index } => self.item(*file, *index).global,
            Def::NativeFunction { function, .. } => function.global,
            _ => false,
        }
    }

    /// The member `name` of what `receiver` designates: a script, for a
    /// static call, else the type of its value.
    fn member_of_expr(
        &self,
        file: SourceFile,
        receiver: &SyntaxNode,
        name: &str,
        ns: Namespace,
    ) -> Option<Def> {
        if receiver.kind() == SyntaxKind::NAME_REF {
            match self.resolve_name_ref(file, receiver) {
                Some(Def::Script { file: target }) => {
                    // `self.x` reads an instance; `Script.Foo()` a global.
                    return self.member(ScriptRef::Source(target), name, ns);
                }
                Some(Def::NativeScript(script)) => {
                    return self.member(ScriptRef::Native(script), name, ns);
                }
                _ => {}
            }
        }
        self.member_of_ty(file, &self.type_of(file, receiver), name, ns)
    }

    /// The member `name` of a value of type `ty`.
    fn member_of_ty(&self, file: SourceFile, ty: &Ty, name: &str, ns: Namespace) -> Option<Def> {
        if let Ty::Array(_) = ty {
            return (ns == Namespace::Callable)
                .then(|| {
                    self.array_functions()
                        .iter()
                        .find(|f| f.name.eq_ignore_ascii_case(name))
                })
                .flatten()
                .map(|function| Def::NativeFunction {
                    script: &builtins::ARRAY,
                    function,
                    event: false,
                });
        }
        let object = ty.object()?;
        if let Some(script) = self.script(object) {
            return self.member(script, name, ns);
        }
        // A struct value: its fields.
        let Def::Item { file, index } = self.find_struct(file, object)? else {
            return None;
        };
        let items = self.items(file);
        let container = &items.items[index].name;
        items
            .named(name)
            .find(|(_, i)| i.kind == ItemKind::Field && i.container.as_ref() == Some(container))
            .map(|(index, _)| Def::Item { file, index })
    }

    // ── Types ─────────────────────────────────────────────────────────────

    /// The declared type of what `def` designates, used as a value.
    pub fn type_of_def(&self, def: &Def) -> Ty {
        match def {
            Def::Local { file, ty, .. } => self.qualify(*file, Ty::parse(ty)),
            Def::Item { file, index } => {
                let item = self.item(*file, *index);
                match item.kind {
                    ItemKind::Variable
                    | ItemKind::Property
                    | ItemKind::Field
                    | ItemKind::Function => item
                        .ty
                        .as_deref()
                        .map_or(Ty::Unknown, |ty| self.qualify(*file, Ty::parse(ty))),
                    _ => Ty::Unknown,
                }
            }
            Def::NativeFunction { function, .. } => function.returns.map_or(Ty::Unknown, Ty::parse),
            Def::NativeProperty { property, .. } => Ty::parse(property.ty),
            Def::Script { .. } | Def::NativeScript(_) => Ty::Unknown,
        }
    }

    /// The type of an expression, from declarations alone.
    pub fn type_of(&self, file: SourceFile, expr: &SyntaxNode) -> Ty {
        let first = || expr.children().next();
        match expr.kind() {
            SyntaxKind::NAME_REF => match expr.first_token().map(|t| t.kind()) {
                Some(SyntaxKind::SELF_KW) => Ty::Object(self.items(file).name.clone()),
                Some(SyntaxKind::PARENT_KW) => self
                    .parent(ScriptRef::Source(file))
                    .map_or(Ty::Unknown, |p| Ty::Object(self.script_name(p).to_owned())),
                Some(SyntaxKind::LENGTH_KW) => Ty::Int,
                _ => self
                    .resolve_name_ref(file, expr)
                    .map_or(Ty::Unknown, |d| self.type_of_def(&d)),
            },
            SyntaxKind::CALL_EXPR => {
                let callee = match first() {
                    Some(c) if c.kind() == SyntaxKind::FIELD_EXPR => member_name_ref(&c),
                    other => other,
                };
                callee
                    .filter(|c| c.kind() == SyntaxKind::NAME_REF)
                    .and_then(|c| self.resolve_name_ref(file, &c))
                    .map_or(Ty::Unknown, |d| self.type_of_def(&d))
            }
            SyntaxKind::FIELD_EXPR => {
                member_name_ref(expr).map_or(Ty::Unknown, |member| self.type_of(file, &member))
            }
            SyntaxKind::CAST_EXPR => {
                if expr
                    .children_with_tokens()
                    .any(|e| e.kind() == SyntaxKind::IS_KW)
                {
                    Ty::Bool
                } else {
                    child(expr, SyntaxKind::TYPE_REF)
                        .map_or(Ty::Unknown, |t| self.qualify(file, Ty::parse(&compact(&t))))
                }
            }
            SyntaxKind::NEW_EXPR => {
                let Some(type_ref) = child(expr, SyntaxKind::TYPE_REF) else {
                    return Ty::Unknown;
                };
                let ty = self.qualify(file, Ty::parse(&compact(&type_ref)));
                if expr
                    .children_with_tokens()
                    .any(|e| e.kind() == SyntaxKind::L_BRACK)
                {
                    Ty::Array(Box::new(ty))
                } else {
                    ty
                }
            }
            SyntaxKind::INDEX_EXPR => match first().map(|a| self.type_of(file, &a)) {
                Some(Ty::Array(element)) => *element,
                _ => Ty::Unknown,
            },
            SyntaxKind::LITERAL => match expr.first_token().map(|t| t.kind()) {
                Some(SyntaxKind::INT_NUMBER | SyntaxKind::HEX_NUMBER) => Ty::Int,
                Some(SyntaxKind::FLOAT_NUMBER) => Ty::Float,
                Some(SyntaxKind::STRING) => Ty::String,
                Some(SyntaxKind::TRUE_KW | SyntaxKind::FALSE_KW) => Ty::Bool,
                Some(SyntaxKind::NONE_KW) => Ty::None,
                _ => Ty::Unknown,
            },
            SyntaxKind::PAREN_EXPR => first().map_or(Ty::Unknown, |e| self.type_of(file, &e)),
            SyntaxKind::UNARY_EXPR => {
                if expr
                    .children_with_tokens()
                    .any(|e| e.kind() == SyntaxKind::BANG)
                {
                    Ty::Bool
                } else {
                    match first().map(|e| self.type_of(file, &e)) {
                        Some(ty @ (Ty::Int | Ty::Float)) => ty,
                        _ => Ty::Unknown,
                    }
                }
            }
            SyntaxKind::BIN_EXPR => {
                let operator = expr
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE)
                    .map(|t| t.kind());
                match operator {
                    Some(
                        SyntaxKind::EQ2
                        | SyntaxKind::NEQ
                        | SyntaxKind::LT
                        | SyntaxKind::GT
                        | SyntaxKind::LTEQ
                        | SyntaxKind::GTEQ
                        | SyntaxKind::AMP2
                        | SyntaxKind::PIPE2,
                    ) => Ty::Bool,
                    Some(operator) => {
                        let operands: Vec<Ty> =
                            expr.children().map(|e| self.type_of(file, &e)).collect();
                        // `+` with a string on either side concatenates.
                        if operator == SyntaxKind::PLUS && operands.contains(&Ty::String) {
                            Ty::String
                        } else if operands.len() != 2
                            || !operands.iter().all(|t| matches!(t, Ty::Int | Ty::Float))
                        {
                            Ty::Unknown
                        } else if operands.contains(&Ty::Float) {
                            Ty::Float
                        } else {
                            Ty::Int
                        }
                    }
                    None => Ty::Unknown,
                }
            }
            _ => Ty::Unknown,
        }
    }

    // ── Identity ──────────────────────────────────────────────────────────

    /// The declaration `def` overrides, up to the topmost one: an override
    /// and the function it overrides are one symbol.
    pub fn canonical(&self, def: &Def) -> Def {
        let Def::Item { file, index } = def else {
            return def.clone();
        };
        let item = self.item(*file, *index);
        let ns = match item.kind {
            ItemKind::Function | ItemKind::Event => Namespace::Callable,
            ItemKind::Property => Namespace::Value,
            _ => return def.clone(),
        };
        let this = ScriptRef::Source(*file);
        // A state's function overrides the empty state's.
        let mut best = match item.container {
            Some(_) => self
                .own_member(this, &item.name, ns)
                .unwrap_or_else(|| def.clone()),
            None => def.clone(),
        };
        for script in self.chain(this).skip(1) {
            if let Some(found) = self.own_member(script, &item.name, ns) {
                best = found;
            }
        }
        best
    }

    /// The name a definition declares.
    pub fn def_name<'a>(&'a self, def: &'a Def) -> &'a str {
        match def {
            Def::Local { name, .. } => name,
            Def::Item { file, index } => &self.item(*file, *index).name,
            Def::Script { file } => &self.items(*file).name,
            Def::NativeScript(script) => script.name,
            Def::NativeFunction { function, .. } => function.name,
            Def::NativeProperty { property, .. } => property.name,
        }
    }
}

/// The node naming something at `offset`: a `NAME`, a `NAME_REF` or a
/// `TYPE_REF`. The token right before the cursor counts too, so that a name
/// just typed is found.
pub(crate) fn name_node_at(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxNode> {
    let named = |token: SyntaxToken| {
        token.parent_ancestors().find(|n| {
            matches!(
                n.kind(),
                SyntaxKind::NAME | SyntaxKind::NAME_REF | SyntaxKind::TYPE_REF
            )
        })
    };
    let at = root.token_at_offset(offset);
    let mut candidates: Vec<SyntaxToken> = at.collect();
    // Prefer an identifier over punctuation or whitespace.
    candidates.sort_by_key(|t| !(t.kind() == SyntaxKind::IDENT || t.kind().is_keyword()));
    candidates.into_iter().find_map(named)
}

/// True if `node` is called: the first child of a `CALL_EXPR`.
pub(crate) fn is_callee(node: &SyntaxNode) -> bool {
    node.parent().is_some_and(|p| {
        p.kind() == SyntaxKind::CALL_EXPR && p.children().next().as_ref() == Some(node)
    })
}

/// The member of `a.b`: `b`.
pub(crate) fn member_name_ref(field: &SyntaxNode) -> Option<SyntaxNode> {
    field
        .children()
        .skip(1)
        .find(|c| c.kind() == SyntaxKind::NAME_REF)
}

/// A local variable or a parameter declared by `decl`.
fn local(file: SourceFile, decl: &SyntaxNode, param: bool) -> Option<Def> {
    let name = child(decl, SyntaxKind::NAME)?;
    Some(Def::Local {
        file,
        name: compact(&name),
        name_range: trimmed(&name),
        ty: child(decl, SyntaxKind::TYPE_REF)
            .map(|t| compact(&t))
            .unwrap_or_default(),
        param,
    })
}

/// The local variables and parameters visible at `offset`, inside `at`,
/// closest first. With `later`, the variables of the enclosing blocks
/// declared after `offset` follow: the compiler accepts a use before the
/// declaration, in a loop for instance.
pub(crate) fn locals_in_scope(
    file: SourceFile,
    at: &SyntaxNode,
    offset: TextSize,
    later: bool,
) -> Vec<Def> {
    let mut out = Vec::new();
    let mut after = Vec::new();
    for ancestor in at.ancestors() {
        match ancestor.kind() {
            SyntaxKind::STMT_LIST => {
                let (before, rest): (Vec<SyntaxNode>, Vec<SyntaxNode>) = ancestor
                    .children()
                    .filter(|c| c.kind() == SyntaxKind::VARIABLE_DECL)
                    .partition(|c| c.text_range().end() <= offset);
                out.extend(before.iter().rev().filter_map(|c| local(file, c, false)));
                if later {
                    after.extend(
                        rest.iter()
                            .filter(|c| c.text_range().start() > offset)
                            .filter_map(|c| local(file, c, false)),
                    );
                }
            }
            SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
                if let Some(list) = child(&ancestor, SyntaxKind::PARAM_LIST) {
                    out.extend(
                        list.children()
                            .filter(|p| p.kind() == SyntaxKind::PARAM)
                            .filter_map(|p| local(file, &p, true)),
                    );
                }
                break;
            }
            _ => {}
        }
    }
    out.extend(after);
    out
}

fn local_in_scope(file: SourceFile, at: &SyntaxNode, name: &str) -> Option<Def> {
    locals_in_scope(file, at, at.text_range().start(), true)
        .into_iter()
        .find(|d| match d {
            Def::Local { name: n, .. } => n.eq_ignore_ascii_case(name),
            _ => false,
        })
}
