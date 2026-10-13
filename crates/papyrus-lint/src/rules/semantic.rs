//! `PAP005` to `PAP016`: calls, types, members, returns, states, events,
//! read-only properties, `Global` functions and dead code, from the
//! project's semantic model.

use papyrus_hir::Problem;

use crate::{
    CALLS_ARITY, CALLS_NAMED_ARG, CALLS_NOT_GLOBAL, CODE_UNREACHABLE, Context, EVENTS_SIGNATURE,
    Edit, Finding, LOCALS_UNUSED, MEMBERS_UNKNOWN, PARAMS_UNUSED, PROPERTIES_READONLY,
    RETURNS_MISSING, STATES_UNKNOWN, Suggestion, TYPES_MISMATCH,
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
            Problem::UnknownState => &STATES_UNKNOWN,
            Problem::EventSignature => &EVENTS_SIGNATURE,
            Problem::ReadOnly => &PROPERTIES_READONLY,
            Problem::NotGlobal => &CALLS_NOT_GLOBAL,
            Problem::Unreachable => &CODE_UNREACHABLE,
            Problem::UnusedLocal => &LOCALS_UNUSED,
            Problem::UnusedParam => &PARAMS_UNUSED,
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
