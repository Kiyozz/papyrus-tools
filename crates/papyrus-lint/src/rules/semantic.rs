//! `PAP005` to `PAP040`: calls, types, members, returns, states, events,
//! read-only properties, `Global` functions, dead code, SKSE's members,
//! unknown scripts, duplicates, the case of names, overrides, the
//! compiler's other refusals and modders' habits, from the project's
//! semantic model.

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
            Problem::ScriptName => &crate::SCRIPTS_NAME,
            Problem::ExtendsCycle => &crate::SCRIPTS_CYCLE,
            Problem::DuplicateMember => &crate::MEMBERS_DUPLICATE,
            Problem::ReturnValue => &crate::RETURNS_VALUE,
            Problem::OverrideSignature => &crate::OVERRIDES_SIGNATURE,
            Problem::StateFunction => &crate::STATES_FUNCTION,
            Problem::PropertyAccessor => &crate::PROPERTIES_ACCESSORS,
            Problem::ArraySize => &crate::ARRAYS_SIZE,
            Problem::NotAssignable => &crate::CODE_NOT_ASSIGNABLE,
            Problem::NotLiteral => &crate::VALUES_NOT_LITERAL,
            Problem::DefaultOrder => &crate::PARAMS_DEFAULT_ORDER,
            Problem::NativeFunction => &crate::FUNCTIONS_NATIVE,
            Problem::IntRange => &crate::LITERALS_INT_RANGE,
            Problem::RepeatingUpdate => &crate::UPDATES_REPEATING,
            Problem::GetPlayerInLoop => &crate::CALLS_GET_PLAYER,
            Problem::BoolComparison => &crate::BOOLS_COMPARISON,
            Problem::SelfAssign => &crate::CODE_SELF_ASSIGN,
            Problem::SelfCompare => &crate::CODE_SELF_COMPARE,
            Problem::UnusedVariable => &crate::VARIABLES_UNUSED,
            Problem::UnusedImport => &crate::IMPORTS_UNUSED,
            Problem::InfiniteRecursion => &crate::CALLS_INFINITE_RECURSION,
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
