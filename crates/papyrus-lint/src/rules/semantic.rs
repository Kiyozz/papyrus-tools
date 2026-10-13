//! `PAP005` to `PAP020`: calls, types, members, returns, states, events,
//! read-only properties, `Global` functions, dead code, SKSE's members,
//! unknown scripts, duplicate locals and the case of names, from the
//! project's semantic model.

use papyrus_hir::Problem;

use crate::{
    CALLS_ARITY, CALLS_NAMED_ARG, CALLS_NOT_GLOBAL, CODE_UNREACHABLE, Context, EVENTS_SIGNATURE,
    Edit, Finding, LOCALS_DUPLICATE, LOCALS_UNUSED, MEMBERS_SKSE, MEMBERS_UNKNOWN, NAMES_CASE,
    PARAMS_UNUSED, PROPERTIES_READONLY, RETURNS_MISSING, SCRIPTS_UNKNOWN, STATES_UNKNOWN,
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
            Problem::UnknownState => &STATES_UNKNOWN,
            Problem::EventSignature => &EVENTS_SIGNATURE,
            Problem::ReadOnly => &PROPERTIES_READONLY,
            Problem::NotGlobal => &CALLS_NOT_GLOBAL,
            Problem::Unreachable => &CODE_UNREACHABLE,
            Problem::UnusedLocal => &LOCALS_UNUSED,
            Problem::UnusedParam => &PARAMS_UNUSED,
            Problem::SkseMember => &MEMBERS_SKSE,
            Problem::UnknownScript => &SCRIPTS_UNKNOWN,
            Problem::DuplicateLocal => &LOCALS_DUPLICATE,
            Problem::NameCase => &NAMES_CASE,
        };
        let mut suggestions: Vec<Suggestion> = diagnostic
            .fixes
            .into_iter()
            .map(|f| Suggestion {
                title: f.title,
                edits: f
                    .edits
                    .into_iter()
                    .map(|r| Edit {
                        range: r.range,
                        text: r.text,
                    })
                    .collect(),
            })
            .collect();
        // The case of a name changes nothing for the compiler: safe.
        let fix = (diagnostic.problem == Problem::NameCase && !suggestions.is_empty())
            .then(|| suggestions.remove(0).edits);
        findings.push(Finding {
            lint,
            range: diagnostic.range,
            message: diagnostic.message,
            fix,
            unsafe_fix: None,
            // Likely, not certain: suggested only.
            suggestions,
        });
    }
}
