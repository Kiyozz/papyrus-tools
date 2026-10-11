//! `PAP101`–`PAP103` : les conventions de nommage, à la Go.
//!
//! - locales et paramètres en lowerCamelCase, préfixes hongrois compris
//!   (`akTarget`, `aiCount` en sont) ;
//! - variables de script, properties, champs et verrous en lowerCamelCase ;
//! - fonctions, événements, states, structures, groupes et événements
//!   personnalisés en PascalCase.
//!
//! # Correctifs
//!
//! Papyrus est insensible à la casse : renommer `HitCount` en `hitCount` ne
//! change rien pour le compilateur, et ne peut entrer en collision avec rien,
//! pas même une property héritée qu'on ne voit pas. Seules les locales
//! reçoivent un correctif, et seulement quand il ne touche qu'à la casse. Les
//! properties n'en ont pas : les `.esp` les lient par leur nom.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::case::{is_lower_camel, is_pascal, to_lower_camel, to_pascal};
use crate::{Context, Edit, Finding, Lint, NAMING_FUNCTIONS, NAMING_LOCALS, NAMING_VARIABLES};

/// Ce qu'un nom désigne.
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
            // `Get` et `Set` sont imposés par le langage.
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
            Role::Local => "une locale",
            Role::Param => "un paramètre",
            Role::ScriptVariable => "une variable de script",
            Role::Property => "une property",
            Role::Field => "un champ de structure",
            Role::Guard => "un verrou",
            Role::Function => "une fonction",
            Role::Event => "un événement",
            Role::State => "un state",
            Role::Struct => "une structure",
            Role::Group => "un groupe",
            Role::CustomEvent => "un événement personnalisé",
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
        let note = match role {
            Role::Property => {
                " Pas de correctif automatique : les .esp lient les properties par leur nom."
            }
            _ if !case_only => " Le nom change au-delà de la casse : à renommer à la main.",
            _ => "",
        };
        findings.push(Finding {
            lint: role.lint(),
            range: token.text_range(),
            message: format!(
                "`{text}` : {} s'écrit en {style}, `{suggestion}`.{note}",
                role.describe()
            ),
            fix,
            suggestion: None,
        });
    }
}

/// Renomme la déclaration et ses usages dans la fonction qui la contient.
///
/// Un usage, c'est un nom simple en position de valeur : ni un membre
/// (`x.Foo`), ni le nom d'un argument nommé (`Foo(abX = 1)`), ni une
/// fonction appelée (`Foo()`).
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
            continue; // `A:B`, nom qualifié
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

fn is_value_position(name_ref: &SyntaxNode) -> bool {
    let Some(parent) = name_ref.parent() else {
        return true;
    };
    match parent.kind() {
        // `x.Foo` : seul `x` est une valeur.
        SyntaxKind::FIELD_EXPR => parent.first_child().as_ref() == Some(name_ref),
        // `Foo()` : `Foo` est une fonction.
        SyntaxKind::CALL_EXPR => parent.first_child().as_ref() != Some(name_ref),
        // `Foo(abX = x)` : `abX` nomme un paramètre de l'appelée, `x` est
        // une valeur. L'étiquette est celle que suit le `=`.
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
