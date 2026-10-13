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

/// How diagnostics are printed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum MessageFormat {
    /// With the source line and a caret.
    #[default]
    Human,
    /// One line each, for editors:
    /// `path:line:column:end line:end column: error[PAP001]: message`,
    /// lines and columns from 1, in characters.
    Short,
}

/// Prints diagnostics and counts them.
#[derive(Debug)]
pub(crate) struct Reporter {
    color: bool,
    pub(crate) format: MessageFormat,
    errors: usize,
    warnings: usize,
}

impl Reporter {
    pub(crate) fn new(color: bool) -> Reporter {
        Reporter {
            color,
            format: MessageFormat::Human,
            errors: 0,
            warnings: 0,
        }
    }

    /// True if nothing was reported.
    pub(crate) fn is_clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0
    }

    /// The errors reported so far.
    pub(crate) fn errors(&self) -> usize {
        self.errors
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
            if self.format == MessageFormat::Short {
                eprintln!("{}", short(path, source, diagnostic));
                continue;
            }
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

    /// An error that names no file.
    pub(crate) fn general(&mut self, message: &str) {
        self.errors += 1;
        eprintln!("error: {message}");
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

/// `diagnostic` on one line, its path absolute.
fn short(path: &Path, source: &str, diagnostic: &Diagnostic) -> String {
    // Line and column from 1, in characters.
    let at = |offset: usize| {
        let before = &source[..offset.min(source.len())];
        let line = before.matches('\n').count() + 1;
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        (line, before[start..].chars().count() + 1)
    };
    let (line, column) = at(usize::from(diagnostic.range.start()));
    let (end_line, end_column) = at(usize::from(diagnostic.range.end()));
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    format!(
        "{}:{line}:{column}:{end_line}:{end_column}: {severity}[{}]: {}",
        path.display(),
        diagnostic.code,
        diagnostic.message
    )
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
