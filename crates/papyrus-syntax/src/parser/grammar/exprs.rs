//! Expressions.
//!
//! Precedences, from loosest to tightest, after the Creation Kit reference
//! grammar:
//!
//! | Level   | Operators                    |
//! |---------|------------------------------|
//! | 1       | `\|\|`                       |
//! | 2       | `&&`                         |
//! | 3       | `==` `!=` `<` `>` `<=` `>=`  |
//! | 4       | `+` `-`                      |
//! | 5       | `*` `/` `%`                  |
//! | unary   | `-` `!`                      |
//! | cast    | `as`, `is`                   |
//! | postfix | `.member`, `[i]`, `(args)`   |
//!
//! All binary operators are left-associative. Unary wraps the cast:
//! `-x as Float` reads `-(x as Float)`.
//!
//! An expression never crosses a line end: `NEWLINE` is neither an operator
//! nor the start of an operand. Only the `\` continuation, which is trivia,
//! lets it span several lines.

use super::{Parser, namespace_tail, type_ref, type_ref_without_array};
use crate::SyntaxKind::*;
use crate::parser::CompletedMarker;

/// Reads an expression. False if none starts here — the error is then
/// reported and nothing was consumed.
pub(super) fn expr(p: &mut Parser) -> bool {
    expr_bp(p, 0).is_some()
}

/// Binding power of a binary operator, `None` for any other token.
fn binding_power(p: &Parser) -> Option<u8> {
    Some(match p.current() {
        PIPE2 => 1,
        AMP2 => 2,
        EQ2 | NEQ | LT | GT | LTEQ | GTEQ => 3,
        PLUS | MINUS => 4,
        STAR | SLASH | PERCENT => 5,
        _ => return None,
    })
}

/// Precedence climbing.
fn expr_bp(p: &mut Parser, min_bp: u8) -> Option<CompletedMarker> {
    let mut lhs = unary(p)?;
    while let Some(bp) = binding_power(p) {
        if bp <= min_bp {
            break;
        }
        let m = lhs.precede(p);
        p.bump_any();
        expr_bp(p, bp);
        lhs = m.complete(p, BIN_EXPR);
    }
    Some(lhs)
}

fn unary(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(MINUS) || p.at(BANG) {
        let m = p.start();
        p.bump_any();
        unary(p);
        return Some(m.complete(p, UNARY_EXPR));
    }
    cast(p)
}

/// `x as Actor`, `x is Actor` — the latter from Fallout 4 on, which keyword
/// promotion already handles.
fn cast(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = postfix(p)?;
    while p.at(AS_KW) || p.at(IS_KW) {
        let m = lhs.precede(p);
        p.bump_any();
        type_ref(p);
        lhs = m.complete(p, CAST_EXPR);
    }
    Some(lhs)
}

fn postfix(p: &mut Parser) -> Option<CompletedMarker> {
    let mut lhs = atom(p)?;
    loop {
        lhs = match p.current() {
            DOT => {
                let m = lhs.precede(p);
                p.bump(DOT);
                member(p);
                m.complete(p, FIELD_EXPR)
            }
            L_BRACK => {
                let m = lhs.precede(p);
                p.bump(L_BRACK);
                expr(p);
                p.expect(R_BRACK, "`]`");
                m.complete(p, INDEX_EXPR)
            }
            L_PAREN => {
                let m = lhs.precede(p);
                arg_list(p);
                m.complete(p, CALL_EXPR)
            }
            _ => break,
        };
    }
    Some(lhs)
}

/// The name after a dot.
///
/// Reserved words are accepted there: the vanilla corpus writes `.Length`,
/// and members named like keywords. The risk is nil, since the line end is
/// not a keyword and stops the read.
fn member(p: &mut Parser) {
    let kind = p.current();
    if kind == IDENT || kind.is_keyword() {
        let m = p.start();
        p.bump_any();
        m.complete(p, NAME_REF);
    } else {
        p.error("expected a member name");
    }
}

fn atom(p: &mut Parser) -> Option<CompletedMarker> {
    let m = match p.current() {
        INT_NUMBER | HEX_NUMBER | FLOAT_NUMBER | STRING | TRUE_KW | FALSE_KW | NONE_KW => {
            let m = p.start();
            p.bump_any();
            m.complete(p, LITERAL)
        }
        IDENT => {
            // Qualified name: `DLC04:ManagerScript.GetScript()` calls a global
            // function of a namespaced script.
            let m = p.start();
            p.bump(IDENT);
            namespace_tail(p);
            m.complete(p, NAME_REF)
        }
        SELF_KW | PARENT_KW => {
            let m = p.start();
            p.bump_any();
            m.complete(p, NAME_REF)
        }
        L_PAREN => {
            let m = p.start();
            p.bump(L_PAREN);
            expr(p);
            p.expect(R_PAREN, "`)`");
            m.complete(p, PAREN_EXPR)
        }
        NEW_KW => {
            // `new float[4]`: the brackets carry the size, not the type.
            let m = p.start();
            p.bump(NEW_KW);
            type_ref_without_array(p);
            if p.eat(L_BRACK) {
                expr(p);
                p.expect(R_BRACK, "`]`");
            }
            m.complete(p, NEW_EXPR)
        }
        _ => {
            p.error("expected an expression");
            return None;
        }
    };
    Some(m)
}

/// `(a, abForce = true)`, also used by `RequiresGuard(g)`.
pub(super) fn arg_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    while !p.at(R_PAREN) && !p.at(NEWLINE) && !p.at(EOF) {
        let a = p.start();
        // Named argument, allowed since Skyrim: `Activate(akRef, abDefault = true)`.
        if p.at(IDENT) && p.nth(1) == EQ {
            let n = p.start();
            p.bump(IDENT);
            n.complete(p, NAME_REF);
            p.bump(EQ);
        }
        if !expr(p) && !p.at(COMMA) && !p.at(R_PAREN) && !p.at(NEWLINE) && !p.at(EOF) {
            let e = p.start();
            p.bump_any();
            e.complete(p, ERROR_TREE);
        }
        a.complete(p, ARG);
        if !p.eat(COMMA) {
            break;
        }
    }
    p.expect(R_PAREN, "`)`");
    m.complete(p, ARG_LIST);
}
