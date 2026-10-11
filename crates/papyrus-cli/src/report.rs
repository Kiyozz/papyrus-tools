//! L'affichage des diagnostics.

use std::ops::Range;
use std::path::Path;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use papyrus_syntax::TextRange;

/// Gravité d'un diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
}

/// Un problème situé dans un fichier.
#[derive(Debug, Clone)]
pub(crate) struct Diagnostic {
    pub(crate) severity: Severity,
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) range: TextRange,
}

/// Affiche les diagnostics et les compte.
#[derive(Debug)]
pub(crate) struct Reporter {
    color: bool,
    errors: usize,
    warnings: usize,
}

impl Reporter {
    pub(crate) fn new(color: bool) -> Reporter {
        Reporter {
            color,
            errors: 0,
            warnings: 0,
        }
    }

    /// Vrai si rien n'a été signalé.
    pub(crate) fn is_clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0
    }

    pub(crate) fn emit(&mut self, path: &Path, source: &str, diagnostics: &[Diagnostic]) {
        let name = path.display().to_string();
        for diagnostic in diagnostics {
            let (kind, color) = match diagnostic.severity {
                Severity::Error => {
                    self.errors += 1;
                    (ReportKind::Error, Color::Red)
                }
                Severity::Warning => {
                    self.warnings += 1;
                    (ReportKind::Warning, Color::Yellow)
                }
            };
            let span = span(diagnostic.range, source);
            let report = Report::build(kind, (name.as_str(), span.clone()))
                .with_config(
                    Config::default()
                        .with_index_type(IndexType::Byte)
                        .with_color(self.color),
                )
                .with_code(diagnostic.code)
                .with_message(&diagnostic.message)
                // Sans texte, ariadne ne souligne pas : le label dit « ici ».
                .with_label(
                    Label::new((name.as_str(), span))
                        .with_message("ici")
                        .with_color(color),
                )
                .finish();
            let _ = report.eprint((name.as_str(), Source::from(source)));
        }
    }

    /// Un fichier laissé de côté, et pourquoi.
    pub(crate) fn skip(&mut self, path: &Path, reason: &str) {
        self.errors += 1;
        eprintln!("ignoré : {} — {reason}", path.display());
    }

    pub(crate) fn io_error(&mut self, path: &Path, error: &std::io::Error) {
        self.errors += 1;
        eprintln!("erreur : {} — {error}", path.display());
    }

    pub(crate) fn summary(&self, files: usize, verb: &str) {
        eprintln!(
            "{files} fichier(s) {verb} : {} erreur(s), {} avertissement(s)",
            self.errors, self.warnings
        );
    }
}

/// La plage en octets, élargie d'un caractère si elle est vide (fin de
/// fichier) : un label vide ne s'affiche pas.
fn span(range: TextRange, source: &str) -> Range<usize> {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    if start < end {
        return start..end;
    }
    match source[start..].chars().next() {
        Some(c) => start..start + c.len_utf8(),
        None => start..start,
    }
}
