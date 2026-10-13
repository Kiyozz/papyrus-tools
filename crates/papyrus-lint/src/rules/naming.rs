//! `PAP101`–`PAP103`: naming conventions, Go style.
//!
//! - locals and parameters in lowerCamelCase, Hungarian prefixes included
//!   (`akTarget`, `aiCount` qualify);
//! - script variables, properties, fields and guards in lowerCamelCase;
//! - functions, events, states, structs, groups and custom events in
//!   PascalCase.
//!
//! # Fixes
//!
//! Papyrus is case-insensitive: renaming `HitCount` to `hitCount` changes
//! nothing for the compiler and cannot collide with anything, not even an
//! inherited property we cannot see. Only locals get a fix, and only when it
//! touches nothing but case. Properties get none: `.esp` files bind them by
//! name.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::case::{is_lower_camel, is_pascal, to_lower_camel, to_pascal};
use crate::{Context, Edit, Finding, Lint, NAMING_FUNCTIONS, NAMING_LOCALS, NAMING_VARIABLES};

/// What a name designates.
#[derive(Debug, Clone, Copy)]
enum Role {
    Local,
    Param,
    ScriptVariable,
    Property,
    Field,
    Guard,
    Function,
    Event,
    State,
    Struct,
    Group,
    CustomEvent,
}

impl Role {
    fn of(name: &SyntaxNode) -> Option<Role> {
        let owner = name.parent()?;
        Some(match owner.kind() {
            SyntaxKind::PARAM => Role::Param,
            SyntaxKind::VARIABLE_DECL => {
                if owner.parent()?.kind() == SyntaxKind::STMT_LIST {
                    Role::Local
                } else {
                    Role::ScriptVariable
                }
            }
            SyntaxKind::PROPERTY_DECL => Role::Property,
            SyntaxKind::STRUCT_FIELD => Role::Field,
            SyntaxKind::GUARD_DECL => Role::Guard,
            // `Get` and `Set` are imposed by the language.
            SyntaxKind::FUNCTION_DECL if owner.parent()?.kind() == SyntaxKind::PROPERTY_DECL => {
                return None;
            }
            SyntaxKind::FUNCTION_DECL => Role::Function,
            SyntaxKind::EVENT_DECL => Role::Event,
            SyntaxKind::STATE_DECL => Role::State,
            SyntaxKind::STRUCT_DECL => Role::Struct,
            SyntaxKind::GROUP_DECL => Role::Group,
            SyntaxKind::CUSTOM_EVENT_DECL => Role::CustomEvent,
            _ => return None,
        })
    }

    fn describe(self) -> &'static str {
        match self {
            Role::Local => "a local",
            Role::Param => "a parameter",
            Role::ScriptVariable => "a script variable",
            Role::Property => "a property",
            Role::Field => "a struct field",
            Role::Guard => "a guard",
            Role::Function => "a function",
            Role::Event => "an event",
            Role::State => "a state",
            Role::Struct => "a struct",
            Role::Group => "a group",
            Role::CustomEvent => "a custom event",
        }
    }

    fn lint(self) -> &'static Lint {
        match self {
            Role::Local | Role::Param => &NAMING_LOCALS,
            Role::ScriptVariable | Role::Property | Role::Field | Role::Guard => &NAMING_VARIABLES,
            _ => &NAMING_FUNCTIONS,
        }
    }

    fn pascal(self) -> bool {
        !matches!(
            self,
            Role::Local
                | Role::Param
                | Role::ScriptVariable
                | Role::Property
                | Role::Field
                | Role::Guard
        )
    }
}

pub(super) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    for name in context
        .root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NAME)
    {
        let (Some(role), Some(token)) = (Role::of(&name), name.first_token()) else {
            continue;
        };
        let text = token.text();
        let (ok, suggestion, style) = if role.pascal() {
            (is_pascal(text), to_pascal(text), "PascalCase")
        } else {
            (is_lower_camel(text), to_lower_camel(text), "lowerCamelCase")
        };
        if ok {
            continue;
        }

        let case_only = suggestion.eq_ignore_ascii_case(text);
        let fix = match role {
            Role::Local | Role::Param if case_only => Some(rename_in_function(&token, &suggestion)),
            _ => None,
        };
        // Plugins bind properties by name: changing even the case is a risk
        // the user takes explicitly.
        let unsafe_fix = (matches!(role, Role::Property) && case_only)
            .then(|| rename_in_script(context.root, &token, &suggestion));
        let note = match role {
            _ if !case_only => " The name changes beyond case: rename it by hand.",
            Role::Property => " Unsafe fix: plugins (.esp, .esm) bind properties by name.",
            _ => "",
        };
        findings.push(Finding {
            lint: role.lint(),
            range: token.text_range(),
            message: format!(
                "`{text}`: {} is written in {style}, `{suggestion}`.{note}",
                role.describe()
            ),
            fix,
            unsafe_fix,
            suggestion: None,
        });
    }
}

/// Renames the declaration and its uses in the enclosing function.
///
/// A use is a simple name in value position: not a member (`x.Foo`), not the
/// name of a named argument (`Foo(abX = 1)`), not a called function
/// (`Foo()`).
fn rename_in_function(decl: &SyntaxToken, new_name: &str) -> Vec<Edit> {
    let mut edits = vec![Edit {
        range: decl.text_range(),
        text: new_name.to_owned(),
    }];
    let Some(function) = decl
        .parent_ancestors()
        .find(|n| matches!(n.kind(), SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL))
    else {
        return edits;
    };

    for name_ref in function
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NAME_REF)
    {
        let mut tokens = name_ref
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia());
        let (Some(token), None) = (tokens.next(), tokens.next()) else {
            continue; // `A:B`, qualified name
        };
        if token.kind() != SyntaxKind::IDENT || !token.text().eq_ignore_ascii_case(decl.text()) {
            continue;
        }
        if is_value_position(&name_ref) {
            edits.push(Edit {
                range: token.text_range(),
                text: new_name.to_owned(),
            });
        }
    }
    edits
}

/// Renames the declaration and its uses in the whole script: simple names
/// in value position, and `self.Name`. Only the case changes, so a name
/// caught by mistake changes nothing for the compiler.
fn rename_in_script(root: &SyntaxNode, decl: &SyntaxToken, new_name: &str) -> Vec<Edit> {
    let mut edits = vec![Edit {
        range: decl.text_range(),
        text: new_name.to_owned(),
    }];
    for name_ref in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NAME_REF)
    {
        let mut tokens = name_ref
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !t.kind().is_trivia());
        let (Some(token), None) = (tokens.next(), tokens.next()) else {
            continue;
        };
        if token.kind() != SyntaxKind::IDENT || !token.text().eq_ignore_ascii_case(decl.text()) {
            continue;
        }
        let on_self = name_ref.parent().is_some_and(|p| {
            p.kind() == SyntaxKind::FIELD_EXPR
                && p.first_child()
                    .and_then(|r| r.first_token())
                    .is_some_and(|t| t.kind() == SyntaxKind::SELF_KW)
        });
        if on_self || is_value_position(&name_ref) {
            edits.push(Edit {
                range: token.text_range(),
                text: new_name.to_owned(),
            });
        }
    }
    edits
}

fn is_value_position(name_ref: &SyntaxNode) -> bool {
    let Some(parent) = name_ref.parent() else {
        return true;
    };
    match parent.kind() {
        // `x.Foo`: only `x` is a value.
        SyntaxKind::FIELD_EXPR => parent.first_child().as_ref() == Some(name_ref),
        // `Foo()`: `Foo` is a function.
        SyntaxKind::CALL_EXPR => parent.first_child().as_ref() != Some(name_ref),
        // `Foo(abX = x)`: `abX` names a parameter of the callee, `x` is a
        // value. The label is the one followed by `=`.
        SyntaxKind::ARG => {
            let next = std::iter::successors(name_ref.next_sibling_or_token(), |e| {
                e.next_sibling_or_token()
            })
            .find(|e| !e.kind().is_trivia());
            next.is_none_or(|e| e.kind() != SyntaxKind::EQ)
        }
        _ => true,
    }
}
