//! Les lints de Papyrus.
//!
//! Un lint ne signale que ce que le compilateur laisse passer, ou ce qu'il ne
//! dit qu'au moment de compiler — jamais ce qu'une information absente
//! empêche de trancher. Chaque lint a un code stable (`PAP101`) et un nom
//! lisible (`naming/locals`), utilisables l'un comme l'autre dans
//! `papyrus.toml` et dans les commentaires de suppression :
//!
//! ```papyrus
//! int HP = 0 ; papyrus: allow(naming/locals)
//!
//! ; papyrus: allow(PAP001)
//! string s = "café"
//! ```

#![forbid(unsafe_code)]

mod case;
mod rules;
mod suppress;

use std::collections::BTreeMap;

use papyrus_dialect::GameId;
use papyrus_project::flags::FlagDef;
use papyrus_project::{Decoded, Level};
use papyrus_syntax::{SyntaxNode, TextRange, lex};

pub use case::{to_lower_camel, to_pascal};
pub use rules::vanilla_flags;

/// Ce qu'on sait d'un lint.
#[derive(Debug)]
pub struct Lint {
    /// Code stable, `PAP101`.
    pub code: &'static str,
    /// Nom lisible, `naming/locals`.
    pub name: &'static str,
    /// Niveau sans configuration.
    pub default: Level,
    /// Ce que le lint vérifie, en une phrase.
    pub description: &'static str,
}

/// `PAP001` : texte que le compilateur de la cible corrompra.
pub static ENCODING_LOSSY: Lint = Lint {
    code: "PAP001",
    name: "encoding/lossy",
    default: Level::Warn,
    description: "chaîne ou docstring que le compilateur de la cible corrompra sans le dire",
};

/// `PAP002` : flag posé sur un objet que le `.flg` n'autorise pas.
pub static FLAGS_PLACEMENT: Lint = Lint {
    code: "PAP002",
    name: "flags/placement",
    default: Level::Error,
    description: "flag que le fichier .flg du jeu n'autorise pas sur cet objet",
};

/// `PAP003` : nom inconnu dans `; papyrus: allow(...)`.
pub static ALLOW_UNKNOWN: Lint = Lint {
    code: "PAP003",
    name: "allow/unknown",
    default: Level::Warn,
    description: "nom de lint inconnu dans `; papyrus: allow(...)`, qui ne supprime donc rien",
};

/// `PAP101` : locales et paramètres en lowerCamelCase.
pub static NAMING_LOCALS: Lint = Lint {
    code: "PAP101",
    name: "naming/locals",
    default: Level::Warn,
    description: "locales et paramètres en lowerCamelCase",
};

/// `PAP102` : variables de script, properties et champs en lowerCamelCase.
pub static NAMING_VARIABLES: Lint = Lint {
    code: "PAP102",
    name: "naming/variables",
    default: Level::Warn,
    description: "variables de script, properties et champs de structure en lowerCamelCase",
};

/// `PAP103` : fonctions, événements et types en PascalCase.
pub static NAMING_FUNCTIONS: Lint = Lint {
    code: "PAP103",
    name: "naming/functions",
    default: Level::Warn,
    description: "fonctions, événements, states, structures et groupes en PascalCase",
};

/// Tous les lints, dans l'ordre de leur code.
pub static LINTS: &[&Lint] = &[
    &ENCODING_LOSSY,
    &FLAGS_PLACEMENT,
    &ALLOW_UNKNOWN,
    &NAMING_LOCALS,
    &NAMING_VARIABLES,
    &NAMING_FUNCTIONS,
];

/// Le lint désigné par `key`, code ou nom, sans tenir compte de la casse.
pub fn find(key: &str) -> Option<&'static Lint> {
    LINTS
        .iter()
        .copied()
        .find(|l| l.code.eq_ignore_ascii_case(key) || l.name.eq_ignore_ascii_case(key))
}

/// Distance d'édition, sans tenir compte de la casse.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<u8> = a.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let b: Vec<u8> = b.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

/// Le nom de lint le plus proche de `key`, s'il l'est assez pour être une
/// faute de frappe.
pub fn closest(key: &str) -> Option<&'static str> {
    LINTS
        .iter()
        .flat_map(|l| [l.name, l.code])
        .map(|candidate| (distance(key, candidate), candidate))
        .filter(|(d, _)| *d <= 3)
        .min_by_key(|(d, _)| *d)
        .map(|(_, candidate)| candidate)
}

/// « lint `x` inconnu », avec une suggestion si possible.
fn unknown_message(key: &str) -> String {
    match closest(key) {
        Some(name) => format!("lint `{key}` inconnu — vouliez-vous dire `{name}` ?"),
        None => format!("lint `{key}` inconnu ; `papyrus lints` donne la liste"),
    }
}

/// Le niveau de chaque lint.
#[derive(Debug, Clone)]
pub struct Levels(BTreeMap<&'static str, Level>);

impl Default for Levels {
    fn default() -> Levels {
        Levels(LINTS.iter().map(|l| (l.code, l.default)).collect())
    }
}

impl Levels {
    /// Les niveaux par défaut, modifiés par la section `[lints]`.
    pub fn from_config(lints: &BTreeMap<String, Level>) -> Result<Levels, String> {
        let mut levels = Levels::default();
        for (key, level) in lints {
            let lint = find(key).ok_or_else(|| unknown_message(key))?;
            levels.0.insert(lint.code, *level);
        }
        Ok(levels)
    }

    /// Le niveau de `lint`.
    pub fn get(&self, lint: &Lint) -> Level {
        self.0.get(lint.code).copied().unwrap_or(lint.default)
    }
}

/// Un remplacement de texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Ce qui est remplacé.
    pub range: TextRange,
    /// Le nouveau texte.
    pub text: String,
}

/// Un problème trouvé par un lint.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Le lint qui l'a trouvé.
    pub lint: &'static Lint,
    /// Avertissement ou erreur, selon la configuration.
    pub level: Level,
    /// Où.
    pub range: TextRange,
    /// Quoi, et comment y remédier.
    pub message: String,
    /// Le correctif, quand il est sûr.
    pub fix: Option<Vec<Edit>>,
}

/// Ce dont les lints ont besoin.
#[derive(Debug)]
pub struct Context<'a> {
    /// Le fichier tel que décodé.
    pub decoded: &'a Decoded,
    /// L'arbre de `decoded.text`.
    pub root: &'a SyntaxNode,
    /// La cible, si elle est connue. Les lints qui en dépendent se taisent
    /// sans elle.
    pub game: Option<GameId>,
    /// Les flags du `.flg` en vigueur.
    pub flags: &'a [FlagDef],
}

/// Ce que les règles produisent, avant niveaux et suppressions.
pub(crate) struct Finding {
    pub(crate) lint: &'static Lint,
    pub(crate) range: TextRange,
    pub(crate) message: String,
    pub(crate) fix: Option<Vec<Edit>>,
}

/// Lance les lints actifs et retire ce que le code a explicitement autorisé.
pub fn run(context: &Context<'_>, levels: &Levels) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    rules::run(context, &mut findings);

    let allowed = suppress::Suppressions::collect(&context.decoded.text);
    // Un `allow(...)` mal orthographié ne supprime rien : autant le dire.
    for (range, key) in &allowed.unknown {
        findings.push(Finding {
            lint: &ALLOW_UNKNOWN,
            range: *range,
            message: format!(
                "{} ; cette suppression est sans effet",
                unknown_message(key)
            ),
            fix: None,
        });
    }
    let mut diagnostics: Vec<Diagnostic> = findings
        .into_iter()
        .filter_map(|f| {
            let level = levels.get(f.lint);
            if level == Level::Allow || allowed.covers(f.lint, f.range.start()) {
                return None;
            }
            Some(Diagnostic {
                lint: f.lint,
                level,
                range: f.range,
                message: f.message,
                fix: f.fix,
            })
        })
        .collect();
    diagnostics.sort_by_key(|d| (d.range.start(), d.lint.code));
    diagnostics
}

/// Applique les correctifs de `diagnostics` à `source`.
///
/// Les correctifs ne changent que la casse : le résultat est vérifié token
/// par token, sans tenir compte de la casse, et `None` signale qu'il diffère.
/// Ce serait un bug ; le fichier doit alors rester intact.
pub fn apply_fixes(source: &str, diagnostics: &[Diagnostic]) -> Option<String> {
    let mut edits: Vec<&Edit> = diagnostics
        .iter()
        .filter_map(|d| d.fix.as_ref())
        .flatten()
        .collect();
    edits.sort_by(|a, b| {
        (a.range.start(), a.range.end(), &a.text).cmp(&(b.range.start(), b.range.end(), &b.text))
    });
    edits.dedup();

    let mut out = String::with_capacity(source.len());
    let mut cursor = 0;
    for edit in edits {
        let start = usize::from(edit.range.start());
        // Deux correctifs sur la même plage : le premier gagne.
        if start < cursor {
            continue;
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&edit.text);
        cursor = usize::from(edit.range.end());
    }
    out.push_str(&source[cursor..]);

    let before = lex(source);
    let after = lex(&out);
    let same = before.len() == after.len()
        && before
            .iter()
            .zip(&after)
            .all(|(a, b)| a.kind == b.kind && a.text(source).eq_ignore_ascii_case(b.text(&out)));
    same.then_some(out)
}
