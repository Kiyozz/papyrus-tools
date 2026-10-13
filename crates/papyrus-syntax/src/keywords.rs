//! The keyword table — single source of truth for the per-game matrix.
//!
//! Adding a keyword means adding a line here. Nothing else in the lexer or
//! the grammar knows the list.
//!
//! Each entry carries the game from which the word is reserved. The lexer
//! always produces [`SyntaxKind::IDENT`] for an identifier; it is
//! [`crate::token_source`] that promotes it to a keyword, and only if the
//! target is recent enough. Without this filter, the thousands of Skyrim
//! scripts declaring a variable named `Var`, `Struct` or `Guard` would
//! produce syntax errors.

use std::cmp::Ordering;

use papyrus_dialect::GameId;

use crate::SyntaxKind;

/// Reserved words, sorted by name.
///
/// The order enables binary search; a test checks it.
///
/// Sources: the per-game ANTLR grammars of `fireundubh/OpenPapyrus`,
/// cross-checked with Caprica's lexer and, for Starfield, with the vanilla
/// scripts. Disputed cases were settled by compiling a test case with the
/// game's `PapyrusCompiler.exe`.
static KEYWORDS: &[(&str, SyntaxKind, GameId)] = &[
    // `as` exists since Skyrim, contrary to Caprica's classification:
    // `akRef as Actor` compiles with the Skyrim SE compiler.
    ("as", SyntaxKind::AS_KW, GameId::Skyrim),
    ("auto", SyntaxKind::AUTO_KW, GameId::Skyrim),
    ("autoreadonly", SyntaxKind::AUTOREADONLY_KW, GameId::Skyrim),
    ("betaonly", SyntaxKind::BETAONLY_KW, GameId::Fallout4),
    ("bool", SyntaxKind::BOOL_KW, GameId::Skyrim),
    ("collapsed", SyntaxKind::COLLAPSED_KW, GameId::Fallout4),
    (
        "collapsedonbase",
        SyntaxKind::COLLAPSEDONBASE_KW,
        GameId::Fallout4,
    ),
    (
        "collapsedonref",
        SyntaxKind::COLLAPSEDONREF_KW,
        GameId::Fallout4,
    ),
    ("conditional", SyntaxKind::CONDITIONAL_KW, GameId::Skyrim),
    ("const", SyntaxKind::CONST_KW, GameId::Fallout4),
    ("customevent", SyntaxKind::CUSTOMEVENT_KW, GameId::Fallout4),
    (
        "customeventname",
        SyntaxKind::CUSTOMEVENTNAME_KW,
        GameId::Fallout4,
    ),
    ("debugonly", SyntaxKind::DEBUGONLY_KW, GameId::Fallout4),
    ("default", SyntaxKind::DEFAULT_KW, GameId::Fallout4),
    ("else", SyntaxKind::ELSE_KW, GameId::Skyrim),
    ("elseif", SyntaxKind::ELSEIF_KW, GameId::Skyrim),
    (
        "elsetrylockguard",
        SyntaxKind::ELSETRYLOCKGUARD_KW,
        GameId::Starfield,
    ),
    ("endevent", SyntaxKind::ENDEVENT_KW, GameId::Skyrim),
    ("endfunction", SyntaxKind::ENDFUNCTION_KW, GameId::Skyrim),
    ("endgroup", SyntaxKind::ENDGROUP_KW, GameId::Fallout4),
    ("endif", SyntaxKind::ENDIF_KW, GameId::Skyrim),
    (
        "endlockguard",
        SyntaxKind::ENDLOCKGUARD_KW,
        GameId::Starfield,
    ),
    ("endproperty", SyntaxKind::ENDPROPERTY_KW, GameId::Skyrim),
    ("endstate", SyntaxKind::ENDSTATE_KW, GameId::Skyrim),
    ("endstruct", SyntaxKind::ENDSTRUCT_KW, GameId::Fallout4),
    (
        "endtrylockguard",
        SyntaxKind::ENDTRYLOCKGUARD_KW,
        GameId::Starfield,
    ),
    ("endwhile", SyntaxKind::ENDWHILE_KW, GameId::Skyrim),
    ("event", SyntaxKind::EVENT_KW, GameId::Skyrim),
    ("extends", SyntaxKind::EXTENDS_KW, GameId::Skyrim),
    ("false", SyntaxKind::FALSE_KW, GameId::Skyrim),
    ("float", SyntaxKind::FLOAT_KW, GameId::Skyrim),
    ("function", SyntaxKind::FUNCTION_KW, GameId::Skyrim),
    ("global", SyntaxKind::GLOBAL_KW, GameId::Skyrim),
    ("group", SyntaxKind::GROUP_KW, GameId::Fallout4),
    ("guard", SyntaxKind::GUARD_KW, GameId::Starfield),
    ("hidden", SyntaxKind::HIDDEN_KW, GameId::Skyrim),
    ("if", SyntaxKind::IF_KW, GameId::Skyrim),
    ("import", SyntaxKind::IMPORT_KW, GameId::Skyrim),
    ("int", SyntaxKind::INT_KW, GameId::Skyrim),
    // `is` is rejected by the Skyrim SE compiler — verified.
    ("is", SyntaxKind::IS_KW, GameId::Fallout4),
    ("length", SyntaxKind::LENGTH_KW, GameId::Skyrim),
    ("lockguard", SyntaxKind::LOCKGUARD_KW, GameId::Starfield),
    ("mandatory", SyntaxKind::MANDATORY_KW, GameId::Fallout4),
    ("native", SyntaxKind::NATIVE_KW, GameId::Skyrim),
    ("new", SyntaxKind::NEW_KW, GameId::Skyrim),
    ("none", SyntaxKind::NONE_KW, GameId::Skyrim),
    ("parent", SyntaxKind::PARENT_KW, GameId::Skyrim),
    ("private", SyntaxKind::PRIVATE_KW, GameId::Starfield),
    ("property", SyntaxKind::PROPERTY_KW, GameId::Skyrim),
    ("protected", SyntaxKind::PROTECTED_KW, GameId::Starfield),
    (
        "protectsfunctionlogic",
        SyntaxKind::PROTECTSFUNCTIONLOGIC_KW,
        GameId::Starfield,
    ),
    (
        "requiresguard",
        SyntaxKind::REQUIRESGUARD_KW,
        GameId::Starfield,
    ),
    ("return", SyntaxKind::RETURN_KW, GameId::Skyrim),
    (
        "scripteventname",
        SyntaxKind::SCRIPTEVENTNAME_KW,
        GameId::Fallout4,
    ),
    ("scriptname", SyntaxKind::SCRIPTNAME_KW, GameId::Skyrim),
    ("self", SyntaxKind::SELF_KW, GameId::Skyrim),
    ("selfonly", SyntaxKind::SELFONLY_KW, GameId::Starfield),
    ("state", SyntaxKind::STATE_KW, GameId::Skyrim),
    ("string", SyntaxKind::STRING_KW, GameId::Skyrim),
    ("struct", SyntaxKind::STRUCT_KW, GameId::Fallout4),
    (
        "structvarname",
        SyntaxKind::STRUCTVARNAME_KW,
        GameId::Fallout4,
    ),
    ("true", SyntaxKind::TRUE_KW, GameId::Skyrim),
    (
        "trylockguard",
        SyntaxKind::TRYLOCKGUARD_KW,
        GameId::Starfield,
    ),
    ("var", SyntaxKind::VAR_KW, GameId::Fallout4),
    ("while", SyntaxKind::WHILE_KW, GameId::Skyrim),
];

// Deliberately not reserved:
//
// - `Internal` — described by Starfield's ANTLR grammar as an access
//   modifier, but absent from the 5,033 vanilla scripts. Reserving it would
//   break any script declaring an `internal` variable, for no gain.
// - `For`, `ForEach`, `Switch`, `Break`, `Continue`, `Do` — Caprica-only
//   extensions, never accepted by a Bethesda compiler. They will come with
//   `Extensions::caprica_control_flow`.
// - `ScriptObject`, `GlobalVariable`, `Form`, `Actor`… — engine types,
//   resolved by the type system, not by the grammar.

/// Compares a table name, already lowercase, with text of any case.
///
/// Papyrus is case-insensitive everywhere, identifiers included. Since the
/// lexer restricts identifiers to ASCII, a byte-by-byte comparison is
/// enough.
fn cmp_ignore_ascii_case(probe_lower: &str, target: &str) -> Ordering {
    let mut probe = probe_lower.bytes();
    let mut target = target.bytes();
    loop {
        match (probe.next(), target.next()) {
            (Some(p), Some(t)) => match p.cmp(&t.to_ascii_lowercase()) {
                Ordering::Equal => {}
                other => return other,
            },
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// Looks `text` up in the table, case-insensitively.
///
/// Returns the token and the game from which the word is reserved. Dialect
/// filtering does not happen here: see [`crate::token_source::promote`].
pub fn lookup(text: &str) -> Option<(SyntaxKind, GameId)> {
    KEYWORDS
        .binary_search_by(|(name, _, _)| cmp_ignore_ascii_case(name, text))
        .ok()
        .map(|i| {
            let (_, kind, since) = KEYWORDS[i];
            (kind, since)
        })
}

/// The game from which `kind` is a keyword, or `None` if it is not one.
///
/// Linear search: only called to build diagnostics, never on the lexing hot
/// path.
pub fn since(kind: SyntaxKind) -> Option<GameId> {
    KEYWORDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(_, _, since)| *since)
}

/// Canonical spelling of a keyword, as Bethesda writes it.
///
/// Will serve the formatter for `keyword-case = "bethesda"`. The table stores
/// lowercase names, which are the lookup form.
pub fn canonical_lower(kind: SyntaxKind) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(name, _, _)| *name)
}

/// Number of reserved words, all games combined.
pub fn count() -> usize {
    KEYWORDS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_without_duplicates() {
        for pair in KEYWORDS.windows(2) {
            let (a, _, _) = pair[0];
            let (b, _, _) = pair[1];
            assert!(a < b, "unsorted table: `{a}` comes before `{b}`");
        }
    }

    #[test]
    fn names_are_ascii_lowercase() {
        for (name, _, _) in KEYWORDS {
            assert!(
                name.is_ascii() && *name == name.to_ascii_lowercase(),
                "`{name}` must be ASCII lowercase"
            );
        }
    }

    #[test]
    fn each_token_appears_once() {
        let mut kinds: Vec<_> = KEYWORDS.iter().map(|(_, k, _)| *k).collect();
        let total = kinds.len();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), total, "a SyntaxKind is bound to two keywords");
    }

    #[test]
    fn lookup_ignores_case() {
        for spelling in ["if", "IF", "If", "iF"] {
            assert_eq!(lookup(spelling), Some((SyntaxKind::IF_KW, GameId::Skyrim)));
        }
        assert_eq!(
            lookup("EndTryLockGuard"),
            Some((SyntaxKind::ENDTRYLOCKGUARD_KW, GameId::Starfield))
        );
    }

    #[test]
    fn every_word_of_the_table_is_found() {
        for (name, kind, since) in KEYWORDS {
            assert_eq!(lookup(name), Some((*kind, *since)), "`{name}` not found");
            assert_eq!(lookup(&name.to_ascii_uppercase()), Some((*kind, *since)));
        }
    }

    #[test]
    fn an_ordinary_identifier_is_not_a_keyword() {
        for word in [
            "myVariable",
            "actor",
            "onhit",
            "ifx",
            "i",
            "internal",
            "foreach",
        ] {
            assert_eq!(lookup(word), None, "`{word}` should not be reserved");
        }
    }

    #[test]
    fn prefixes_do_not_match() {
        // The comparison must tell a word from its prefix.
        assert_eq!(lookup("self"), Some((SyntaxKind::SELF_KW, GameId::Skyrim)));
        assert_eq!(
            lookup("selfonly"),
            Some((SyntaxKind::SELFONLY_KW, GameId::Starfield))
        );
        assert_eq!(lookup("sel"), None);
        assert_eq!(lookup("selfonlyx"), None);
    }

    #[test]
    fn the_reverse_lookup_is_consistent() {
        for (name, kind, since) in KEYWORDS {
            assert_eq!(super::since(*kind), Some(*since));
            assert_eq!(canonical_lower(*kind), Some(*name));
        }
        assert_eq!(super::since(SyntaxKind::IDENT), None);
        assert_eq!(super::since(SyntaxKind::L_PAREN), None);
    }
}
