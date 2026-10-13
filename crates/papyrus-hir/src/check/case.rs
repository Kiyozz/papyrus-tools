//! Names written with another case than their declaration: the compiler
//! ignores case, readers do not.
//!
//! The declaration is the reference, unless it breaks the naming convention
//! the use follows — `Initialize()` calling `Function initialize()` —: then
//! the declaration is the one to rename, if the project owns it.

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use super::{Diagnostic, Fix, Problem, Replacement};
use crate::SourceFile;
use crate::case::{is_lower_camel, is_pascal};
use crate::items::{ItemKind, compact, trimmed};
use crate::semantics::{Def, Semantics, name_node_at, type_name_range};

impl Semantics<'_> {
    /// A name of a value or a callee, against what it resolves to.
    pub(super) fn check_case(
        &self,
        file: SourceFile,
        name_ref: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        // The name of a named argument, `abFadeOut` in `abFadeOut = true`.
        let named_arg = name_ref.parent().is_some_and(|p| {
            p.kind() == SyntaxKind::ARG
                && p.children().next().as_ref() == Some(name_ref)
                && p.children_with_tokens().any(|e| e.kind() == SyntaxKind::EQ)
        });
        if named_arg {
            return;
        }
        let Some(def) = self.resolve_name_ref(file, name_ref) else {
            return;
        };
        let written = compact(name_ref);
        let declared = self.def_name(&def);
        if !differs(&written, declared) {
            return;
        }
        let range = trimmed(name_ref);
        if self.rename_to_use(&def, &written) {
            out.push(Diagnostic {
                problem: Problem::NameCase,
                range,
                message: format!(
                    "`{written}` is declared `{declared}`, against the naming convention: \
                     rename the declaration."
                ),
                fixes: Vec::new(),
            });
        } else {
            out.push(write_declared(range, &written, declared));
        }
    }

    /// A type: a script or a struct, `[]` aside.
    pub(super) fn check_type_case(
        &self,
        file: SourceFile,
        type_ref: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        let written = compact(type_ref);
        let written = written.trim_end_matches("[]");
        if let Some(def) = self.resolve_type(file, written) {
            let declared = self.def_name(&def);
            if differs(written, declared) {
                out.push(write_declared(type_name_range(type_ref), written, declared));
            }
        }
    }

    /// The name to give the declaration of the name at `offset`, when its
    /// use follows the naming convention its declaration breaks: `Initialize`
    /// for `Function initialize()` called `Initialize()`.
    pub fn case_rename(&self, file: SourceFile, offset: TextSize) -> Option<String> {
        let node =
            name_node_at(&self.root(file), offset).filter(|n| n.kind() == SyntaxKind::NAME_REF)?;
        let def = self.resolve_name_ref(file, &node)?;
        let written = compact(&node);
        (differs(&written, self.def_name(&def)) && self.rename_to_use(&def, &written))
            .then_some(written)
    }

    /// True if the declaration of `def` should take the case of `written`:
    /// the project's own function, script variable or local, its name
    /// breaking the convention `written` follows. Never a property, which
    /// plugins bind by name, nor an event, named by the engine.
    fn rename_to_use(&self, def: &Def, written: &str) -> bool {
        let pascal = match def {
            Def::Local { .. } => false,
            Def::Item { file, index } => {
                if self.index().is_imported(*file) {
                    return false;
                }
                match self.item(*file, *index).kind {
                    ItemKind::Function => true,
                    ItemKind::Variable => false,
                    _ => return false,
                }
            }
            _ => return false,
        };
        let follows = |name: &str| {
            if pascal {
                is_pascal(name)
            } else {
                is_lower_camel(name)
            }
        };
        follows(written)
            && !follows(self.def_name(def))
            && self.renamable(def).is_ok_and(|warning| warning.is_none())
    }
}

/// True if `written` names `declared` with another case. `Foo:Bar` may
/// resolve to `Bar`: only a full name compares.
fn differs(written: &str, declared: &str) -> bool {
    written != declared && written.eq_ignore_ascii_case(declared)
}

/// The use to write like its declaration.
fn write_declared(range: TextRange, written: &str, declared: &str) -> Diagnostic {
    Diagnostic {
        problem: Problem::NameCase,
        range,
        message: format!("`{written}` is declared `{declared}`."),
        fixes: vec![Fix {
            title: format!("Write `{declared}`"),
            edits: vec![Replacement {
                range,
                text: declared.to_owned(),
            }],
        }],
    }
}
