//! Diagnostic rendering.

use std::ops::Range;
use std::path::Path;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use papyrus_syntax::TextRange;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
}

/// A problem located in a file.
#[derive(Debug, Clone)]
pub(crate) struct Diagnostic {
    pub(crate) severity: Severity,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) range: TextRange,
}

/// Prints diagnostics and counts them.
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

    /// True if nothing was reported.
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
                .with_code(&diagnostic.code)
                .with_message(&diagnostic.message)
                // Without text, ariadne does not underline: the label says "here".
                .with_label(
                    Label::new((name.as_str(), span))
                        .with_message("here")
                        .with_color(color),
                )
                .finish();
            let _ = report.eprint((name.as_str(), Source::from(source)));
        }
    }

    /// A file left aside, and why.
    pub(crate) fn skip(&mut self, path: &Path, reason: &str) {
        self.errors += 1;
        eprintln!("skipped: {} — {reason}", path.display());
    }

    pub(crate) fn io_error(&mut self, path: &Path, error: &std::io::Error) {
        self.errors += 1;
        eprintln!("error: {} — {error}", path.display());
    }

    pub(crate) fn summary(&self, files: usize, verb: &str) {
        eprintln!(
            "{files} file(s) {verb}: {} error(s), {} warning(s)",
            self.errors, self.warnings
        );
    }
}

/// The byte range, widened by one character when empty (end of file): an
/// empty label is not displayed.
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
