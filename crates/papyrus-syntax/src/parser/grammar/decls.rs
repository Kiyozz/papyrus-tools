//! Le fichier, son en-tête et les déclarations de portée script.

use super::{
    Parser, STMT_RECOVERY, block_end, eol, error_line, exprs, flag_list, name, script_name, stmts,
    type_len, type_ref, type_ref_without_array,
};
use crate::SyntaxKind::*;
use crate::parser::Marker;

/// La racine : en-tête, puis déclarations jusqu'à la fin du fichier.
///
/// Ne s'arrête qu'à `EOF` : un token laissé de côté casserait le round-trip.
pub(crate) fn source_file(p: &mut Parser) {
    let m = p.start();
    while p.eat(NEWLINE) {}
    script_header(p);
    while !p.at(EOF) {
        if p.eat(NEWLINE) {
            continue;
        }
        item(p);
    }
    m.complete(p, SOURCE_FILE);
}

/// `ScriptName Foo extends Bar Hidden Conditional`
fn script_header(p: &mut Parser) {
    if !p.at(SCRIPTNAME_KW) {
        p.error("un script commence par `ScriptName`");
        return;
    }
    let m = p.start();
    p.bump(SCRIPTNAME_KW);
    script_name(p);
    if p.at(EXTENDS_KW) {
        let e = p.start();
        p.bump(EXTENDS_KW);
        type_ref_without_array(p);
        e.complete(p, EXTENDS_CLAUSE);
    }
    flag_list(p);
    eol(p);
    m.complete(p, SCRIPT_HEADER);
}

/// Une déclaration de portée script. Consomme toujours au moins un token.
fn item(p: &mut Parser) {
    match p.current() {
        IMPORT_KW => import(p),
        STATE_KW => state(p),
        AUTO_KW if p.nth(1) == STATE_KW => state(p),
        FUNCTION_KW => function(p),
        EVENT_KW => event(p),
        STRUCT_KW => struct_decl(p),
        GROUP_KW => group(p),
        CUSTOMEVENT_KW => custom_event(p),
        GUARD_KW => guard(p),
        _ if type_len(p, 0).is_some() => typed_item(p),
        _ => error_line(p, "déclaration attendue"),
    }
}

/// Vrai si la ligne est un type suivi de `kw` : `int Function`, `Actor Property`.
fn at_typed(p: &Parser, kw: crate::SyntaxKind) -> bool {
    type_len(p, 0).is_some_and(|len| p.nth(len) == kw)
}

/// Une déclaration ouverte par un type : property, fonction ou variable.
fn typed_item(p: &mut Parser) {
    let m = p.start();
    type_ref(p);
    match p.current() {
        PROPERTY_KW => property_rest(p, m),
        FUNCTION_KW => function_rest(p, m),
        IDENT => variable_rest(p, m),
        _ => {
            p.error("`Property`, `Function` ou un nom de variable attendu");
            super::skip_line(p);
            p.eat(NEWLINE);
            m.complete(p, ERROR_TREE);
        }
    }
}

/// `import Foo:Bar`
fn import(p: &mut Parser) {
    let m = p.start();
    p.bump(IMPORT_KW);
    type_ref_without_array(p);
    eol(p);
    m.complete(p, IMPORT);
}

/// Variable de script ou locale : `Actor a = None Conditional`.
pub(super) fn variable_decl(p: &mut Parser) {
    let m = p.start();
    type_ref(p);
    variable_rest(p, m);
}

fn variable_rest(p: &mut Parser, m: Marker) {
    name(p);
    if p.eat(EQ) {
        exprs::expr(p);
    }
    // `Conditional`, `Hidden`, `RequiresGuard(g)` — Starfield en met même sur
    // les variables.
    flag_list(p);
    eol(p);
    m.complete(p, VARIABLE_DECL);
}

/// Une fonction, avec ou sans type de retour.
fn function(p: &mut Parser) {
    let m = p.start();
    if !p.at(FUNCTION_KW) {
        type_ref(p);
    }
    function_rest(p, m);
}

fn function_rest(p: &mut Parser, m: Marker) {
    p.bump(FUNCTION_KW);
    name(p);
    param_list(p);
    let flags = flag_list(p);
    eol(p);
    // Une fonction native n'a ni corps ni `EndFunction`.
    if !flags.native {
        stmts::stmt_list(p);
        block_end(p, ENDFUNCTION_KW, "`EndFunction`");
    }
    m.complete(p, FUNCTION_DECL);
}

/// `Event OnInit()`, ou la forme distante de Fallout 4 :
/// `Event ObjectReference.OnActivate(ObjectReference akSender, …)`.
fn event(p: &mut Parser) {
    let m = p.start();
    p.bump(EVENT_KW);
    if type_len(p, 0).is_some_and(|len| p.nth(len) == DOT) {
        type_ref(p);
        p.bump(DOT);
    }
    name(p);
    param_list(p);
    let flags = flag_list(p);
    eol(p);
    if !flags.native {
        stmts::stmt_list(p);
        block_end(p, ENDEVENT_KW, "`EndEvent`");
    }
    m.complete(p, EVENT_DECL);
}

/// `(int aiCount = 3, Actor akRef)`
fn param_list(p: &mut Parser) {
    if !p.at(L_PAREN) {
        p.error("`(` attendu");
        return;
    }
    let m = p.start();
    p.bump(L_PAREN);
    while !p.at(R_PAREN) && !p.at(NEWLINE) && !p.at(EOF) {
        if type_len(p, 0).is_some() {
            param(p);
        } else {
            let e = p.start();
            p.error("paramètre attendu");
            p.bump_any();
            e.complete(p, ERROR_TREE);
        }
        if !p.eat(COMMA) {
            break;
        }
    }
    p.expect(R_PAREN, "`)`");
    m.complete(p, PARAM_LIST);
}

fn param(p: &mut Parser) {
    let m = p.start();
    type_ref(p);
    name(p);
    if p.eat(EQ) {
        exprs::expr(p);
    }
    m.complete(p, PARAM);
}

/// Une property, courte ou longue.
///
/// La forme se décide aux flags et non à leur position : Starfield écrit
/// `Mandatory Const Auto` aussi bien que `Auto Const Mandatory`. Sans `Auto`
/// ni `AutoReadOnly`, la property a un corps qui se termine par `EndProperty`.
fn property_rest(p: &mut Parser, m: Marker) {
    p.bump(PROPERTY_KW);
    name(p);
    if p.eat(EQ) {
        exprs::expr(p);
    }
    let flags = flag_list(p);
    eol(p);

    if !flags.auto {
        loop {
            if p.at(EOF) || p.at(ENDPROPERTY_KW) {
                break;
            }
            if p.eat(NEWLINE) {
                continue;
            }
            if p.at(FUNCTION_KW) || at_typed(p, FUNCTION_KW) {
                function(p);
            } else if p.at_ts(STMT_RECOVERY) || type_len(p, 0).is_some() {
                // Sans doute la déclaration suivante : `EndProperty` a été
                // oublié, inutile d'avaler le reste du fichier.
                break;
            } else {
                error_line(p, "`Function` ou `EndProperty` attendu");
            }
        }
        block_end(p, ENDPROPERTY_KW, "`EndProperty`");
    }
    m.complete(p, PROPERTY_DECL);
}

/// `Auto State Foo … EndState` : seulement des fonctions et des événements.
fn state(p: &mut Parser) {
    let m = p.start();
    p.eat(AUTO_KW);
    p.bump(STATE_KW);
    name(p);
    eol(p);
    loop {
        if p.at(EOF) || p.at(ENDSTATE_KW) {
            break;
        }
        if p.eat(NEWLINE) {
            continue;
        }
        if p.at(EVENT_KW) {
            event(p);
        } else if p.at(FUNCTION_KW) || at_typed(p, FUNCTION_KW) {
            function(p);
        } else if p.at_ts(STMT_RECOVERY) || type_len(p, 0).is_some() || p.at(AUTO_KW) {
            break;
        } else {
            error_line(p, "`Function`, `Event` ou `EndState` attendu");
        }
    }
    block_end(p, ENDSTATE_KW, "`EndState`");
    m.complete(p, STATE_DECL);
}

/// `Struct Foo … EndStruct`
fn struct_decl(p: &mut Parser) {
    let m = p.start();
    p.bump(STRUCT_KW);
    name(p);
    eol(p);
    loop {
        if p.at(EOF) || p.at(ENDSTRUCT_KW) {
            break;
        }
        if p.eat(NEWLINE) {
            continue;
        }
        if at_typed(p, IDENT) {
            let f = p.start();
            type_ref(p);
            name(p);
            if p.eat(EQ) {
                exprs::expr(p);
            }
            flag_list(p);
            eol(p);
            f.complete(p, STRUCT_FIELD);
        } else if p.at_ts(STMT_RECOVERY) || type_len(p, 0).is_some() {
            break;
        } else {
            error_line(p, "champ ou `EndStruct` attendu");
        }
    }
    block_end(p, ENDSTRUCT_KW, "`EndStruct`");
    m.complete(p, STRUCT_DECL);
}

/// `Group Foo CollapsedOnRef … EndGroup` : seulement des properties.
fn group(p: &mut Parser) {
    let m = p.start();
    p.bump(GROUP_KW);
    name(p);
    flag_list(p);
    eol(p);
    loop {
        if p.at(EOF) || p.at(ENDGROUP_KW) {
            break;
        }
        if p.eat(NEWLINE) {
            continue;
        }
        if at_typed(p, PROPERTY_KW) {
            let item = p.start();
            type_ref(p);
            property_rest(p, item);
        } else if p.at_ts(STMT_RECOVERY) || type_len(p, 0).is_some() {
            break;
        } else {
            error_line(p, "property ou `EndGroup` attendu");
        }
    }
    block_end(p, ENDGROUP_KW, "`EndGroup`");
    m.complete(p, GROUP_DECL);
}

/// `CustomEvent Foo`
fn custom_event(p: &mut Parser) {
    let m = p.start();
    p.bump(CUSTOMEVENT_KW);
    name(p);
    eol(p);
    m.complete(p, CUSTOM_EVENT_DECL);
}

/// `Guard foo ProtectsFunctionLogic`
fn guard(p: &mut Parser) {
    let m = p.start();
    p.bump(GUARD_KW);
    name(p);
    flag_list(p);
    eol(p);
    m.complete(p, GUARD_DECL);
}
