//! La table des mots-clés — source de vérité unique de la matrice par jeu.
//!
//! Ajouter un mot-clé, c'est ajouter une ligne ici. Rien d'autre dans le lexer
//! ni dans la grammaire ne connaît la liste.
//!
//! Chaque entrée porte le jeu à partir duquel le mot est réservé. Le lexer
//! produit toujours [`SyntaxKind::IDENT`] pour un identifiant ; c'est
//! [`crate::token_source`] qui le promeut en mot-clé, et seulement si la cible
//! est assez récente. Sans ce filtre, les milliers de scripts Skyrim qui
//! déclarent une variable nommée `Var`, `Struct` ou `Guard` produiraient des
//! erreurs de syntaxe.

use std::cmp::Ordering;

use papyrus_dialect::GameId;

use crate::SyntaxKind;

/// Mots réservés, triés par nom.
///
/// Le tri conditionne la recherche dichotomique ; un test le vérifie.
///
/// Sources : les grammaires ANTLR par jeu de `fireundubh/OpenPapyrus`,
/// recoupées avec le lexer de Caprica et, pour Starfield, avec les scripts
/// vanilla. Les cas litigieux ont été tranchés en compilant un cas de test avec
/// le `PapyrusCompiler.exe` du jeu concerné.
static KEYWORDS: &[(&str, SyntaxKind, GameId)] = &[
    // `as` existe depuis Skyrim, contrairement à ce qu'indique le classement de
    // Caprica : `akRef as Actor` compile avec le compilateur de Skyrim SE.
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
    // `is` est rejeté par le compilateur de Skyrim SE — vérifié.
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

// Non réservés, volontairement :
//
// - `Internal` — décrit par la grammaire ANTLR de Starfield comme un
//   modificateur d'accès, mais absent des 5 033 scripts vanilla. Le réserver
//   casserait tout script déclarant une variable `internal` sans rien apporter.
// - `For`, `ForEach`, `Switch`, `Break`, `Continue`, `Do` — extensions propres
//   à Caprica, jamais acceptées par un compilateur Bethesda. Elles viendront
//   avec `Extensions::caprica_control_flow`.
// - `ScriptObject`, `GlobalVariable`, `Form`, `Actor`… — des types du moteur,
//   résolus par le système de types et non par la grammaire.

/// Compare un nom de la table, déjà en minuscules, à un texte de casse quelconque.
///
/// Papyrus est insensible à la casse partout, y compris sur les identifiants.
/// Les identifiants étant restreints à l'ASCII par le lexer, l'octet à octet
/// suffit.
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

/// Cherche `text` dans la table, sans tenir compte de la casse.
///
/// Renvoie le token et le jeu depuis lequel le mot est réservé. Le filtrage par
/// dialecte n'a pas lieu ici : voir [`crate::token_source::promote`].
pub fn lookup(text: &str) -> Option<(SyntaxKind, GameId)> {
    KEYWORDS
        .binary_search_by(|(name, _, _)| cmp_ignore_ascii_case(name, text))
        .ok()
        .map(|i| {
            let (_, kind, since) = KEYWORDS[i];
            (kind, since)
        })
}

/// Jeu depuis lequel `kind` est un mot-clé, ou `None` si c'en est pas un.
///
/// Recherche linéaire : n'est appelée que pour construire des diagnostics,
/// jamais dans le chemin chaud du lexage.
pub fn since(kind: SyntaxKind) -> Option<GameId> {
    KEYWORDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(_, _, since)| *since)
}

/// Orthographe canonique d'un mot-clé, telle qu'écrite par Bethesda.
///
/// Servira au formateur pour `keyword-case = "bethesda"`. La table stocke les
/// noms en minuscules, qui sont la forme de recherche.
pub fn canonical_lower(kind: SyntaxKind) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(name, _, _)| *name)
}

/// Nombre de mots réservés, tous jeux confondus.
pub fn count() -> usize {
    KEYWORDS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_table_est_triee_et_sans_doublon() {
        for pair in KEYWORDS.windows(2) {
            let (a, _, _) = pair[0];
            let (b, _, _) = pair[1];
            assert!(a < b, "table non triée : `{a}` précède `{b}`");
        }
    }

    #[test]
    fn les_noms_sont_en_minuscules_ascii() {
        for (name, _, _) in KEYWORDS {
            assert!(
                name.is_ascii() && *name == name.to_ascii_lowercase(),
                "`{name}` doit être en minuscules ASCII"
            );
        }
    }

    #[test]
    fn chaque_token_n_apparait_qu_une_fois() {
        let mut kinds: Vec<_> = KEYWORDS.iter().map(|(_, k, _)| *k).collect();
        let total = kinds.len();
        kinds.sort();
        kinds.dedup();
        assert_eq!(
            kinds.len(),
            total,
            "un SyntaxKind est associé à deux mots-clés"
        );
    }

    #[test]
    fn la_recherche_ignore_la_casse() {
        for spelling in ["if", "IF", "If", "iF"] {
            assert_eq!(lookup(spelling), Some((SyntaxKind::IF_KW, GameId::Skyrim)));
        }
        assert_eq!(
            lookup("EndTryLockGuard"),
            Some((SyntaxKind::ENDTRYLOCKGUARD_KW, GameId::Starfield))
        );
    }

    #[test]
    fn tous_les_mots_de_la_table_se_retrouvent() {
        for (name, kind, since) in KEYWORDS {
            assert_eq!(lookup(name), Some((*kind, *since)), "`{name}` introuvable");
            assert_eq!(lookup(&name.to_ascii_uppercase()), Some((*kind, *since)));
        }
    }

    #[test]
    fn un_identifiant_ordinaire_n_est_pas_un_mot_cle() {
        for word in [
            "myVariable",
            "actor",
            "onhit",
            "ifx",
            "i",
            "internal",
            "foreach",
        ] {
            assert_eq!(lookup(word), None, "`{word}` ne devrait pas être réservé");
        }
    }

    #[test]
    fn les_prefixes_ne_matchent_pas() {
        // La comparaison doit distinguer un mot de son préfixe.
        assert_eq!(lookup("self"), Some((SyntaxKind::SELF_KW, GameId::Skyrim)));
        assert_eq!(
            lookup("selfonly"),
            Some((SyntaxKind::SELFONLY_KW, GameId::Starfield))
        );
        assert_eq!(lookup("sel"), None);
        assert_eq!(lookup("selfonlyx"), None);
    }

    #[test]
    fn le_reverse_lookup_est_coherent() {
        for (name, kind, since) in KEYWORDS {
            assert_eq!(super::since(*kind), Some(*since));
            assert_eq!(canonical_lower(*kind), Some(*name));
        }
        assert_eq!(super::since(SyntaxKind::IDENT), None);
        assert_eq!(super::since(SyntaxKind::L_PAREN), None);
    }
}
