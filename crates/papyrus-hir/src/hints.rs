//! What an editor shows while a call is typed: the signature of the callee
//! with the current parameter, and the names of the parameters before the
//! arguments.

use papyrus_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};

use crate::SourceFile;
use crate::calls::{Signature, args, callee};
use crate::items::{compact, trimmed};
use crate::semantics::Semantics;

/// The call around the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    /// What is called.
    pub signature: Signature,
    /// The parameter the cursor is on, if the callee has it.
    pub active: Option<usize>,
}

/// Which arguments get the name of their parameter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParameterHints {
    /// None.
    None,
    /// Literals only: `Disable(abFadeOut: true)`, where the value alone says
    /// nothing.
    #[default]
    Literals,
    /// Every positional argument, unless it already carries the name.
    All,
}

/// A name shown before an argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// Where: the start of the argument.
    pub offset: TextSize,
    /// The parameter's name.
    pub label: String,
}

/// True for a value that says nothing by itself: `true`, `-1`, `"x"`,
/// `None`.
fn literal(value: &SyntaxNode) -> bool {
    match value.kind() {
        SyntaxKind::LITERAL => true,
        SyntaxKind::UNARY_EXPR => value.children().next().is_some_and(|v| literal(&v)),
        _ => false,
    }
}

/// The name without its Hungarian prefix: `akActor` gives `actor`.
fn bare(name: &str) -> String {
    let lower = name.trim_start_matches(|c: char| c.is_ascii_lowercase());
    if lower.is_empty() { name } else { lower }.to_ascii_lowercase()
}

/// True if `value` already says which parameter it fills: `akActor` or
/// The index of the argument of `list` at `offset`: the commas before it.
pub(crate) fn arg_index(list: &SyntaxNode, offset: TextSize) -> usize {
    list.children_with_tokens()
        .filter(|e| e.kind() == SyntaxKind::COMMA && e.text_range().end() <= offset)
        .count()
}

/// `self.actor` for `akActor`.
fn named_like(value: &SyntaxNode, param: &str) -> bool {
    let last = match value.kind() {
        SyntaxKind::NAME_REF => compact(value),
        SyntaxKind::FIELD_EXPR => value
            .children()
            .last()
            .map(|n| compact(&n))
            .unwrap_or_default(),
        _ => return false,
    };
    let last = last.to_ascii_lowercase();
    last == param.to_ascii_lowercase() || last.ends_with(&bare(param))
}

impl Semantics<'_> {
    /// The signature of the call whose arguments hold `offset`.
    pub fn signature_help(&self, file: SourceFile, offset: TextSize) -> Option<SignatureHelp> {
        let (list, signature) = self.call_at(file, offset)?;
        let index = arg_index(&list, offset);
        // A named argument is on its own parameter.
        let named = args(&list)
            .into_iter()
            .nth(index)
            .and_then(|a| a.name)
            .map(|n| signature.param(&compact(&n)));
        let active = match named {
            Some(param) => param,
            None => (index < signature.params.len()).then_some(index),
        };
        Some(SignatureHelp { signature, active })
    }

    /// The argument list holding `offset`, and the signature of its call.
    pub(crate) fn call_at(
        &self,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<(SyntaxNode, Signature)> {
        let root = self.root(file);
        let mut token = root.token_at_offset(offset).left_biased()?;
        // After `Foo(1, `, the space may lie outside the unclosed list.
        while token.kind().is_trivia() || token.kind() == SyntaxKind::NEWLINE {
            token = token.prev_token()?;
        }
        // The innermost argument list the cursor is inside of.
        let list = token.parent_ancestors().find(|n| {
            if n.kind() != SyntaxKind::ARG_LIST {
                return false;
            }
            let bound = |kind| {
                n.children_with_tokens()
                    .find(|e| e.kind() == kind)
                    .map(|e| e.text_range())
            };
            let opened = bound(SyntaxKind::L_PAREN).is_some_and(|p| p.end() <= offset);
            let open = bound(SyntaxKind::R_PAREN).is_none_or(|p| offset <= p.start());
            opened && open
        })?;
        let call = list
            .parent()
            .filter(|p| p.kind() == SyntaxKind::CALL_EXPR)?;
        let def = self.resolve_name_ref(file, &callee(&call)?)?;
        let signature = self.signature(&def)?;
        Some((list, signature))
    }

    /// The parameter names to show before the arguments of the calls in
    /// `range`.
    pub fn parameter_hints(
        &self,
        file: SourceFile,
        range: TextRange,
        mode: ParameterHints,
    ) -> Vec<InlayHint> {
        if mode == ParameterHints::None {
            return Vec::new();
        }
        let mut out = Vec::new();
        let root = self.root(file);
        for call in root.descendants() {
            if call.kind() != SyntaxKind::CALL_EXPR || call.text_range().intersect(range).is_none()
            {
                continue;
            }
            let Some(list) = call.children().find(|c| c.kind() == SyntaxKind::ARG_LIST) else {
                continue;
            };
            let Some(signature) = callee(&call)
                .and_then(|c| self.resolve_name_ref(file, &c))
                .and_then(|def| self.signature(&def))
            else {
                continue;
            };
            for (arg, param) in args(&list).iter().zip(&signature.params) {
                // Named arguments already say it; positional ones stop there.
                if arg.name.is_some() {
                    break;
                }
                let Some(value) = &arg.value else {
                    continue;
                };
                let shown = match mode {
                    ParameterHints::Literals => literal(value),
                    _ => !named_like(value, &param.name),
                };
                if shown {
                    out.push(InlayHint {
                        offset: trimmed(value).start(),
                        label: param.name.clone(),
                    });
                }
            }
        }
        out
    }
}
