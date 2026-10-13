//! Calls: what a call reaches, and the arguments it passes.

use papyrus_syntax::{SyntaxKind, SyntaxNode};

use crate::items::ItemKind;
use crate::semantics::{Def, Semantics, member_name_ref};
use crate::ty::Ty;

/// A parameter of a function or an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParam {
    /// Its name.
    pub name: String,
    /// Its type, as written.
    pub written: String,
    /// Its type, resolved from the declaring script.
    pub ty: Ty,
    /// Its default value, as written.
    pub default: Option<String>,
}

/// What a call reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The function or event name.
    pub name: String,
    /// The return type, as written.
    pub returns: Option<String>,
    /// The parameters.
    pub params: Vec<SignatureParam>,
    /// An event rather than a function.
    pub event: bool,
    /// The docstring.
    pub doc: Option<String>,
}

impl Signature {
    /// The signature as displayed, `int Foo(int a, float b = 1.0)`, and the
    /// byte range of each parameter in it.
    pub fn label(&self) -> (String, Vec<(usize, usize)>) {
        let mut out = String::new();
        if let Some(returns) = &self.returns {
            out.push_str(returns);
            out.push(' ');
        }
        out.push_str(&self.name);
        out.push('(');
        let mut ranges = Vec::new();
        for (i, param) in self.params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let start = out.len();
            out.push_str(&param.written);
            out.push(' ');
            out.push_str(&param.name);
            if let Some(default) = &param.default {
                out.push_str(" = ");
                out.push_str(default);
            }
            ranges.push((start, out.len()));
        }
        out.push(')');
        (out, ranges)
    }

    /// The parameter `name`, case-insensitively.
    pub fn param(&self, name: &str) -> Option<usize> {
        self.params
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// An argument of a call.
#[derive(Debug, Clone)]
pub(crate) struct Arg {
    /// The `ARG` node.
    pub(crate) node: SyntaxNode,
    /// The name of a named argument: `abFadeOut` in `abFadeOut = true`.
    pub(crate) name: Option<SyntaxNode>,
    /// The value passed.
    pub(crate) value: Option<SyntaxNode>,
}

/// The arguments of an `ARG_LIST`.
pub(crate) fn args(list: &SyntaxNode) -> Vec<Arg> {
    list.children()
        .filter(|a| a.kind() == SyntaxKind::ARG)
        .map(|node| {
            let named = node
                .children_with_tokens()
                .any(|e| e.kind() == SyntaxKind::EQ);
            let mut nodes = node.children();
            let name = if named {
                nodes.next().filter(|n| n.kind() == SyntaxKind::NAME_REF)
            } else {
                None
            };
            let value = nodes.next();
            Arg { node, name, value }
        })
        .collect()
}

/// The name called: `Foo` in `Foo()` and in `a.Foo()`.
pub(crate) fn callee(call: &SyntaxNode) -> Option<SyntaxNode> {
    let first = call.children().next()?;
    match first.kind() {
        SyntaxKind::NAME_REF => Some(first),
        SyntaxKind::FIELD_EXPR => member_name_ref(&first),
        _ => None,
    }
}

/// The argument list of a call.
pub(crate) fn arg_list(call: &SyntaxNode) -> Option<SyntaxNode> {
    call.children().find(|c| c.kind() == SyntaxKind::ARG_LIST)
}

impl Semantics<'_> {
    /// The signature of a function or an event; `None` for anything else.
    pub fn signature(&self, def: &Def) -> Option<Signature> {
        match def {
            Def::Item { file, index } => {
                let item = self.item(*file, *index);
                // `Event Actor.OnDeath` is called by the engine only.
                if !item.kind.is_callable() || item.remote.is_some() {
                    return None;
                }
                Some(Signature {
                    name: item.name.clone(),
                    returns: (item.kind == ItemKind::Function)
                        .then(|| item.ty.clone())
                        .flatten(),
                    params: item
                        .params
                        .iter()
                        .map(|p| SignatureParam {
                            name: p.name.clone(),
                            written: p.ty.clone(),
                            ty: self.qualify(*file, Ty::parse(&p.ty)),
                            default: p.default.clone(),
                        })
                        .collect(),
                    event: item.kind == ItemKind::Event,
                    doc: item.doc.clone(),
                })
            }
            Def::NativeFunction {
                function, event, ..
            } => Some(Signature {
                name: function.name.to_owned(),
                returns: function.returns.map(str::to_owned),
                params: function
                    .params
                    .iter()
                    .map(|p| SignatureParam {
                        name: p.name.to_owned(),
                        written: p.ty.to_owned(),
                        ty: Ty::parse(p.ty),
                        default: p.default.map(str::to_owned),
                    })
                    .collect(),
                event: *event,
                doc: None,
            }),
            _ => None,
        }
    }
}
