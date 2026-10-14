//! What the compiler refuses in expressions and values: array sizes,
//! assignments to what cannot be assigned, initial values that are not
//! literals, integers beyond 32 bits, operands and casts of incompatible
//! types, values of calls that return nothing.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::{Diagnostic, Problem, child};
use crate::SourceFile;
use crate::items::{compact, trimmed};
use crate::semantics::Semantics;
use crate::ty::Ty;

/// The first significant token of `node` among its direct children.
fn operator(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE)
}

/// A literal, or a negated number: what an initial value may be.
fn is_literal(value: &SyntaxNode) -> bool {
    match value.kind() {
        SyntaxKind::LITERAL => true,
        SyntaxKind::UNARY_EXPR => {
            operator(value).is_some_and(|t| t.kind() == SyntaxKind::MINUS)
                && value.children().next().is_some_and(|inner| {
                    inner.kind() == SyntaxKind::LITERAL
                        && inner.first_token().is_some_and(|t| {
                            matches!(
                                t.kind(),
                                SyntaxKind::INT_NUMBER
                                    | SyntaxKind::HEX_NUMBER
                                    | SyntaxKind::FLOAT_NUMBER
                            )
                        })
                })
        }
        _ => false,
    }
}

/// The value after `=` in a declaration.
fn initial_value(decl: &SyntaxNode) -> Option<SyntaxNode> {
    decl.children_with_tokens()
        .skip_while(|e| e.kind() != SyntaxKind::EQ)
        .skip(1)
        .find_map(|e| e.into_node())
        .filter(|n| n.kind() != SyntaxKind::FLAG_LIST)
}

/// An integer literal's value: decimal or hexadecimal.
fn int_value(token: &SyntaxToken) -> Option<u128> {
    let text = token.text();
    match token.kind() {
        SyntaxKind::INT_NUMBER => text.parse().ok(),
        SyntaxKind::HEX_NUMBER => {
            let digits = text.get(2..)?;
            u128::from_str_radix(digits, 16).ok()
        }
        _ => None,
    }
}

/// Known, and not a number: what arithmetic refuses.
fn not_numeric(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Bool | Ty::String | Ty::None | Ty::Object(_) | Ty::Array(_)
    )
}

impl Semantics<'_> {
    fn mismatch_at(&self, node: &SyntaxNode, message: String, out: &mut Vec<Diagnostic>) {
        out.push(Diagnostic {
            problem: Problem::Mismatch,
            range: trimmed(node),
            message,
            fixes: Vec::new(),
        });
    }

    /// Initial values of script variables and properties, defaults of
    /// parameters: literals only. Integers beyond 32 bits, anywhere.
    pub(super) fn check_values(
        &self,
        root: &SyntaxNode,
        broken: &dyn Fn(&SyntaxNode) -> bool,
        out: &mut Vec<Diagnostic>,
    ) {
        for node in root.descendants() {
            let what = match node.kind() {
                SyntaxKind::VARIABLE_DECL
                    if node
                        .parent()
                        .is_some_and(|p| p.kind() == SyntaxKind::SOURCE_FILE) =>
                {
                    "the initial value of a script variable"
                }
                SyntaxKind::PROPERTY_DECL => "the initial value of a property",
                SyntaxKind::PARAM => "a default value",
                _ => continue,
            };
            if broken(&node) {
                continue;
            }
            let Some(value) = initial_value(&node) else {
                continue;
            };
            if !is_literal(&value) {
                out.push(Diagnostic {
                    problem: Problem::NotLiteral,
                    range: trimmed(&value),
                    message: format!(
                        "{what} must be a literal, like `0`, `-1.5`, `\"text\"` or `None`; \
                         compute it in `OnInit` instead"
                    ),
                    fixes: Vec::new(),
                });
            }
        }
        let fallout4 = self.since_fallout4();
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            let Some(value) = int_value(&token) else {
                continue;
            };
            let hex = token.kind() == SyntaxKind::HEX_NUMBER;
            let refused = if hex {
                value > u128::from(u32::MAX)
            } else {
                fallout4 && value > i32::MAX as u128
            };
            if refused {
                let limit = if hex { "0xFFFFFFFF" } else { "2147483647" };
                out.push(Diagnostic {
                    problem: Problem::IntRange,
                    range: token.text_range(),
                    message: format!("`{}` does not fit an `int`: at most {limit}", token.text()),
                    fixes: Vec::new(),
                });
            }
        }
    }

    /// The expression-level problems of a node of a body.
    pub(super) fn check_expr(
        &self,
        file: SourceFile,
        node: &SyntaxNode,
        out: &mut Vec<Diagnostic>,
    ) {
        match node.kind() {
            SyntaxKind::NEW_EXPR => self.check_array_size(node, out),
            SyntaxKind::ASSIGN_STMT => self.check_assignable(node, out),
            SyntaxKind::BIN_EXPR => self.check_operands(file, node, out),
            SyntaxKind::UNARY_EXPR => {
                let Some(inner) = node.children().next() else {
                    return;
                };
                if operator(node).is_some_and(|t| t.kind() == SyntaxKind::MINUS) {
                    let ty = self.type_of(file, &inner);
                    if not_numeric(&ty) {
                        self.mismatch_at(
                            node,
                            format!(
                                "`{}` is `{ty}`: only a number can be negated",
                                compact(&inner)
                            ),
                            out,
                        );
                    }
                }
            }
            SyntaxKind::CAST_EXPR => self.check_cast(file, node, out),
            SyntaxKind::IF_STMT | SyntaxKind::ELSEIF_CLAUSE | SyntaxKind::WHILE_STMT
                if self.since_fallout4() =>
            {
                if let Some(condition) = node.children().next() {
                    self.check_void(file, &condition, "a condition", out);
                }
            }
            _ => {}
        }
    }

    /// A call that returns nothing, used as a value.
    pub(super) fn check_void(
        &self,
        file: SourceFile,
        value: &SyntaxNode,
        what: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        if self.returns_nothing(file, value) == Some(true) {
            self.mismatch_at(
                value,
                format!("`{}` returns nothing: it cannot be {what}", compact(value)),
                out,
            );
        }
    }

    /// `new T[n]`: Skyrim wants a literal from 1 to 128, later games at most
    /// 128.
    fn check_array_size(&self, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if !node
            .children_with_tokens()
            .any(|e| e.kind() == SyntaxKind::L_BRACK)
        {
            return;
        }
        let Some(size) = node.children().find(|c| c.kind() != SyntaxKind::TYPE_REF) else {
            return;
        };
        let skyrim = self.skyrim();
        let literal = (size.kind() == SyntaxKind::LITERAL)
            .then(|| size.first_token().and_then(|t| int_value(&t)))
            .flatten();
        let message = match literal {
            Some(n) if n > 128 => Some(format!("an array holds at most 128 elements, not {n}")),
            Some(0) if skyrim => {
                Some("Skyrim's arrays hold from 1 to 128 elements, not 0".to_owned())
            }
            Some(_) => None,
            None if skyrim && self.game().is_some() => Some(format!(
                "Skyrim wants the size of an array as a literal from 1 to 128, not `{}`; \
                 SKSE's `Utility.CreateIntArray` and its siblings take a variable",
                compact(&size)
            )),
            None => None,
        };
        if let Some(message) = message {
            out.push(Diagnostic {
                problem: Problem::ArraySize,
                range: trimmed(&size),
                message,
                fixes: Vec::new(),
            });
        }
    }

    /// `F() = 1`, `a.Length = 2`.
    fn check_assignable(&self, stmt: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(target) = stmt.children().next() else {
            return;
        };
        let assignable = match target.kind() {
            SyntaxKind::NAME_REF | SyntaxKind::INDEX_EXPR => true,
            SyntaxKind::FIELD_EXPR => crate::semantics::member_name_ref(&target)
                .and_then(|m| m.first_token())
                .is_some_and(|t| t.kind() != SyntaxKind::LENGTH_KW),
            _ => false,
        };
        if assignable {
            return;
        }
        let what = if target.kind() == SyntaxKind::FIELD_EXPR {
            "`.Length` is read-only".to_owned()
        } else {
            format!("`{}` cannot be assigned", compact(&target))
        };
        out.push(Diagnostic {
            problem: Problem::NotAssignable,
            range: trimmed(&target),
            message: format!("{what}: only a variable, a property or an array element can"),
            fixes: Vec::new(),
        });
    }

    /// The operands of a binary expression.
    fn check_operands(&self, file: SourceFile, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        let Some(op) = operator(node) else {
            return;
        };
        let operands: Vec<SyntaxNode> = node.children().collect();
        let [left, right] = &operands[..] else {
            return;
        };
        let (a, b) = (self.type_of(file, left), self.type_of(file, right));
        let message = match op.kind() {
            SyntaxKind::PLUS if a == Ty::String || b == Ty::String => None,
            SyntaxKind::PLUS | SyntaxKind::MINUS | SyntaxKind::STAR | SyntaxKind::SLASH
                if (not_numeric(&a) && !matches!(b, Ty::Unknown | Ty::Var))
                    || (not_numeric(&b) && !matches!(a, Ty::Unknown | Ty::Var)) =>
            {
                let verb = match op.kind() {
                    SyntaxKind::PLUS => "add",
                    SyntaxKind::MINUS => "subtract",
                    SyntaxKind::STAR => "multiply",
                    _ => "divide",
                };
                Some(format!(
                    "cannot {verb} a `{a}` and a `{b}`: arithmetic takes numbers, and `+` strings"
                ))
            }
            SyntaxKind::PERCENT
                if (a == Ty::Float || not_numeric(&a) || b == Ty::Float || not_numeric(&b))
                    && !matches!(a, Ty::Unknown | Ty::Var)
                    && !matches!(b, Ty::Unknown | Ty::Var) =>
            {
                Some(format!("`%` takes integers, not a `{a}` and a `{b}`"))
            }
            SyntaxKind::EQ2 | SyntaxKind::NEQ
                if self.coerces(&a, &b) == Some(false) && self.coerces(&b, &a) == Some(false) =>
            {
                Some(format!(
                    "a `{a}` and a `{b}` cannot be compared: their types are unrelated"
                ))
            }
            SyntaxKind::LT | SyntaxKind::GT | SyntaxKind::LTEQ | SyntaxKind::GTEQ
                if a == Ty::Bool || b == Ty::Bool =>
            {
                Some("booleans cannot be ordered with `<` or `>`".to_owned())
            }
            _ => None,
        };
        if let Some(message) = message {
            self.mismatch_at(node, message, out);
        }
    }

    /// `x as T` between types no cast relates.
    fn check_cast(&self, file: SourceFile, node: &SyntaxNode, out: &mut Vec<Diagnostic>) {
        if node
            .children_with_tokens()
            .any(|e| e.kind() == SyntaxKind::IS_KW)
        {
            return;
        }
        let (Some(value), Some(target)) =
            (node.children().next(), child(node, SyntaxKind::TYPE_REF))
        else {
            return;
        };
        let from = self.type_of(file, &value);
        let to = self.qualify(file, Ty::parse(&compact(&target)));
        let refused = match (&from, &to) {
            (Ty::Object(_), Ty::Int | Ty::Float) | (Ty::Int | Ty::Float, Ty::Object(_)) => {
                self.coerces(&from, &to) == Some(false)
            }
            (Ty::Object(_), Ty::Object(_)) => {
                self.coerces(&from, &to) == Some(false) && self.coerces(&to, &from) == Some(false)
            }
            _ => false,
        };
        if refused {
            self.mismatch_at(
                node,
                format!("a `{from}` cannot be cast to a `{to}`: their types are unrelated"),
                out,
            );
        }
    }
}
