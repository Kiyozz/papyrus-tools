//! Les instructions, dans les corps de fonction et d'événement.

use super::{Parser, STMT_RECOVERY, TokenSet, block_end, decls, eol, exprs, type_len};
use crate::SyntaxKind::*;

/// Opérateurs d'affectation, simples et composés.
const ASSIGN_OPS: TokenSet = TokenSet::new(&[EQ, PLUS_EQ, MINUS_EQ, STAR_EQ, SLASH_EQ, PERCENT_EQ]);

/// Une suite d'instructions, jusqu'au premier mot de [`STMT_RECOVERY`].
///
/// C'est à l'appelant de consommer le terminateur qu'il attend ; s'il trouve
/// autre chose, l'erreur est la sienne, et la liste n'a rien avalé de trop.
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

/// Une instruction. Consomme toujours au moins un token.
fn stmt(p: &mut Parser) {
    match p.current() {
        RETURN_KW => return_stmt(p),
        IF_KW => if_stmt(p),
        WHILE_KW => while_stmt(p),
        LOCKGUARD_KW => lock_guard_stmt(p),
        TRYLOCKGUARD_KW => try_lock_guard_stmt(p),
        // `Actor a`, `int[] t`, `Foo:Bar b` — un type suivi d'un nom. `a.b()`,
        // `a = 1` et `t[0] = 1` n'ont pas cette forme.
        _ if type_len(p, 0).is_some_and(|len| p.nth(len) == IDENT) => decls::variable_decl(p),
        _ => expr_stmt(p),
    }
}

/// `Return` ou `Return expr`
fn return_stmt(p: &mut Parser) {
    let m = p.start();
    p.bump(RETURN_KW);
    if !p.at(NEWLINE) && !p.at(EOF) {
        exprs::expr(p);
    }
    eol(p);
    m.complete(p, RETURN_STMT);
}

/// Un appel, ou une affectation.
fn expr_stmt(p: &mut Parser) {
    let m = p.start();
    if !exprs::expr(p) {
        // L'erreur est déjà signalée ; reste à garantir la progression.
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
/// La branche alternative s'ouvre sur un simple `Else` dans le vanilla
/// (`SQ_TraitsQuestScript.psc`). `ElseTryLockGuard`, que décrit la grammaire
/// ANTLR mais qu'aucun script n'emploie, est accepté au même endroit.
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

/// Les verrous d'un `LockGuard`.
///
/// Le corpus vanilla n'en prend jamais qu'un ; la liste coûte une virgule et
/// évite une fausse erreur si un script en prend plusieurs.
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
