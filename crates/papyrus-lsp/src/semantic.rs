//! Semantic highlighting: what each identifier designates, from its place in
//! the tree.
//!
//! Without name resolution, a name in value position is a variable; a used
//! property or parameter is not told apart from a local.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use tower_lsp_server::ls_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};

use crate::position::LineMap;

/// The token types, in index order.
const TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::TYPE,
    SemanticTokenType::CLASS,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::METHOD,
    SemanticTokenType::EVENT,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::STRUCT,
];

#[derive(Debug, Clone, Copy)]
enum Kind {
    Keyword,
    Comment,
    String,
    Number,
    Type,
    Class,
    Function,
    Method,
    Event,
    Parameter,
    Variable,
    Property,
    Namespace,
    Struct,
}

const DECLARATION: u32 = 1 << 0;
const DOCUMENTATION: u32 = 1 << 1;

/// The legend announced to the client.
pub(crate) fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TYPES.to_vec(),
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DOCUMENTATION,
        ],
    }
}

/// The significant token following `node`.
fn next_significant(node: &SyntaxNode) -> Option<SyntaxKind> {
    std::iter::successors(node.next_sibling_or_token(), |e| e.next_sibling_or_token())
        .map(|e| e.kind())
        .find(|k| !k.is_trivia())
}

/// What a name in use position designates.
fn usage(name_ref: &SyntaxNode) -> Kind {
    let Some(parent) = name_ref.parent() else {
        return Kind::Variable;
    };
    let first = parent.first_child().as_ref() == Some(name_ref);
    match parent.kind() {
        SyntaxKind::CALL_EXPR if first => Kind::Function,
        // `x.Foo` or `x.Foo()`
        SyntaxKind::FIELD_EXPR if !first => {
            let called = parent.parent().is_some_and(|p| {
                p.kind() == SyntaxKind::CALL_EXPR && p.first_child().as_ref() == Some(&parent)
            });
            if called { Kind::Method } else { Kind::Property }
        }
        SyntaxKind::ARG if next_significant(name_ref) == Some(SyntaxKind::EQ) => Kind::Parameter,
        _ => Kind::Variable,
    }
}

/// What a name declares.
fn declaration(name: &SyntaxNode) -> Option<Kind> {
    Some(match name.parent()?.kind() {
        SyntaxKind::SCRIPT_HEADER => Kind::Class,
        SyntaxKind::FUNCTION_DECL => Kind::Function,
        SyntaxKind::EVENT_DECL | SyntaxKind::CUSTOM_EVENT_DECL => Kind::Event,
        SyntaxKind::PARAM => Kind::Parameter,
        SyntaxKind::VARIABLE_DECL | SyntaxKind::GUARD_DECL => Kind::Variable,
        SyntaxKind::PROPERTY_DECL | SyntaxKind::STRUCT_FIELD => Kind::Property,
        SyntaxKind::STATE_DECL | SyntaxKind::GROUP_DECL => Kind::Namespace,
        SyntaxKind::STRUCT_DECL => Kind::Struct,
        _ => return None,
    })
}

fn classify(token: &SyntaxToken) -> Option<(Kind, u32)> {
    let kind = token.kind();
    match kind {
        SyntaxKind::LINE_COMMENT | SyntaxKind::BLOCK_COMMENT => return Some((Kind::Comment, 0)),
        SyntaxKind::DOC_COMMENT => return Some((Kind::Comment, DOCUMENTATION)),
        SyntaxKind::STRING => return Some((Kind::String, 0)),
        SyntaxKind::INT_NUMBER | SyntaxKind::HEX_NUMBER | SyntaxKind::FLOAT_NUMBER => {
            return Some((Kind::Number, 0));
        }
        _ => {}
    }
    if kind != SyntaxKind::IDENT && !kind.is_keyword() {
        return None;
    }
    // A keyword can serve as a name: `Auto State Default`.
    let parent = token.parent()?;
    match parent.kind() {
        SyntaxKind::NAME => declaration(&parent).map(|k| (k, DECLARATION)),
        SyntaxKind::TYPE_REF => Some((Kind::Type, 0)),
        SyntaxKind::NAME_REF if kind.is_keyword() => Some((Kind::Keyword, 0)), // self, parent
        SyntaxKind::NAME_REF => Some((usage(&parent), 0)),
        _ if kind.is_keyword() => Some((Kind::Keyword, 0)),
        _ => None,
    }
}

/// The tokens of `root`, encoded as LSP expects: each relative to the
/// previous one, and split at every line end.
pub(crate) fn tokens(root: &SyntaxNode, map: &LineMap) -> Vec<SemanticToken> {
    let mut out = Vec::new();
    let (mut last_line, mut last_start) = (0, 0);
    let mut push = |range: TextRange, kind: Kind, modifiers: u32| {
        let start = map.position(range.start());
        let end = map.position(range.end());
        if end.line != start.line || end.character <= start.character {
            return;
        }
        let delta_line = start.line - last_line;
        let delta_start = if delta_line == 0 {
            start.character - last_start
        } else {
            start.character
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: end.character - start.character,
            token_type: kind as u32,
            token_modifiers_bitset: modifiers,
        });
        (last_line, last_start) = (start.line, start.character);
    };

    for token in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let Some((kind, modifiers)) = classify(&token) else {
            continue;
        };
        let range = token.text_range();
        let (first, last) = (map.line(range.start()), map.line(range.end()));
        if first == last {
            push(range, kind, modifiers);
            continue;
        }
        // A comment spanning several lines: one piece per line.
        for line in first..=last {
            let start = map
                .line_start(line)
                .unwrap_or(range.start())
                .max(range.start());
            let end = map.line_end(line).unwrap_or(range.end()).min(range.end());
            if start < end {
                push(TextRange::new(start, end), kind, modifiers);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Columns;
    use papyrus_dialect::{Dialect, GameId};

    /// `(text, type, modifiers)` of each token, positions decoded.
    fn decode(text: &str) -> Vec<(String, &'static str, u32)> {
        let parse = papyrus_syntax::parse(text, Dialect::new(GameId::Fallout4));
        let map = LineMap::new(text, Columns::Utf8);
        let lines: Vec<&str> = text.split('\n').collect();
        let (mut line, mut start) = (0, 0);
        tokens(&parse.syntax(), &map)
            .into_iter()
            .map(|t| {
                if t.delta_line > 0 {
                    line += t.delta_line;
                    start = 0;
                }
                start += t.delta_start;
                let s = start as usize;
                let word = lines[line as usize][s..s + t.length as usize].to_owned();
                (
                    word,
                    TYPES[t.token_type as usize].as_str(),
                    t.token_modifiers_bitset,
                )
            })
            .collect()
    }

    #[test]
    fn each_name_is_classified_by_its_place() {
        let text = "\
ScriptName Foo extends ObjectReference
{ doc
on two lines }
Actor Property target Auto
Function Run(int aiCount) ; note
  target.Kill(abForce = true)
  Run(aiCount)
EndFunction
";
        let found = decode(text);
        let expect = |word: &str| {
            found
                .iter()
                .find(|(w, _, _)| w == word)
                .map(|(_, kind, modifiers)| (*kind, *modifiers))
        };
        assert_eq!(expect("ScriptName"), Some(("keyword", 0)));
        assert_eq!(expect("Foo"), Some(("class", DECLARATION)));
        assert_eq!(expect("ObjectReference"), Some(("type", 0)));
        assert_eq!(expect("{ doc"), Some(("comment", DOCUMENTATION)));
        assert_eq!(expect("on two lines }"), Some(("comment", DOCUMENTATION)));
        assert_eq!(expect("Actor"), Some(("type", 0)));
        assert_eq!(expect("target"), Some(("property", DECLARATION)));
        assert_eq!(expect("Run"), Some(("function", DECLARATION)));
        assert_eq!(expect("int"), Some(("type", 0)));
        assert_eq!(expect("aiCount"), Some(("parameter", DECLARATION)));
        assert_eq!(expect("; note"), Some(("comment", 0)));
        assert_eq!(expect("Kill"), Some(("method", 0)));
        assert_eq!(expect("abForce"), Some(("parameter", 0)));
        assert_eq!(expect("true"), Some(("keyword", 0)));
        // The uses: the variable `target`, the call `Run`.
        let uses: Vec<_> = found
            .iter()
            .filter(|(w, _, m)| (w == "target" || w == "Run") && *m == 0)
            .map(|(w, k, _)| (w.as_str(), *k))
            .collect();
        assert_eq!(uses, [("target", "variable"), ("Run", "function")]);
    }
}
