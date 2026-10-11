//! Le formateur canonique de Papyrus.
//!
//! Un seul style, peu d'options : le but est qu'on cesse d'en discuter, comme
//! avec `gofmt`. Le formateur travaille sur l'arbre syntaxique concret, ligne
//! par ligne — Papyrus est orienté ligne, et le formateur n'a jamais à décider
//! où couper.
//!
//! # Ce qu'il fait
//!
//! - réindente selon l'imbrication ;
//! - normalise les espaces dans la ligne ;
//! - écrit les mots-clés en minuscules, sauf quand ils servent de nom
//!   (`Auto State Default`, `.Length`) ;
//! - garde au plus une ligne vide, aucune avant un mot qui ferme un bloc ;
//! - retire les espaces de fin, écrit des fins de ligne LF et termine le
//!   fichier par une seule.
//!
//! # Ce qu'il ne fait jamais
//!
//! - toucher aux identifiants : Papyrus est insensible à la casse, mais
//!   renommer est l'affaire du linter, qui sait ce qui est sûr ;
//! - toucher au contenu des chaînes et des commentaires ;
//! - changer l'encodage : un BOM présent est conservé, aucun n'est ajouté ;
//! - formater un fichier qui ne se parse pas.
//!
//! # Garde-fou
//!
//! Avant de rendre son résultat, le formateur vérifie que la suite des tokens
//! significatifs est inchangée, à la casse des mots-clés près. Au moindre
//! écart, il refuse : mieux vaut ne pas formater que corrompre un script.

#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::fmt;

use papyrus_dialect::Dialect;
use papyrus_syntax::{ParseError, SyntaxKind, SyntaxNode, SyntaxToken, lex_with_dialect, parse};

use SyntaxKind::*;

/// Une unité d'indentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// Ce nombre d'espaces par niveau.
    Spaces(u8),
    /// Une tabulation par niveau.
    Tabs,
}

impl Default for Indent {
    fn default() -> Indent {
        Indent::Spaces(2)
    }
}

/// Les réglages du formateur.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatOptions {
    /// L'indentation, deux espaces par défaut.
    pub indent: Indent,
}

/// Pourquoi un fichier n'a pas été formaté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// La source ne se parse pas : la formater risquerait de l'abîmer.
    Syntax(Vec<ParseError>),
    /// Le résultat ne contient pas les mêmes tokens que la source. C'est un
    /// bug du formateur ; le fichier est laissé intact.
    Unsafe {
        /// Ce qui diffère, pour le rapport de bug.
        detail: String,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Syntax(errors) => {
                write!(
                    f,
                    "{} erreur(s) de syntaxe, fichier non formaté",
                    errors.len()
                )
            }
            FormatError::Unsafe { detail } => {
                write!(
                    f,
                    "formatage refusé, le résultat altérerait le code : {detail}"
                )
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Formate `source`, lue pour la cible `dialect`.
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
    print(&parse.syntax(), options, &mut out);

    let before = fingerprint(source, dialect);
    let after = fingerprint(&out, dialect);
    if before != after {
        let at = before
            .iter()
            .zip(&after)
            .position(|(a, b)| a != b)
            .unwrap_or(before.len().min(after.len()));
        return Err(FormatError::Unsafe {
            detail: format!(
                "token {at} : {:?} devient {:?}",
                before.get(at),
                after.get(at)
            ),
        });
    }
    Ok(out)
}

// ── Découpage en lignes ───────────────────────────────────────────────────

/// Une ligne physique : ses tokens, espaces exclus.
#[derive(Debug, Default)]
struct Line {
    tokens: Vec<SyntaxToken>,
    /// La ligne prolonge la précédente par un `\`.
    continuation: bool,
}

impl Line {
    /// Vrai si la ligne ne porte que des commentaires ou une docstring.
    fn is_comment_only(&self) -> bool {
        self.tokens.iter().all(|t| t.kind().is_comment())
    }
}

fn lines(root: &SyntaxNode) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut current = Line::default();
    for element in root.descendants_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
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

/// Profondeur d'imbrication d'un token significatif.
///
/// Un niveau par corps d'instructions, et un par membre d'un bloc de
/// déclarations : fonction dans un `State`, champ dans un `Struct`, property
/// dans un `Group`, accesseur dans une `Property`.
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

/// Vrai pour les mots qui ferment un bloc ou en ouvrent la branche suivante.
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

/// Le niveau de chaque ligne.
///
/// Une ligne de code prend celui de son premier token. Une ligne de
/// commentaire n'en a pas — la trivia n'est pas toujours rangée dans le bloc
/// qu'elle décrit — et s'aligne donc sur le code qui la suit, ou sur le corps
/// du bloc si ce code le ferme.
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

    // Une continuation s'indente de deux niveaux sous sa ligne logique.
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

// ── Écriture ──────────────────────────────────────────────────────────────

fn print(root: &SyntaxNode, options: &FormatOptions, out: &mut String) {
    let lines = lines(root);
    let levels = levels(&lines);
    let unit = match options.indent {
        Indent::Spaces(n) => " ".repeat(usize::from(n)),
        Indent::Tabs => "\t".to_owned(),
    };

    let mut started = false;
    let mut blank = false;
    for (line, level) in lines.iter().zip(levels) {
        if line.tokens.is_empty() {
            // Au plus une ligne vide, jamais en tête ni en fin de fichier.
            blank = started;
            continue;
        }
        // Ni avant `EndIf`, `EndFunction`, `Else`… : un bloc ne finit pas sur
        // du vide.
        let closes = line
            .tokens
            .first()
            .is_some_and(|t| is_closer(t.kind()) && !line.continuation);
        if blank && !closes {
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
            out.push_str(&text(token));
            prev = Some(token);
        }
        out.push('\n');
        started = true;
    }
}

/// Le texte d'un token, tel que le formateur l'écrit.
fn text(token: &SyntaxToken) -> Cow<'_, str> {
    let raw = token.text();
    match token.kind() {
        LINE_COMMENT => Cow::Borrowed(raw.trim_end()),
        BLOCK_COMMENT | DOC_COMMENT => Cow::Owned(raw.replace("\r\n", "\n")),
        LINE_CONTINUATION => Cow::Borrowed("\\"),
        kind if kind.is_keyword() && !keeps_case(token) => Cow::Owned(raw.to_ascii_lowercase()),
        _ => Cow::Borrowed(raw),
    }
}

/// Vrai pour un mot-clé qui sert ici de nom, et garde donc la casse choisie
/// par l'auteur.
fn keeps_case(token: &SyntaxToken) -> bool {
    let Some(parent) = token.parent() else {
        return false;
    };
    match parent.kind() {
        // `Auto State Default`, `Group Mandatory`
        NAME => true,
        // `.Length` ; `self` et `parent` restent des mots-clés.
        NAME_REF => !matches!(token.kind(), SELF_KW | PARENT_KW),
        // Des types nommés, comme `Actor` — pas des primitifs.
        TYPE_REF => matches!(
            token.kind(),
            CUSTOMEVENTNAME_KW | SCRIPTEVENTNAME_KW | STRUCTVARNAME_KW
        ),
        _ => false,
    }
}

fn parent_kind(token: &SyntaxToken) -> Option<SyntaxKind> {
    token.parent().map(|p| p.kind())
}

/// Vrai s'il faut une espace entre deux tokens voisins sur une ligne.
fn space_between(prev: &SyntaxToken, next: &SyntaxToken) -> bool {
    let (p, n) = (prev.kind(), next.kind());

    // Les commentaires et la continuation se détachent toujours.
    if p.is_comment() || n.is_comment() || n == LINE_CONTINUATION {
        return true;
    }

    match n {
        COMMA | R_PAREN | R_BRACK | DOT | COLON => return false,
        // `Foo(`, `Function F(`, `RequiresGuard(` — mais `If (`.
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

// ── Garde-fou ─────────────────────────────────────────────────────────────

/// Ce que le formatage ne doit pas changer : les tokens significatifs, les
/// commentaires, et les fins de ligne qui séparent du code.
fn fingerprint(source: &str, dialect: Dialect) -> Vec<(SyntaxKind, String)> {
    let mut out: Vec<(SyntaxKind, String)> = Vec::new();
    let mut after_newline = true;
    for token in lex_with_dialect(source, dialect) {
        let raw = token.text(source);
        let text = match token.kind {
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
            kind if kind.is_keyword() => raw.to_ascii_lowercase(),
            _ => raw.to_owned(),
        };
        out.push((token.kind, text));
        after_newline = false;
    }
    if out.last().is_some_and(|(k, _)| *k == NEWLINE) {
        out.pop();
    }
    out
}
