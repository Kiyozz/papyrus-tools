//! The compiler's implicit casts, and what can be known of them.
//!
//! A value goes where another type is expected when the compiler casts it
//! implicitly: `int` to `float`, anything to `string` or `bool`, an object
//! to one of its ancestors, `None` to any object or array, anything to
//! `var`. Every other cast must be written with `as`.
//!
//! An object type is only judged when its whole `extends` chain is known:
//! an unknown ancestor could be the expected type. Whatever cannot be
//! decided is `None`, never an error.

use crate::SourceFile;
use crate::items::ItemKind;
use crate::semantics::{MAX_DEPTH, ScriptRef, Semantics};
use crate::ty::Ty;

/// What an object type designates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Object {
    /// A script.
    Script(ScriptRef),
    /// A struct: the script declaring it, its index there.
    Struct(SourceFile, usize),
}

impl Semantics<'_> {
    /// True if the whole `extends` chain of `script` is known, up to a
    /// script without parent.
    pub fn chain_complete(&self, script: ScriptRef) -> bool {
        let mut depth = 0;
        let mut last = script;
        for current in self.chain(script) {
            depth += 1;
            last = current;
        }
        let written = match last {
            ScriptRef::Source(file) => self.items(file).parent.is_some(),
            ScriptRef::Native(script) => script.parent.is_some(),
        };
        depth < MAX_DEPTH && !written
    }

    /// The script or struct an object type names: a script, else
    /// `Script:Struct`, as [`Semantics::type_of`] qualifies them.
    pub(crate) fn object(&self, name: &str) -> Option<Object> {
        if let Some(script) = self.script(name) {
            return Some(Object::Script(script));
        }
        let (script, name) = name.rsplit_once(':')?;
        let ScriptRef::Source(file) = self.script(script)? else {
            return None;
        };
        self.items(file)
            .named(name)
            .find(|(_, i)| i.kind == ItemKind::Struct)
            .map(|(index, _)| Object::Struct(file, index))
    }

    /// True if `object` is known to be a script, of a known lineage.
    fn known_object(&self, name: &str) -> bool {
        match self.object(name) {
            Some(Object::Script(script)) => self.chain_complete(script),
            Some(Object::Struct(..)) => true,
            None => false,
        }
    }

    /// Whether a value of type `from` may be used where `to` is expected
    /// without a cast. `None` when it cannot be known.
    pub fn coerces(&self, from: &Ty, to: &Ty) -> Option<bool> {
        match (from, to) {
            (Ty::Unknown, _) | (_, Ty::Unknown) | (Ty::Var, _) => None,
            (_, Ty::Var | Ty::Bool | Ty::String) => Some(true),
            (Ty::Int, Ty::Int | Ty::Float) | (Ty::Float, Ty::Float) => Some(true),
            (Ty::None, Ty::Object(name)) => self.object(name).map(|_| true),
            (Ty::None, Ty::Array(_)) => Some(true),
            (Ty::Object(from), Ty::Object(to)) => self.extends(from, to),
            (Ty::Array(from), Ty::Array(to)) => self.same_element(from, to),
            // An object becomes a number or an array only through `as`, if
            // at all.
            (Ty::Object(name), _) => self.known_object(name).then_some(false),
            (_, Ty::Object(name)) => self.object(name).map(|_| false),
            _ => Some(false),
        }
    }

    /// Whether an object of type `from` is a `to`.
    fn extends(&self, from: &str, to: &str) -> Option<bool> {
        let (from, to) = (self.object(from)?, self.object(to)?);
        match (from, to) {
            (Object::Script(from), Object::Script(to)) => {
                if self.chain(from).any(|s| s == to) {
                    Some(true)
                } else {
                    self.chain_complete(from).then_some(false)
                }
            }
            (Object::Struct(..), Object::Struct(..)) => Some(from == to),
            (Object::Script(from), Object::Struct(..)) => {
                self.chain_complete(from).then_some(false)
            }
            (Object::Struct(..), Object::Script(_)) => Some(false),
        }
    }

    /// Whether arrays of `from` go where arrays of `to` are expected: the
    /// same type of element, known for sure.
    fn same_element(&self, from: &Ty, to: &Ty) -> Option<bool> {
        let base = |ty: &Ty| matches!(ty, Ty::Bool | Ty::Int | Ty::Float | Ty::String);
        match (from, to) {
            (Ty::Var, _) | (_, Ty::Var) => None,
            _ if base(from) && base(to) => Some(from == to),
            (Ty::Object(a), Ty::Object(b)) => {
                let (a, b) = (self.object(a)?, self.object(b)?);
                // A child's array may or may not pass for its parent's: only
                // the same type is sure.
                (a == b).then_some(true)
            }
            _ => None,
        }
    }
}
