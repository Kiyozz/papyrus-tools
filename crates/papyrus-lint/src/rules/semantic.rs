//! `PAP005` to `PAP009`: calls, types, members and returns, from the
//! project's semantic model.

use papyrus_hir::Problem;

use crate::{
    CALLS_ARITY, CALLS_NAMED_ARG, Context, Edit, Finding, MEMBERS_UNKNOWN, RETURNS_MISSING,
    Suggestion, TYPES_MISMATCH,
};

pub(crate) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    let Some(model) = context.model else {
        return;
    };
    for diagnostic in model.semantics.check(model.file) {
        let lint = match diagnostic.problem {
            Problem::Arity => &CALLS_ARITY,
            Problem::NamedArg => &CALLS_NAMED_ARG,
            Problem::Mismatch => &TYPES_MISMATCH,
            Problem::UnknownMember => &MEMBERS_UNKNOWN,
            Problem::MissingReturn => &RETURNS_MISSING,
        };
        findings.push(Finding {
            lint,
            range: diagnostic.range,
            message: diagnostic.message,
            fix: None,
            unsafe_fix: None,
            // The closest name is likely, not certain: suggested only.
            suggestion: diagnostic.replacement.map(|r| Suggestion {
                title: format!("Replace with `{}`", r.text),
                edits: vec![Edit {
                    range: r.range,
                    text: r.text,
                }],
            }),
        });
    }
}
