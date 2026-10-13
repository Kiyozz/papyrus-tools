//! Names written with another case than their declaration: the compiler
//! ignores case, readers do not.

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange};

use super::{Diagnostic, Fix, Problem, Replacement};
use crate::SourceFile;
use crate::items::{compact, trimmed};
use crate::semantics::{Semantics, type_name_range};

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
        if let Some(def) = self.resolve_name_ref(file, name_ref) {
            push(
                out,
                trimmed(name_ref),
                &compact(name_ref),
                self.def_name(&def),
            );
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
            push(out, type_name_range(type_ref), written, self.def_name(&def));
        }
    }
}

fn push(out: &mut Vec<Diagnostic>, range: TextRange, written: &str, declared: &str) {
    // `Foo:Bar` may resolve to `Bar`: only a full name compares.
    if written == declared || !written.eq_ignore_ascii_case(declared) {
        return;
    }
    out.push(Diagnostic {
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
    });
}
