//! La grammaire de Papyrus, sur-ensemble des trois moteurs.
//!
//! Chaque fonction lit une construction et émet ses événements. Les règles qui
//! ne valent que pour certains jeux tiennent en deux mécanismes :
//!
//! - les mots-clés récents ne sont reconnus que si la cible les réserve (voir
//!   [`crate::token_source`]) — rien à faire ici ;
//! - les constructions qui n'ouvrent pas sur un mot-clé, comme les namespaces,
//!   sont toujours lues, et signalées si la cible ne les accepte pas. L'arbre
//!   reste complet dans les deux cas.
//!
//! # Récupération d'erreurs
//!
//! Trois niveaux, du plus fin au plus large :
//!
//! 1. [`Parser::expect`] signale l'absence d'un token sans rien consommer ;
//! 2. [`eol`] ancre sur la ligne : ce qui traîne avant la fin de ligne part
//!    dans un [`ERROR_TREE`], la ligne suivante repart de zéro ;
//! 3. [`STMT_RECOVERY`] ancre sur les blocs : une liste d'instructions s'arrête
//!    sur tout mot qui ferme un bloc ou ouvre une déclaration, quelle que soit
//!    la profondeur, si bien qu'un `EndIf` oublié n'emporte pas le fichier.

mod decls;
mod exprs;
mod stmts;

use papyrus_dialect::Feature;

use super::{Parser, TokenSet};
use crate::SyntaxKind::{self, *};

pub(super) use decls::source_file;

/// Tokens qui peuvent ouvrir un type.
///
/// `CustomEventName`, `ScriptEventName` et `StructVarName` sont des mots-clés
/// *et* des types : `ScriptObject.psc` déclare
/// `RegisterForCustomEvent(ScriptObject akSender, CustomEventName asEventName)`.
const TYPE_START: TokenSet = TokenSet::new(&[
    INT_KW,
    FLOAT_KW,
    BOOL_KW,
    STRING_KW,
    VAR_KW,
    CUSTOMEVENTNAME_KW,
    SCRIPTEVENTNAME_KW,
    STRUCTVARNAME_KW,
    IDENT,
]);

/// Flags de script, de déclaration et de fonction, tous jeux confondus.
///
/// La grammaire ne vérifie pas qu'un flag est permis à l'endroit où il paraît :
/// c'est l'affaire d'un lint, qui saura dire pourquoi.
const FLAGS: TokenSet = TokenSet::new(&[
    AUTO_KW,
    AUTOREADONLY_KW,
    HIDDEN_KW,
    CONDITIONAL_KW,
    NATIVE_KW,
    GLOBAL_KW,
    CONST_KW,
    MANDATORY_KW,
    DEBUGONLY_KW,
    BETAONLY_KW,
    COLLAPSED_KW,
    COLLAPSEDONREF_KW,
    COLLAPSEDONBASE_KW,
    DEFAULT_KW,
    PRIVATE_KW,
    PROTECTED_KW,
    SELFONLY_KW,
    REQUIRESGUARD_KW,
    PROTECTSFUNCTIONLOGIC_KW,
]);

/// Les mots sur lesquels toute liste d'instructions s'arrête.
///
/// Aucun ne peut ouvrir une instruction : s'arrêter dessus ne coupe jamais du
/// code valide, et chaque construction réclame ensuite son propre terminateur.
const STMT_RECOVERY: TokenSet = TokenSet::new(&[
    EOF,
    ENDFUNCTION_KW,
    ENDEVENT_KW,
    ENDIF_KW,
    ELSEIF_KW,
    ELSE_KW,
    ENDWHILE_KW,
    ENDPROPERTY_KW,
    ENDSTATE_KW,
    ENDSTRUCT_KW,
    ENDGROUP_KW,
    ENDLOCKGUARD_KW,
    ELSETRYLOCKGUARD_KW,
    ENDTRYLOCKGUARD_KW,
    FUNCTION_KW,
    EVENT_KW,
    PROPERTY_KW,
    STATE_KW,
    STRUCT_KW,
    GROUP_KW,
    CUSTOMEVENT_KW,
    SCRIPTNAME_KW,
    IMPORT_KW,
]);

/// Ce que [`flag_list`] a vu et qui change la forme de la déclaration.
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    /// `Auto` ou `AutoReadOnly` : la property n'a pas de corps.
    auto: bool,
    /// `Native` : la fonction n'a pas de corps.
    native: bool,
}

/// Consomme la fin de ligne, ou range ce qui traîne avant dans une erreur.
fn eol(p: &mut Parser) {
    if p.eat(NEWLINE) || p.at(EOF) {
        return;
    }
    p.error("fin de ligne attendue");
    skip_line(p);
    p.eat(NEWLINE);
}

/// Range le reste de la ligne dans un [`ERROR_TREE`].
///
/// S'arrête aussi sur [`STMT_RECOVERY`] : un `EndFunction` collé à du texte
/// invalide doit encore fermer sa fonction.
fn skip_line(p: &mut Parser) {
    if p.at(NEWLINE) || p.at_ts(STMT_RECOVERY) {
        return;
    }
    let m = p.start();
    while !p.at(NEWLINE) && !p.at_ts(STMT_RECOVERY) {
        p.bump_any();
    }
    m.complete(p, ERROR_TREE);
}

/// Signale une ligne incompréhensible et la saute entièrement.
///
/// Consomme toujours au moins un token, ce qui garantit que la boucle
/// appelante progresse.
fn error_line(p: &mut Parser, message: &str) {
    p.error(message);
    let m = p.start();
    p.bump_any();
    while !p.at(NEWLINE) && !p.at_ts(STMT_RECOVERY) {
        p.bump_any();
    }
    m.complete(p, ERROR_TREE);
    p.eat(NEWLINE);
}

/// Un nom de déclaration : un identifiant simple.
///
/// Les flags n'étant réservés qu'en position de flag, ils sont admis comme
/// noms — le vanilla écrit `Auto State Default` et `Group Mandatory`.
fn name(p: &mut Parser) {
    if p.at(IDENT) || p.at_ts(FLAGS) {
        let m = p.start();
        p.bump_any();
        m.complete(p, NAME);
    } else {
        p.error("nom attendu");
    }
}

/// Un nom de script, éventuellement qualifié : `DLC01:MonScript`.
fn script_name(p: &mut Parser) {
    if !p.at(IDENT) {
        p.error("nom de script attendu");
        return;
    }
    let m = p.start();
    p.bump(IDENT);
    namespace_tail(p);
    m.complete(p, NAME);
}

/// `:Segment` répété, avec le diagnostic de compatibilité.
fn namespace_tail(p: &mut Parser) {
    while p.at(COLON) && p.nth(1) == IDENT {
        if !p.dialect().supports(Feature::Namespace) {
            let range = p.current_range();
            p.error_at(range, incompatible(p, Feature::Namespace));
        }
        p.bump(COLON);
        p.bump(IDENT);
    }
}

/// Le message d'une construction que la cible ne connaît pas.
fn incompatible(p: &Parser, feature: Feature) -> String {
    format!(
        "{} : non disponible pour {} (à partir de {})",
        feature.describe(),
        p.dialect().game.display_name(),
        feature.since().display_name()
    )
}

/// Vrai si un type commence ici.
fn at_type(p: &Parser) -> bool {
    p.at_ts(TYPE_START)
}

/// `int`, `Foo:Bar`, `Actor[]`.
fn type_ref(p: &mut Parser) {
    let m = p.start();
    type_base(p);
    if p.at(L_BRACK) && p.nth(1) == R_BRACK {
        p.bump(L_BRACK);
        p.bump(R_BRACK);
    }
    m.complete(p, TYPE_REF);
}

/// Un type sans suffixe de tableau, pour `new float[4]` où les crochets
/// portent la taille.
fn type_ref_without_array(p: &mut Parser) {
    let m = p.start();
    type_base(p);
    m.complete(p, TYPE_REF);
}

fn type_base(p: &mut Parser) {
    if at_type(p) {
        let simple = !p.at(IDENT);
        p.bump_any();
        if !simple {
            namespace_tail(p);
        }
    } else {
        p.error("type attendu");
    }
}

/// Longueur, en tokens, du type qui commence au `n`-ième token — ou `None`.
///
/// Sert à distinguer `Actor a` (déclaration) de `a.b()` ou `a = 1`
/// (instructions) sans rien consommer.
fn type_len(p: &Parser, n: usize) -> Option<usize> {
    let mut len = match p.nth(n) {
        INT_KW | FLOAT_KW | BOOL_KW | STRING_KW | VAR_KW | CUSTOMEVENTNAME_KW
        | SCRIPTEVENTNAME_KW | STRUCTVARNAME_KW => 1,
        IDENT => {
            let mut len = 1;
            while p.nth(n + len) == COLON && p.nth(n + len + 1) == IDENT {
                len += 2;
            }
            len
        }
        _ => return None,
    };
    if p.nth(n + len) == L_BRACK && p.nth(n + len + 1) == R_BRACK {
        len += 2;
    }
    Some(len)
}

/// Suite de flags, éventuellement vide. `RequiresGuard(g)` porte des
/// arguments.
fn flag_list(p: &mut Parser) -> Flags {
    let mut flags = Flags::default();
    let list = p.start();
    while p.at_ts(FLAGS) {
        match p.current() {
            AUTO_KW | AUTOREADONLY_KW => flags.auto = true,
            NATIVE_KW => flags.native = true,
            _ => {}
        }
        let m = p.start();
        p.bump_any();
        if p.at(L_PAREN) {
            exprs::arg_list(p);
        }
        m.complete(p, FLAG);
    }
    list.complete(p, FLAG_LIST);
    flags
}

/// Termine un bloc : consomme `end` et sa fin de ligne, ou signale l'oubli.
fn block_end(p: &mut Parser, end: SyntaxKind, what: &str) {
    if p.expect(end, what) {
        eol(p);
    }
}
