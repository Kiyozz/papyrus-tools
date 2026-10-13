//! What a script declares — its header and its items — lowered from the tree.
//!
//! Function bodies are left out: they only matter to the file being edited,
//! which is read from its tree directly.

use papyrus_syntax::{NodeOrToken, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// Script variable: `int count = 0`.
    Variable,
    /// `int Property Count Auto`, in a group or not.
    Property,
    /// `Function Foo()`, in a state or not.
    Function,
    /// `Event OnInit()`, in a state or not, remote or not.
    Event,
    /// `State Busy`.
    State,
    /// `Struct Entry`.
    Struct,
    /// A field of a struct.
    Field,
    /// `Group Settings`.
    Group,
    /// `CustomEvent OnDone`.
    CustomEvent,
    /// `Guard lock`.
    Guard,
}

impl ItemKind {
    /// The keyword introducing it, for messages.
    pub const fn describe(self) -> &'static str {
        match self {
            ItemKind::Variable => "variable",
            ItemKind::Property => "property",
            ItemKind::Function => "function",
            ItemKind::Event => "event",
            ItemKind::State => "state",
            ItemKind::Struct => "struct",
            ItemKind::Field => "field",
            ItemKind::Group => "group",
            ItemKind::CustomEvent => "custom event",
            ItemKind::Guard => "guard",
        }
    }

    /// Functions and events: what a call can reach.
    pub const fn is_callable(self) -> bool {
        matches!(self, ItemKind::Function | ItemKind::Event)
    }
}

/// A parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: String,
    /// The type, as written.
    pub ty: String,
    /// The default value, as written.
    pub default: Option<String>,
}

/// A declaration of the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What it is.
    pub kind: ItemKind,
    /// Its name, as written.
    pub name: String,
    /// Where its name is.
    pub name_range: TextRange,
    /// The whole declaration, trivia excluded.
    pub range: TextRange,
    /// The type of a variable, property or field, the return type of a
    /// function; as written.
    pub ty: Option<String>,
    /// The parameters of a function or an event.
    pub params: Vec<Param>,
    /// The state, struct or group holding it.
    pub container: Option<String>,
    /// `Event Actor.OnDeath`: the script listened to.
    pub remote: Option<String>,
    /// `Global`: called on the script, not on an instance.
    pub global: bool,
    /// `Native`: implemented by the engine.
    pub native: bool,
    /// A property that cannot be assigned: `AutoReadOnly`, `Const`, or a
    /// full property without `Set`.
    pub readonly: bool,
    /// The declaration line, whitespace collapsed: `int Function Foo(int a) Global`.
    pub signature: String,
    /// The docstring, braces and surrounding whitespace removed.
    pub doc: Option<String>,
}

/// What a script declares.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptItems {
    /// Its name, `MyMod:Base` for a namespaced script; empty without a header.
    pub name: String,
    /// Where its name is.
    pub name_range: TextRange,
    /// The header, trivia excluded.
    pub range: TextRange,
    /// The parent, as written after `extends`.
    pub parent: Option<String>,
    /// The scripts named by `import`, whose global functions are callable
    /// unqualified.
    pub imports: Vec<String>,
    /// The header line, whitespace collapsed.
    pub signature: String,
    /// The script's docstring.
    pub doc: Option<String>,
    /// Its declarations, in source order.
    pub items: Vec<Item>,
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// The significant text of a node: non-trivia tokens, joined.
pub(crate) fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE)
        .map(|t| t.text().to_owned())
        .collect()
}

/// The extent of a node without the surrounding trivia and line break.
pub(crate) fn trimmed(node: &SyntaxNode) -> TextRange {
    let significant = |t: &SyntaxToken| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE;
    let tokens = || {
        node.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(significant)
    };
    match (tokens().next(), tokens().last()) {
        (Some(first), Some(last)) => first.text_range().cover(last.text_range()),
        _ => node.text_range(),
    }
}

/// True for the nodes that declare something, and for bodies.
fn is_member(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::VARIABLE_DECL
            | SyntaxKind::PROPERTY_DECL
            | SyntaxKind::FUNCTION_DECL
            | SyntaxKind::EVENT_DECL
            | SyntaxKind::STATE_DECL
            | SyntaxKind::STRUCT_DECL
            | SyntaxKind::STRUCT_FIELD
            | SyntaxKind::GROUP_DECL
            | SyntaxKind::CUSTOM_EVENT_DECL
            | SyntaxKind::GUARD_DECL
            | SyntaxKind::STMT_LIST
    )
}

fn is_block_end(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ENDFUNCTION_KW
            | SyntaxKind::ENDEVENT_KW
            | SyntaxKind::ENDPROPERTY_KW
            | SyntaxKind::ENDSTATE_KW
            | SyntaxKind::ENDSTRUCT_KW
            | SyntaxKind::ENDGROUP_KW
    )
}

/// The declaration line of `node`: its own tokens, without nested
/// declarations, body, docstring or comments, whitespace collapsed.
pub(crate) fn signature(node: &SyntaxNode) -> String {
    fn walk(node: &SyntaxNode, out: &mut String, space: &mut bool) {
        for element in node.children_with_tokens() {
            match element {
                NodeOrToken::Node(n) if is_member(n.kind()) => {}
                NodeOrToken::Node(n) => walk(&n, out, space),
                NodeOrToken::Token(t) => {
                    let kind = t.kind();
                    if kind.is_trivia() || kind == SyntaxKind::NEWLINE {
                        *space = !out.is_empty();
                    } else if !is_block_end(kind) {
                        if *space {
                            out.push(' ');
                        }
                        *space = false;
                        out.push_str(t.text());
                    }
                }
            }
        }
    }
    let mut out = String::new();
    walk(node, &mut out, &mut false);
    out
}

/// The docstring of a declaration: inside the node, after the header line,
/// or right after the node for one-line declarations.
pub(crate) fn doc(node: &SyntaxNode) -> Option<String> {
    let inside = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == SyntaxKind::DOC_COMMENT);
    let after = || {
        let mut next = node.next_sibling_or_token();
        while let Some(element) = next {
            match element {
                NodeOrToken::Token(t) if t.kind() == SyntaxKind::DOC_COMMENT => return Some(t),
                NodeOrToken::Token(t)
                    if matches!(t.kind(), SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE) =>
                {
                    next = t.next_sibling_or_token();
                }
                _ => return None,
            }
        }
        None
    };
    let token = inside.or_else(after)?;
    let text = token.text();
    let text = text.strip_prefix('{').unwrap_or(text);
    let text = text.strip_suffix('}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let text = lines.join("\n").trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn has_flag(node: &SyntaxNode, flag: SyntaxKind) -> bool {
    child(node, SyntaxKind::FLAG_LIST).is_some_and(|list| {
        list.children()
            .filter_map(|f| f.first_token())
            .any(|t| t.kind() == flag)
    })
}

fn readonly(property: &SyntaxNode) -> bool {
    if has_flag(property, SyntaxKind::AUTOREADONLY_KW) || has_flag(property, SyntaxKind::CONST_KW) {
        return true;
    }
    !has_flag(property, SyntaxKind::AUTO_KW)
        && !property
            .children()
            .filter(|c| c.kind() == SyntaxKind::FUNCTION_DECL)
            .filter_map(|f| child(&f, SyntaxKind::NAME))
            .any(|n| compact(&n).eq_ignore_ascii_case("Set"))
}

fn params(node: &SyntaxNode) -> Vec<Param> {
    let Some(list) = child(node, SyntaxKind::PARAM_LIST) else {
        return Vec::new();
    };
    list.children()
        .filter(|p| p.kind() == SyntaxKind::PARAM)
        .filter_map(|p| {
            let name = compact(&child(&p, SyntaxKind::NAME)?);
            let ty = child(&p, SyntaxKind::TYPE_REF)
                .map(|t| compact(&t))
                .unwrap_or_default();
            // The default value follows `=`.
            let default = p
                .children_with_tokens()
                .skip_while(|e| e.kind() != SyntaxKind::EQ)
                .skip(1)
                .find_map(|e| e.into_node())
                .map(|n| compact(&n));
            Some(Param { name, ty, default })
        })
        .collect()
}

/// Lowers one declaration node; `None` for anything else.
fn item(node: &SyntaxNode, container: Option<&str>) -> Option<Item> {
    let kind = match node.kind() {
        SyntaxKind::VARIABLE_DECL => ItemKind::Variable,
        SyntaxKind::PROPERTY_DECL => ItemKind::Property,
        SyntaxKind::FUNCTION_DECL => ItemKind::Function,
        SyntaxKind::EVENT_DECL => ItemKind::Event,
        SyntaxKind::STATE_DECL => ItemKind::State,
        SyntaxKind::STRUCT_DECL => ItemKind::Struct,
        SyntaxKind::STRUCT_FIELD => ItemKind::Field,
        SyntaxKind::GROUP_DECL => ItemKind::Group,
        SyntaxKind::CUSTOM_EVENT_DECL => ItemKind::CustomEvent,
        SyntaxKind::GUARD_DECL => ItemKind::Guard,
        _ => return None,
    };
    let name_node = child(node, SyntaxKind::NAME)?;
    let name = compact(&name_node);
    if name.is_empty() {
        return None;
    }
    let type_ref = child(node, SyntaxKind::TYPE_REF).map(|t| compact(&t));
    let (ty, remote) = match kind {
        // `Event Actor.OnDeath`: the type is the script listened to.
        ItemKind::Event => (None, type_ref),
        _ => (type_ref, None),
    };
    Some(Item {
        kind,
        name,
        name_range: trimmed(&name_node),
        range: trimmed(node),
        ty,
        params: params(node),
        container: container.map(str::to_owned),
        remote,
        global: has_flag(node, SyntaxKind::GLOBAL_KW),
        native: has_flag(node, SyntaxKind::NATIVE_KW),
        readonly: kind == ItemKind::Property && readonly(node),
        signature: signature(node),
        doc: doc(node),
    })
}

impl ScriptItems {
    /// Lowers the tree of a whole file.
    pub fn lower(root: &SyntaxNode) -> ScriptItems {
        let mut out = ScriptItems::default();
        for node in root.children() {
            match node.kind() {
                SyntaxKind::SCRIPT_HEADER => {
                    if let Some(name) = child(&node, SyntaxKind::NAME) {
                        out.name = compact(&name);
                        out.name_range = trimmed(&name);
                    }
                    out.range = trimmed(&node);
                    out.parent = child(&node, SyntaxKind::EXTENDS_CLAUSE)
                        .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
                        .map(|t| compact(&t));
                    out.signature = signature(&node);
                    out.doc = doc(&node);
                }
                SyntaxKind::IMPORT => {
                    out.imports
                        .extend(child(&node, SyntaxKind::TYPE_REF).map(|t| compact(&t)));
                }
                _ => {
                    let Some(item) = item(&node, None) else {
                        continue;
                    };
                    let container = item.name.clone();
                    let nested = matches!(
                        item.kind,
                        ItemKind::State | ItemKind::Struct | ItemKind::Group
                    );
                    out.items.push(item);
                    if nested {
                        out.items.extend(
                            node.children()
                                .filter_map(|c| self::item(&c, Some(&container))),
                        );
                    }
                }
            }
        }
        out
    }

    /// The items named `name`, case-insensitively.
    pub fn named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = (usize, &'a Item)> + 'a {
        self.items
            .iter()
            .enumerate()
            .filter(move |(_, i)| i.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use papyrus_dialect::{Dialect, GameId};

    use super::*;

    fn lower(source: &str) -> ScriptItems {
        let parse = papyrus_syntax::parse(source, Dialect::new(GameId::Fallout4));
        ScriptItems::lower(&parse.syntax())
    }

    #[test]
    fn header_and_items_are_lowered() {
        let items = lower(
            "ScriptName MyMod:Foo extends Quest\n{ The script }\nimport Utility\n\
             int count = 0\n\
             Actor Property Who Auto\n{ Who to follow }\n\
             int Function Bar(int a, float b = 1.0) Global\n{ Adds }\n\treturn a\nEndFunction\n\
             State Busy\n\tEvent OnInit()\n\tEndEvent\nEndState\n\
             Struct Entry\n\tint Count\nEndStruct\n\
             Event Actor.OnDeath(Actor akKiller)\nEndEvent\n",
        );
        assert_eq!(items.name, "MyMod:Foo");
        assert_eq!(items.parent.as_deref(), Some("Quest"));
        assert_eq!(items.imports, ["Utility"]);
        assert_eq!(items.doc.as_deref(), Some("The script"));
        assert_eq!(items.signature, "ScriptName MyMod:Foo extends Quest");

        let summary: Vec<(ItemKind, &str, Option<&str>)> = items
            .items
            .iter()
            .map(|i| (i.kind, i.name.as_str(), i.container.as_deref()))
            .collect();
        assert_eq!(
            summary,
            [
                (ItemKind::Variable, "count", None),
                (ItemKind::Property, "Who", None),
                (ItemKind::Function, "Bar", None),
                (ItemKind::State, "Busy", None),
                (ItemKind::Event, "OnInit", Some("Busy")),
                (ItemKind::Struct, "Entry", None),
                (ItemKind::Field, "Count", Some("Entry")),
                (ItemKind::Event, "OnDeath", None),
            ]
        );

        let bar = &items.items[2];
        assert_eq!(
            bar.signature,
            "int Function Bar(int a, float b = 1.0) Global"
        );
        assert_eq!(bar.doc.as_deref(), Some("Adds"));
        assert_eq!(bar.ty.as_deref(), Some("int"));
        assert!(bar.global && !bar.native);
        assert_eq!(
            bar.params,
            [
                Param {
                    name: "a".into(),
                    ty: "int".into(),
                    default: None
                },
                Param {
                    name: "b".into(),
                    ty: "float".into(),
                    default: Some("1.0".into())
                },
            ]
        );
        assert_eq!(items.items[1].doc.as_deref(), Some("Who to follow"));
        assert_eq!(items.items[7].remote.as_deref(), Some("Actor"));
    }
}
