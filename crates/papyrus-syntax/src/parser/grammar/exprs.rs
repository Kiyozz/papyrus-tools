//! Les expressions.
//!
//! Précédences, de la plus lâche à la plus serrée, d'après la grammaire de
//! référence du Creation Kit :
//!
//! | Niveau | Opérateurs                   |
//! |--------|------------------------------|
//! | 1      | `\|\|`                       |
//! | 2      | `&&`                         |
//! | 3      | `==` `!=` `<` `>` `<=` `>=`  |
//! | 4      | `+` `-`                      |
//! | 5      | `*` `/` `%`                  |
//! | unaire | `-` `!`                      |
//! | cast   | `as`, `is`                   |
//! | postfixe | `.membre`, `[i]`, `(args)` |
//!
//! Tous les binaires sont associatifs à gauche. L'unaire enveloppe le cast :
//! `-x as Float` se lit `-(x as Float)`.
//!
//! Une expression ne franchit jamais une fin de ligne : `NEWLINE` n'est ni un
//! opérateur ni le début d'un opérande. Seule la continuation `\`, qui est de
//! la trivia, permet de l'étendre sur plusieurs lignes.

use super::{Parser, namespace_tail, type_ref, type_ref_without_array};
use crate::SyntaxKind::*;
use crate::parser::CompletedMarker;

/// Lit une expression. Faux si aucune ne commence ici — l'erreur est alors
/// signalée et rien n'a été consommé.
pub(super) fn expr(p: &mut Parser) -> bool {
    expr_bp(p, 0).is_some()
}

/// Force de liaison d'un opérateur binaire, `None` pour tout autre token.
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

/// Escalade de précédence.
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

/// `x as Actor`, `x is Actor` — le second à partir de Fallout 4, ce que la
/// promotion des mots-clés règle déjà.
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

/// Le nom après un point.
///
/// Les mots réservés y sont acceptés : le corpus vanilla écrit `.Length`, et
/// des membres nommés comme des mots-clés. Le risque est nul, puisque la fin
/// de ligne n'est pas un mot-clé et arrête la lecture.
fn member(p: &mut Parser) {
    let kind = p.current();
    if kind == IDENT || kind.is_keyword() {
        let m = p.start();
        p.bump_any();
        m.complete(p, NAME_REF);
    } else {
        p.error("nom de membre attendu");
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
            // Nom qualifié : `DLC04:ManagerScript.GetScript()` appelle une
            // fonction globale d'un script rangé dans un namespace.
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
            // `new float[4]` : les crochets portent la taille, pas le type.
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
            p.error("expression attendue");
            return None;
        }
    };
    Some(m)
}

/// `(a, abForce = true)`, aussi employé par `RequiresGuard(g)`.
pub(super) fn arg_list(p: &mut Parser) {
    let m = p.start();
    p.bump(L_PAREN);
    while !p.at(R_PAREN) && !p.at(NEWLINE) && !p.at(EOF) {
        let a = p.start();
        // Argument nommé, admis dès Skyrim : `Activate(akRef, abDefault = true)`.
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
