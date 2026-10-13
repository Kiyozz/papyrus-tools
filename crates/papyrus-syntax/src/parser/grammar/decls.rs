//! The file, its header and the script-level declarations.

use papyrus_dialect::Feature;

use super::{
    Parser, STMT_RECOVERY, block_end, eol, error_line, exprs, flag_list, incompatible, name,
    script_name, stmts, type_len, type_ref, type_ref_without_array,
};
use crate::SyntaxKind::*;
use crate::parser::Marker;

/// The root: header, then declarations up to the end of the file.
///
/// Only stops at `EOF`: a token left aside would break the round trip.
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
        p.error("a script starts with `ScriptName`");
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

/// A script-level declaration. Always consumes at least one token.
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
        _ => error_line(p, "expected a declaration"),
    }
}

/// True if the line is a type followed by `kw`: `int Function`, `Actor Property`.
fn at_typed(p: &Parser, kw: crate::SyntaxKind) -> bool {
    type_len(p, 0).is_some_and(|len| p.nth(len) == kw)
}

/// A declaration opened by a type: property, function or variable.
fn typed_item(p: &mut Parser) {
    let m = p.start();
    type_ref(p);
    match p.current() {
        PROPERTY_KW => property_rest(p, m),
        FUNCTION_KW => function_rest(p, m),
        IDENT => variable_rest(p, m),
        _ => {
            p.error("expected `Property`, `Function` or a variable name");
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

/// Script or local variable: `Actor a = None Conditional`.
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
    // `Conditional`, `Hidden`, `RequiresGuard(g)` — Starfield even puts them
    // on variables.
    flag_list(p);
    eol(p);
    m.complete(p, VARIABLE_DECL);
}

/// A function, with or without a return type.
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
    // A native function has neither body nor `EndFunction`.
    if !flags.native {
        stmts::stmt_list(p);
        block_end(p, ENDFUNCTION_KW, "`EndFunction`");
    }
    m.complete(p, FUNCTION_DECL);
}

/// `Event OnInit()`, or Fallout 4's remote form:
/// `Event ObjectReference.OnActivate(ObjectReference akSender, …)`.
fn event(p: &mut Parser) {
    let m = p.start();
    p.bump(EVENT_KW);
    if type_len(p, 0).is_some_and(|len| p.nth(len) == DOT) {
        if !p.dialect().supports(Feature::RemoteEvent) {
            let range = p.current_range();
            p.error_at(range, incompatible(p, Feature::RemoteEvent));
        }
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
        p.error("expected `(`");
        return;
    }
    let m = p.start();
    p.bump(L_PAREN);
    while !p.at(R_PAREN) && !p.at(NEWLINE) && !p.at(EOF) {
        if type_len(p, 0).is_some() {
            param(p);
        } else {
            let e = p.start();
            p.error("expected a parameter");
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

/// A property, short or long.
///
/// The form is decided by the flags, not by their position: Starfield writes
/// `Mandatory Const Auto` as well as `Auto Const Mandatory`. Without `Auto`
/// or `AutoReadOnly`, the property has a body ending with `EndProperty`.
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
                // Most likely the next declaration: `EndProperty` was
                // forgotten, no need to swallow the rest of the file.
                break;
            } else {
                error_line(p, "expected `Function` or `EndProperty`");
            }
        }
        block_end(p, ENDPROPERTY_KW, "`EndProperty`");
    }
    m.complete(p, PROPERTY_DECL);
}

/// `Auto State Foo … EndState`: only functions and events.
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
            error_line(p, "expected `Function`, `Event` or `EndState`");
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
            error_line(p, "expected a field or `EndStruct`");
        }
    }
    block_end(p, ENDSTRUCT_KW, "`EndStruct`");
    m.complete(p, STRUCT_DECL);
}

/// `Group Foo CollapsedOnRef … EndGroup`: only properties.
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
            error_line(p, "expected a property or `EndGroup`");
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
