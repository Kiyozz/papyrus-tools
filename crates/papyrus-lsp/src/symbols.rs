//! Le plan du document et ses zones repliables.

use papyrus_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use tower_lsp_server::ls_types::{DocumentSymbol, FoldingRange, FoldingRangeKind, SymbolKind};

use crate::position::LineMap;

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// Le texte significatif d'un nœud, espaces réduits à un seul.
fn text(node: &SyntaxNode) -> String {
    node.text()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// L'étendue d'un nœud sans la trivia ni la fin de ligne qui le bordent.
fn trimmed(node: &SyntaxNode) -> TextRange {
    let significant = |t: &SyntaxToken| !t.kind().is_trivia() && t.kind() != SyntaxKind::NEWLINE;
    let tokens = || {
        node.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(significant)
    };
    match (tokens().next(), tokens().last()) {
        (Some(first), Some(last)) => first.text_range().cover(last.text_range()),
        _ => node.text_range(),
    }
}

/// Le genre de symbole d'une déclaration, et le détail à afficher.
fn describe(node: &SyntaxNode) -> Option<(SymbolKind, Option<String>)> {
    let type_ref = || child(node, SyntaxKind::TYPE_REF).map(|t| text(&t));
    Some(match node.kind() {
        SyntaxKind::FUNCTION_DECL => {
            let params = child(node, SyntaxKind::PARAM_LIST).map(|p| text(&p));
            let detail = match (type_ref(), params) {
                (Some(ret), Some(params)) => format!("{params} → {ret}"),
                (None, Some(params)) => params,
                (ret, None) => ret.unwrap_or_default(),
            };
            (SymbolKind::FUNCTION, Some(detail))
        }
        SyntaxKind::EVENT_DECL => (
            SymbolKind::EVENT,
            child(node, SyntaxKind::PARAM_LIST).map(|p| text(&p)),
        ),
        SyntaxKind::PROPERTY_DECL => (SymbolKind::PROPERTY, type_ref()),
        SyntaxKind::VARIABLE_DECL => (SymbolKind::VARIABLE, type_ref()),
        SyntaxKind::STATE_DECL => (SymbolKind::NAMESPACE, Some("State".to_owned())),
        SyntaxKind::STRUCT_DECL => (SymbolKind::STRUCT, None),
        SyntaxKind::STRUCT_FIELD => (SymbolKind::FIELD, type_ref()),
        SyntaxKind::GROUP_DECL => (SymbolKind::PACKAGE, Some("Group".to_owned())),
        SyntaxKind::CUSTOM_EVENT_DECL => (SymbolKind::EVENT, Some("CustomEvent".to_owned())),
        SyntaxKind::GUARD_DECL => (SymbolKind::VARIABLE, Some("Guard".to_owned())),
        _ => return None,
    })
}

#[allow(deprecated)] // `DocumentSymbol::deprecated`, que le type impose.
fn symbol(node: &SyntaxNode, map: &LineMap) -> Option<DocumentSymbol> {
    let (kind, detail) = describe(node)?;
    let name_node = child(node, SyntaxKind::NAME)?;
    let mut name = text(&name_node);
    // `Event Actor.OnDeath` : le script écouté fait partie du nom.
    if node.kind() == SyntaxKind::EVENT_DECL {
        if let Some(remote) = child(node, SyntaxKind::TYPE_REF) {
            name = format!("{}.{name}", text(&remote));
        }
    }
    if name.is_empty() {
        return None;
    }
    let children = members(node, map);
    Some(DocumentSymbol {
        name,
        detail: detail.filter(|d| !d.is_empty()),
        kind,
        tags: None,
        deprecated: None,
        range: map.range(trimmed(node)),
        selection_range: map.range(trimmed(&name_node)),
        children: (!children.is_empty()).then_some(children),
    })
}

/// Les déclarations contenues dans `node` : celles d'un state, d'une
/// structure, d'un groupe. Le corps des fonctions n'en fait pas partie.
fn members(node: &SyntaxNode, map: &LineMap) -> Vec<DocumentSymbol> {
    if matches!(
        node.kind(),
        SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL | SyntaxKind::PROPERTY_DECL
    ) {
        return Vec::new();
    }
    node.children().filter_map(|c| symbol(&c, map)).collect()
}

/// Le plan du document : le script, et ses déclarations dedans.
#[allow(deprecated)]
pub(crate) fn document_symbols(root: &SyntaxNode, map: &LineMap) -> Vec<DocumentSymbol> {
    let items = members(root, map);
    let Some(header) = child(root, SyntaxKind::SCRIPT_HEADER) else {
        return items;
    };
    let Some(name) = child(&header, SyntaxKind::NAME) else {
        return items;
    };
    let parent = child(&header, SyntaxKind::EXTENDS_CLAUSE)
        .and_then(|e| child(&e, SyntaxKind::TYPE_REF))
        .map(|t| format!("extends {}", text(&t)));
    vec![DocumentSymbol {
        name: text(&name),
        detail: parent,
        kind: SymbolKind::CLASS,
        tags: None,
        deprecated: None,
        range: map.range(trimmed(root)),
        selection_range: map.range(trimmed(&name)),
        children: (!items.is_empty()).then_some(items),
    }]
}

// ── Repli ─────────────────────────────────────────────────────────────────

fn fold(
    map: &LineMap,
    start: u32,
    end: u32,
    kind: Option<FoldingRangeKind>,
) -> Option<FoldingRange> {
    (end > start)
        .then(|| FoldingRange {
            start_line: start,
            end_line: end,
            kind,
            ..FoldingRange::default()
        })
        .filter(|_| map.line_start(end).is_some())
}

/// Les zones repliables : les blocs, jusqu'à la ligne qui précède leur mot
/// de fin, chaque branche d'un `If` à part ; les commentaires sur plusieurs
/// lignes ; les suites d'`Import`.
pub(crate) fn folding_ranges(root: &SyntaxNode, map: &LineMap) -> Vec<FoldingRange> {
    let mut out = Vec::new();
    for node in root.descendants() {
        let block = matches!(
            node.kind(),
            SyntaxKind::FUNCTION_DECL
                | SyntaxKind::EVENT_DECL
                | SyntaxKind::STATE_DECL
                | SyntaxKind::STRUCT_DECL
                | SyntaxKind::GROUP_DECL
                | SyntaxKind::PROPERTY_DECL
                | SyntaxKind::IF_STMT
                | SyntaxKind::ELSEIF_CLAUSE
                | SyntaxKind::ELSE_CLAUSE
                | SyntaxKind::WHILE_STMT
                | SyntaxKind::LOCK_GUARD_STMT
                | SyntaxKind::TRY_LOCK_GUARD_STMT
                | SyntaxKind::ELSE_TRY_LOCK_GUARD_CLAUSE
        );
        if !block {
            continue;
        }
        let range = trimmed(&node);
        let start = map.line(range.start());
        // Une branche s'arrête où commence la suivante ; un bloc, à son mot
        // de fin, qui reste visible.
        let next_clause = node.children().find(|c| {
            matches!(
                c.kind(),
                SyntaxKind::ELSEIF_CLAUSE
                    | SyntaxKind::ELSE_CLAUSE
                    | SyntaxKind::ELSE_TRY_LOCK_GUARD_CLAUSE
            )
        });
        let stop = match next_clause {
            Some(clause) => map.line(trimmed(&clause).start()),
            None if matches!(
                node.kind(),
                SyntaxKind::ELSEIF_CLAUSE
                    | SyntaxKind::ELSE_CLAUSE
                    | SyntaxKind::ELSE_TRY_LOCK_GUARD_CLAUSE
            ) =>
            {
                // La clause s'arrête avant la suivante, ou avant le mot de
                // fin du bloc parent.
                let next = std::iter::successors(node.next_sibling(), |n| n.next_sibling())
                    .next()
                    .map(|n| trimmed(&n).start());
                let end = next.or_else(|| node.parent().map(|p| trimmed(&p).end()));
                match end {
                    Some(end) => map.line(end),
                    None => continue,
                }
            }
            None => map.line(range.end()),
        };
        out.extend(fold(map, start, stop.saturating_sub(1), None));
    }

    // Commentaires : un bloc ou une docstring sur plusieurs lignes, ou une
    // suite de commentaires de ligne.
    let mut run: Option<(u32, u32)> = None;
    let mut imports: Option<(u32, u32)> = None;
    for element in root.descendants_with_tokens() {
        let Some(token) = element.as_token() else {
            if let Some(node) = element.as_node() {
                if node.kind() == SyntaxKind::IMPORT {
                    let line = map.line(trimmed(node).start());
                    imports = match imports {
                        Some((first, last)) if line == last + 1 => Some((first, line)),
                        other => {
                            if let Some((first, last)) = other {
                                out.extend(fold(map, first, last, Some(FoldingRangeKind::Imports)));
                            }
                            Some((line, line))
                        }
                    };
                }
            }
            continue;
        };
        match token.kind() {
            SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT => {
                let range = token.text_range();
                out.extend(fold(
                    map,
                    map.line(range.start()),
                    map.line(range.end()),
                    Some(FoldingRangeKind::Comment),
                ));
            }
            SyntaxKind::LINE_COMMENT => {
                let line = map.line(token.text_range().start());
                run = match run {
                    Some((first, last)) if line == last + 1 => Some((first, line)),
                    other => {
                        if let Some((first, last)) = other {
                            out.extend(fold(map, first, last, Some(FoldingRangeKind::Comment)));
                        }
                        Some((line, line))
                    }
                };
            }
            _ => {}
        }
    }
    if let Some((first, last)) = run {
        out.extend(fold(map, first, last, Some(FoldingRangeKind::Comment)));
    }
    if let Some((first, last)) = imports {
        out.extend(fold(map, first, last, Some(FoldingRangeKind::Imports)));
    }
    out.sort_by_key(|f| (f.start_line, f.end_line));
    out.dedup_by_key(|f| (f.start_line, f.end_line));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Columns;
    use papyrus_dialect::{Dialect, GameId};

    fn parse(text: &str) -> (SyntaxNode, LineMap) {
        let parse = papyrus_syntax::parse(text, Dialect::new(GameId::Fallout4));
        assert!(parse.ok(), "{:?}", parse.errors());
        (parse.syntax(), LineMap::new(text, Columns::Utf16))
    }

    const SOURCE: &str = "\
ScriptName Foo extends ObjectReference
Import Utility
Import Debug
int Property count = 0 Auto
Function DoThing(int aiCount = 1) global
  If aiCount
    aiCount += 1
  ElseIf aiCount > 2
    aiCount -= 1
  Else
    aiCount = 0
  EndIf
EndFunction
; un
; deux
State Busy
  Event OnActivate(ObjectReference akActionRef)
  EndEvent
EndState
Event Actor.OnDeath(Actor akSender, Actor akKiller)
EndEvent
";

    #[test]
    fn le_plan_suit_les_declarations() {
        let (root, map) = parse(SOURCE);
        let symbols = document_symbols(&root, &map);
        assert_eq!(symbols.len(), 1);
        let script = &symbols[0];
        assert_eq!(script.name, "Foo");
        assert_eq!(script.detail.as_deref(), Some("extends ObjectReference"));
        let items: Vec<(&str, SymbolKind)> = script
            .children
            .iter()
            .flatten()
            .map(|s| (s.name.as_str(), s.kind))
            .collect();
        assert_eq!(
            items,
            [
                ("count", SymbolKind::PROPERTY),
                ("DoThing", SymbolKind::FUNCTION),
                ("Busy", SymbolKind::NAMESPACE),
                ("Actor.OnDeath", SymbolKind::EVENT),
            ]
        );
        let function = &script.children.as_ref().unwrap()[1];
        assert_eq!(function.detail.as_deref(), Some("(int aiCount = 1)"));
        assert_eq!(function.range.start.line, 4);
        assert_eq!(function.range.end.line, 12);
        let state = &script.children.as_ref().unwrap()[2];
        assert_eq!(state.children.as_ref().unwrap()[0].name, "OnActivate");
    }

    #[test]
    fn chaque_bloc_se_replie_jusqu_a_son_mot_de_fin() {
        let (root, map) = parse(SOURCE);
        let folds: Vec<(u32, u32)> = folding_ranges(&root, &map)
            .iter()
            .map(|f| (f.start_line, f.end_line))
            .collect();
        assert_eq!(
            folds,
            [
                (1, 2),   // Import
                (4, 11),  // Function
                (5, 6),   // If
                (7, 8),   // ElseIf
                (9, 10),  // Else
                (13, 14), // ; un ; deux
                (15, 17), // State ; un événement vide n'a rien à replier
            ]
        );
    }
}
