//! Statements, in function and event bodies.

use super::{Parser, STMT_RECOVERY, TokenSet, block_end, decls, eol, exprs, type_len};
use crate::SyntaxKind::*;

/// Assignment operators, plain and compound.
const ASSIGN_OPS: TokenSet = TokenSet::new(&[EQ, PLUS_EQ, MINUS_EQ, STAR_EQ, SLASH_EQ, PERCENT_EQ]);

/// A sequence of statements, up to the first word of [`STMT_RECOVERY`].
///
/// The caller consumes the terminator it expects; if it finds something
/// else, the error is its own, and the list swallowed nothing extra.
pub(super) fn stmt_list(p: &mut Parser) {
    let m = p.start();
    while !p.at_ts(STMT_RECOVERY) {
        if p.eat(NEWLINE) {
            continue;
        }
        stmt(p);
    }
    m.complete(p, STMT_LIST);
}

/// A statement. Always consumes at least one token.
fn stmt(p: &mut Parser) {
    match p.current() {
        RETURN_KW => return_stmt(p),
        IF_KW => if_stmt(p),
        WHILE_KW => while_stmt(p),
        LOCKGUARD_KW => lock_guard_stmt(p),
        TRYLOCKGUARD_KW => try_lock_guard_stmt(p),
        // `Actor a`, `int[] t`, `Foo:Bar b` — a type followed by a name.
        // `a.b()`, `a = 1` and `t[0] = 1` do not have this shape.
        _ if type_len(p, 0).is_some_and(|len| p.nth(len) == IDENT) => decls::variable_decl(p),
        _ => expr_stmt(p),
    }
}

/// `Return` or `Return expr`
fn return_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(RETURN_KW);
    if !p.at(NEWLINE) && !p.at(EOF) {
        exprs::expr(p);
    }
    eol(p);
    m.complete(p, RETURN_STMT);
}

/// A call, or an assignment.
fn expr_stmt(p: &mut Parser) {
    let m = p.start();
    if !exprs::expr(p) {
        // The error is already reported; progress must still be guaranteed.
        m.abandon(p);
        let e = p.start();
        p.bump_any();
        e.complete(p, ERROR_TREE);
        super::skip_line(p);
        p.eat(NEWLINE);
        return;
    }
    if p.at_ts(ASSIGN_OPS) {
        p.bump_any();
        exprs::expr(p);
        eol(p);
        m.complete(p, ASSIGN_STMT);
    } else {
        eol(p);
        m.complete(p, EXPR_STMT);
    }
}

/// `If … ElseIf … Else … EndIf`
fn if_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(IF_KW);
    exprs::expr(p);
    eol(p);
    stmt_list(p);

    while p.at(ELSEIF_KW) {
        let c = p.start();
        p.bump(ELSEIF_KW);
        exprs::expr(p);
        eol(p);
        stmt_list(p);
        c.complete(p, ELSEIF_CLAUSE);
    }
    if p.at(ELSE_KW) {
        let c = p.start();
        p.bump(ELSE_KW);
        eol(p);
        stmt_list(p);
        c.complete(p, ELSE_CLAUSE);
    }

    block_end(p, ENDIF_KW, "`EndIf`");
    m.complete(p, IF_STMT);
}

/// `While … EndWhile`
fn while_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(WHILE_KW);
    exprs::expr(p);
    eol(p);
    stmt_list(p);
    block_end(p, ENDWHILE_KW, "`EndWhile`");
    m.complete(p, WHILE_STMT);
}

/// `LockGuard g … EndLockGuard` — Starfield.
fn lock_guard_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(LOCKGUARD_KW);
    guard_list(p);
    eol(p);
    stmt_list(p);
    block_end(p, ENDLOCKGUARD_KW, "`EndLockGuard`");
    m.complete(p, LOCK_GUARD_STMT);
}

/// `TryLockGuard g … [Else …] EndTryLockGuard` — Starfield.
///
/// The alternative branch opens with a plain `Else` in vanilla
/// (`SQ_TraitsQuestScript.psc`). `ElseTryLockGuard`, which the ANTLR grammar
/// describes but no script uses, is accepted at the same place.
fn try_lock_guard_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(TRYLOCKGUARD_KW);
    guard_list(p);
    eol(p);
    stmt_list(p);
    if p.at(ELSE_KW) || p.at(ELSETRYLOCKGUARD_KW) {
        let c = p.start();
        p.bump_any();
        eol(p);
        stmt_list(p);
        c.complete(p, ELSE_TRY_LOCK_GUARD_CLAUSE);
    }
    block_end(p, ENDTRYLOCKGUARD_KW, "`EndTryLockGuard`");
    m.complete(p, TRY_LOCK_GUARD_STMT);
}

/// The locks of a `LockGuard`.
///
/// The vanilla corpus never takes more than one; the list costs a comma and
/// avoids a false error if a script takes several.
fn guard_list(p: &mut Parser) {
    let m = p.start();
    loop {
        exprs::expr(p);
        if !p.eat(COMMA) {
            break;
        }
    }
    m.complete(p, GUARD_LIST);
}
