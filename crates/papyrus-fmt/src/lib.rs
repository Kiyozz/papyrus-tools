//! The canonical Papyrus formatter.
//!
//! One style, few options: the goal is to stop arguing about it, as with
//! `gofmt`. The formatter works on the concrete syntax tree, line by line —
//! Papyrus is line-oriented, and the formatter never has to decide where to
//! break.
//!
//! # What it does
//!
//! - reindents according to nesting;
//! - normalizes spaces within the line;
//! - writes keywords in lowercase, or in PascalCase with
//!   [`KeywordCase::Pascal`], except when they serve as names (`Auto State
//!   Default`, `.Length`), and `True`, `False`, `None` as a parameter's
//!   default value, which the compiler compares as text with the one of the
//!   function overridden;
//! - drops the parentheses around a whole `If`, `ElseIf` or `While`
//!   condition;
//! - keeps at most one blank line, none before a word that closes a block,
//!   and sets functions, events, states, structs, groups and full properties
//!   apart with one;
//! - strips trailing spaces, writes LF line endings and ends the file with
//!   exactly one.
//!
//! # What it never does
//!
//! - touch identifiers: Papyrus is case-insensitive, but renaming is the
//!   linter's business, which knows what is safe;
//! - touch the contents of strings and comments;
//! - change the encoding: an existing BOM is kept, none is added;
//! - format a file that does not parse.
//!
//! # Safeguard
//!
//! Before returning its result, the formatter checks that the sequence of
//! significant tokens is unchanged, up to keyword case and the parentheses of
//! conditions. At the slightest
//! difference it refuses: better not to format than to corrupt a script.

#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;

use papyrus_dialect::Dialect;
use papyrus_syntax::{ParseError, SyntaxKind, SyntaxNode, SyntaxToken, parse};

use SyntaxKind::*;

/// An indentation unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// This many spaces per level.
    Spaces(u8),
    /// One tab per level.
    Tabs,
}

impl Default for Indent {
    fn default() -> Indent {
        Indent::Spaces(2)
    }
}

/// How keywords are written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeywordCase {
    /// `endfunction`, the default.
    #[default]
    Lower,
    /// `EndFunction`, as the Creation Kit writes them.
    Pascal,
}

/// The formatter settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatOptions {
    /// The indentation, two spaces by default.
    pub indent: Indent,
    /// The case of keywords, lowercase by default.
    pub keywords: KeywordCase,
}

/// Keywords in PascalCase, as the Creation Kit writes them.
const PASCAL: &[&str] = &[
    "As",
    "Auto",
    "AutoReadOnly",
    "BetaOnly",
    "Bool",
    "Collapsed",
    "CollapsedOnBase",
    "CollapsedOnRef",
    "Conditional",
    "Const",
    "CustomEvent",
    "CustomEventName",
    "DebugOnly",
    "Default",
    "Else",
    "ElseIf",
    "ElseTryLockGuard",
    "EndEvent",
    "EndFunction",
    "EndGroup",
    "EndIf",
    "EndLockGuard",
    "EndProperty",
    "EndState",
    "EndStruct",
    "EndTryLockGuard",
    "EndWhile",
    "Event",
    "Extends",
    "False",
    "Float",
    "Function",
    "Global",
    "Group",
    "Guard",
    "Hidden",
    "If",
    "Import",
    "Int",
    "Is",
    "Length",
    "LockGuard",
    "Mandatory",
    "Native",
    "New",
    "None",
    "Parent",
    "Private",
    "Property",
    "Protected",
    "ProtectsFunctionLogic",
    "RequiresGuard",
    "Return",
    "ScriptEventName",
    "ScriptName",
    "Self",
    "SelfOnly",
    "State",
    "String",
    "Struct",
    "StructVarName",
    "True",
    "TryLockGuard",
    "Var",
    "While",
];

/// The PascalCase spelling of a keyword.
fn pascal(keyword: &str) -> Option<&'static str> {
    PASCAL
        .iter()
        .find(|p| p.eq_ignore_ascii_case(keyword))
        .copied()
}

/// Why a file was not formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// The source does not parse: formatting it could damage it.
    Syntax(Vec<ParseError>),
    /// The result does not contain the same tokens as the source. This is a
    /// formatter bug; the file is left untouched.
    Unsafe {
        /// What differs, for the bug report.
        detail: String,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Syntax(errors) => {
                write!(f, "{} syntax error(s), file not formatted", errors.len())
            }
            FormatError::Unsafe { detail } => {
                write!(
                    f,
                    "formatting refused, the result would alter the code: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Formats `source`, read for the `dialect` target.
pub fn format(
    source: &str,
    dialect: Dialect,
    options: &FormatOptions,
) -> Result<String, FormatError> {
    let parse = parse(source, dialect);
    if !parse.ok() {
        return Err(FormatError::Syntax(parse.errors().to_vec()));
    }

    let mut out = String::with_capacity(source.len());
    if source.starts_with('\u{feff}') {
        out.push('\u{feff}');
    }
    let root = parse.syntax();
    print(&root, options, &mut out);

    let before = fingerprint(&root);
    let after = fingerprint(&papyrus_syntax::parse(&out, dialect).syntax());
    if before != after {
        let at = before
            .iter()
            .zip(&after)
            .position(|(a, b)| a != b)
            .unwrap_or(before.len().min(after.len()));
        return Err(FormatError::Unsafe {
            detail: format!(
                "token {at}: {:?} becomes {:?}",
                before.get(at),
                after.get(at)
            ),
        });
    }
    Ok(out)
}

// ── Splitting into lines ──────────────────────────────────────────────────

/// A physical line: its tokens, spaces excluded.
#[derive(Debug, Default)]
struct Line {
    tokens: Vec<SyntaxToken>,
    /// The line continues the previous one through a `\`.
    continuation: bool,
}

impl Line {
    /// True if the line only holds comments or a docstring.
    fn is_comment_only(&self) -> bool {
        self.tokens.iter().all(|t| t.kind().is_comment())
    }
}

/// The parentheses around a whole `If`, `ElseIf` or `While` condition, every
/// layer: `If (x)` is `If x`.
fn redundant_parens(root: &SyntaxNode) -> HashSet<SyntaxToken> {
    let mut out = HashSet::new();
    for node in root.descendants() {
        if !matches!(node.kind(), IF_STMT | ELSEIF_CLAUSE | WHILE_STMT) {
            continue;
        }
        let mut condition = node.children().next();
        while let Some(paren) = condition.filter(|c| c.kind() == PAREN_EXPR) {
            let tokens: Vec<SyntaxToken> = paren
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| matches!(t.kind(), L_PAREN | R_PAREN))
                .collect();
            // An unclosed one is left to the parser's judgment.
            if tokens.len() != 2 {
                break;
            }
            out.extend(tokens);
            condition = paren.children().next();
        }
    }
    out
}

fn lines(root: &SyntaxNode) -> Vec<Line> {
    let parens = redundant_parens(root);
    let mut lines = Vec::new();
    let mut current = Line::default();
    for element in root.descendants_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        if parens.contains(&token) {
            continue;
        }
        match token.kind() {
            WHITESPACE => {}
            NEWLINE => lines.push(std::mem::take(&mut current)),
            LINE_CONTINUATION => {
                current.tokens.push(token);
                lines.push(std::mem::take(&mut current));
                current.continuation = true;
            }
            _ => current.tokens.push(token),
        }
    }
    if !current.tokens.is_empty() {
        lines.push(current);
    }
    lines
}

// ── Indentation ───────────────────────────────────────────────────────────

/// Nesting depth of a significant token.
///
/// One level per statement body, and one per member of a declaration block:
/// function in a `State`, field in a `Struct`, property in a `Group`, accessor
/// in a `Property`.
fn level(token: &SyntaxToken) -> usize {
    token
        .parent_ancestors()
        .filter(|node| match node.kind() {
            STMT_LIST => true,
            FUNCTION_DECL | EVENT_DECL | STRUCT_FIELD | PROPERTY_DECL => {
                node.parent().is_some_and(|p| {
                    matches!(
                        p.kind(),
                        STATE_DECL | STRUCT_DECL | GROUP_DECL | PROPERTY_DECL
                    )
                })
            }
            _ => false,
        })
        .count()
}

/// True for words that close a block or open its next branch.
fn is_closer(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        ENDFUNCTION_KW
            | ENDEVENT_KW
            | ENDIF_KW
            | ELSEIF_KW
            | ELSE_KW
            | ENDWHILE_KW
            | ENDPROPERTY_KW
            | ENDSTATE_KW
            | ENDSTRUCT_KW
            | ENDGROUP_KW
            | ENDLOCKGUARD_KW
            | ELSETRYLOCKGUARD_KW
            | ENDTRYLOCKGUARD_KW
    )
}

/// The level of each line.
///
/// A code line takes the level of its first token. A comment line has none —
/// trivia is not always stored in the block it describes — so it aligns with
/// the code that follows it, or with the block body if that code closes the
/// block.
fn levels(lines: &[Line]) -> Vec<usize> {
    let mut levels = vec![0; lines.len()];
    let mut next_code: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().rev() {
        let Some(first) = line.tokens.first() else {
            continue;
        };
        if line.continuation {
            continue;
        }
        if line.is_comment_only() {
            levels[i] = next_code.unwrap_or(0);
        } else {
            let level = level(first);
            levels[i] = level;
            next_code = Some(if is_closer(first.kind()) {
                level + 1
            } else {
                level
            });
        }
    }

    // A continuation is indented two levels below its logical line.
    let mut base = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.continuation {
            levels[i] = base + 2;
        } else if !line.tokens.is_empty() {
            base = levels[i];
        }
    }
    levels
}

// ── Blocks ────────────────────────────────────────────────────────────────

/// A declaration over several lines — function, event, state, struct,
/// group, full property — of the script, a state or a group: set apart by
/// blank lines.
fn is_block(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        FUNCTION_DECL | EVENT_DECL | STATE_DECL | STRUCT_DECL | GROUP_DECL | PROPERTY_DECL
    ) && node
        .parent()
        .is_some_and(|p| matches!(p.kind(), SOURCE_FILE | STATE_DECL | GROUP_DECL))
        && node.children_with_tokens().any(|e| {
            matches!(
                e.kind(),
                ENDFUNCTION_KW
                    | ENDEVENT_KW
                    | ENDSTATE_KW
                    | ENDSTRUCT_KW
                    | ENDGROUP_KW
                    | ENDPROPERTY_KW
            )
        })
}

/// The first token of a node that is neither trivia nor a line break.
fn first_significant(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !t.kind().is_trivia() && t.kind() != NEWLINE)
}

/// The lines that must follow a blank line: the start of a block, with the
/// comments right above it, and what follows its end.
fn set_apart(lines: &[Line], levels: &[usize]) -> Vec<bool> {
    let mut apart = vec![false; lines.len()];
    let code = |i: usize| !lines[i].tokens.is_empty() && !lines[i].continuation;
    for (i, line) in lines.iter().enumerate() {
        let Some(first) = line.tokens.first().filter(|_| code(i)) else {
            continue;
        };
        let block = first.parent_ancestors().find(is_block);
        let starts = block
            .as_ref()
            .and_then(first_significant)
            .is_some_and(|t| &t == first);
        if starts {
            let mut start = i;
            while start > 0 && code(start - 1) && lines[start - 1].is_comment_only() {
                start -= 1;
            }
            // Not the first member of its state or group.
            let previous = (0..start).rev().find(|&j| code(j));
            if previous.is_some_and(|j| levels[j] >= levels[start]) {
                apart[start] = true;
            }
        }
        let ends = first.parent().is_some_and(|p| is_block(&p)) && is_closer(first.kind());
        if ends {
            if let Some(next) = (i + 1..lines.len()).find(|&j| code(j)) {
                apart[next] = true;
            }
        }
    }
    apart
}

// ── Writing ───────────────────────────────────────────────────────────────

fn print(root: &SyntaxNode, options: &FormatOptions, out: &mut String) {
    let lines = lines(root);
    let levels = levels(&lines);
    let apart = set_apart(&lines, &levels);
    let unit = match options.indent {
        Indent::Spaces(n) => " ".repeat(usize::from(n)),
        Indent::Tabs => "\t".to_owned(),
    };

    let mut started = false;
    let mut blank = false;
    for ((line, level), apart) in lines.iter().zip(levels).zip(apart) {
        if line.tokens.is_empty() {
            // At most one blank line, never at the start or end of the file.
            blank = started;
            continue;
        }
        // Nor before `EndIf`, `EndFunction`, `Else`…: a block does not end on
        // emptiness.
        let closes = line
            .tokens
            .first()
            .is_some_and(|t| is_closer(t.kind()) && !line.continuation);
        if (blank || (apart && started)) && !closes {
            out.push('\n');
        }
        blank = false;
        for _ in 0..level {
            out.push_str(&unit);
        }
        let mut prev: Option<&SyntaxToken> = None;
        for token in &line.tokens {
            if let Some(prev) = prev {
                if space_between(prev, token) {
                    out.push(' ');
                }
            }
            out.push_str(&text(token, options.keywords));
            prev = Some(token);
        }
        out.push('\n');
        started = true;
    }
}

/// The text of a token, as the formatter writes it.
fn text(token: &SyntaxToken, case: KeywordCase) -> Cow<'_, str> {
    let raw = token.text();
    match token.kind() {
        LINE_COMMENT => Cow::Borrowed(raw.trim_end()),
        BLOCK_COMMENT | DOC_COMMENT => Cow::Owned(raw.replace("\r\n", "\n")),
        LINE_CONTINUATION => Cow::Borrowed("\\"),
        kind if kind.is_keyword() && !keeps_case(token) => match case {
            KeywordCase::Lower => Cow::Owned(raw.to_ascii_lowercase()),
            KeywordCase::Pascal => Cow::Borrowed(pascal(raw).unwrap_or(raw)),
        },
        _ => Cow::Borrowed(raw),
    }
}

/// True for a keyword used here as a name, which therefore keeps the case
/// chosen by the author.
fn keeps_case(token: &SyntaxToken) -> bool {
    let Some(parent) = token.parent() else {
        return false;
    };
    match parent.kind() {
        // `Auto State Default`, `Group Mandatory`
        NAME => true,
        // `.Length`; `self` and `parent` stay keywords.
        NAME_REF => !matches!(token.kind(), SELF_KW | PARENT_KW),
        // Named types, like `Actor` — not primitives.
        TYPE_REF => matches!(
            token.kind(),
            CUSTOMEVENTNAME_KW | SCRIPTEVENTNAME_KW | STRUCTVARNAME_KW
        ),
        // A parameter's default value: compilers compare it with the parent's,
        // or the empty state's, as text — `False` against `false` is an error.
        LITERAL => {
            matches!(token.kind(), TRUE_KW | FALSE_KW | NONE_KW)
                && parent.parent().is_some_and(|p| p.kind() == PARAM)
        }
        _ => false,
    }
}

fn parent_kind(token: &SyntaxToken) -> Option<SyntaxKind> {
    token.parent().map(|p| p.kind())
}

/// True if two neighboring tokens on a line need a space between them.
fn space_between(prev: &SyntaxToken, next: &SyntaxToken) -> bool {
    let (p, n) = (prev.kind(), next.kind());

    // Comments and line continuations always stand apart.
    if p.is_comment() || n.is_comment() || n == LINE_CONTINUATION {
        return true;
    }

    match n {
        COMMA | R_PAREN | R_BRACK | DOT | COLON => return false,
        // `Foo(`, `Function F(`, `RequiresGuard(` — but `If (`.
        L_PAREN if matches!(parent_kind(next), Some(ARG_LIST | PARAM_LIST)) => return false,
        // `t[0]`, `int[]`, `new int[4]`
        L_BRACK => return false,
        _ => {}
    }
    match p {
        L_PAREN | L_BRACK | DOT | COLON => false,
        MINUS | BANG if parent_kind(prev) == Some(UNARY_EXPR) => false,
        _ => true,
    }
}

// ── Safeguard ─────────────────────────────────────────────────────────────

/// What formatting must not change: significant tokens — the parentheses
/// around a whole condition aside —, comments, the line breaks that
/// separate code, and the case of the keywords that keep it.
fn fingerprint(root: &SyntaxNode) -> Vec<(SyntaxKind, String)> {
    let parens = redundant_parens(root);
    let mut out: Vec<(SyntaxKind, String)> = Vec::new();
    let mut after_newline = true;
    for token in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !parens.contains(t))
    {
        let raw = token.text();
        let text = match token.kind() {
            WHITESPACE => continue,
            NEWLINE => {
                if !after_newline {
                    out.push((NEWLINE, String::new()));
                }
                after_newline = true;
                continue;
            }
            LINE_CONTINUATION => "\\".to_owned(),
            LINE_COMMENT => raw.trim_end().to_owned(),
            BLOCK_COMMENT | DOC_COMMENT => raw.replace("\r\n", "\n"),
            kind if kind.is_keyword() && !keeps_case(&token) => raw.to_ascii_lowercase(),
            _ => raw.to_owned(),
        };
        out.push((token.kind(), text));
        after_newline = false;
    }
    if out.last().is_some_and(|(k, _)| *k == NEWLINE) {
        out.pop();
    }
    out
}
