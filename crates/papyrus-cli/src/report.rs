//! Diagnostic rendering.

use std::ops::Range;
use std::path::Path;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use papyrus_lint::{LINTS, explanation};
use papyrus_project::Level;
use papyrus_syntax::TextRange;
use serde_json::{Value, json};

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
    /// A JSON array on standard output, for scripts.
    Json,
    /// SARIF 2.1.0 on standard output, for GitHub code scanning.
    Sarif,
}

/// A problem kept for the JSON and SARIF reports.
#[derive(Debug)]
struct Record {
    path: std::path::PathBuf,
    severity: &'static str,
    code: String,
    name: Option<String>,
    message: String,
    start: (usize, usize),
    end: (usize, usize),
}

/// Prints diagnostics and counts them.
#[derive(Debug)]
pub(crate) struct Reporter {
    color: bool,
    pub(crate) format: MessageFormat,
    errors: usize,
    warnings: usize,
    records: Vec<Record>,
}

impl Reporter {
    pub(crate) fn new(color: bool) -> Reporter {
        Reporter {
            color,
            format: MessageFormat::Human,
            errors: 0,
            warnings: 0,
            records: Vec::new(),
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
            match self.format {
                MessageFormat::Human => {}
                MessageFormat::Short => {
                    eprintln!("{}", short(path, source, diagnostic));
                    continue;
                }
                MessageFormat::Json | MessageFormat::Sarif => {
                    self.records.push(Record::of(path, source, diagnostic));
                    continue;
                }
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

    /// Where to learn more, once problems were shown to someone.
    pub(crate) fn hint(&self) {
        if self.format == MessageFormat::Human && !self.is_clean() {
            eprintln!("`papyrus explain <code>` says what a lint means, and what to do");
        }
    }

    /// Prints the JSON or SARIF report, on standard output.
    pub(crate) fn finish(&self) {
        let report = match self.format {
            MessageFormat::Human | MessageFormat::Short => return,
            MessageFormat::Json => json_report(&self.records),
            MessageFormat::Sarif => sarif_report(&self.records),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("a JSON value")
        );
    }
}

impl Record {
    fn of(path: &Path, source: &str, diagnostic: &Diagnostic) -> Record {
        let (code, name) = match diagnostic.code.split_once(' ') {
            Some((code, name)) => (code.to_owned(), Some(name.to_owned())),
            None => (diagnostic.code.clone(), None),
        };
        Record {
            path: std::path::absolute(path).unwrap_or_else(|_| path.to_owned()),
            severity: severity(diagnostic),
            code,
            name,
            message: diagnostic.message.clone(),
            start: position(source, usize::from(diagnostic.range.start())),
            end: position(source, usize::from(diagnostic.range.end())),
        }
    }
}

fn json_report(records: &[Record]) -> Value {
    records
        .iter()
        .map(|r| {
            json!({
                "path": r.path,
                "severity": r.severity,
                "code": r.code,
                "name": r.name,
                "message": r.message,
                "line": r.start.0,
                "column": r.start.1,
                "endLine": r.end.0,
                "endColumn": r.end.1,
            })
        })
        .collect()
}

fn sarif_report(records: &[Record]) -> Value {
    let rules: Vec<Value> = LINTS
        .iter()
        .map(|lint| {
            let level = match lint.default {
                Level::Allow => "none",
                Level::Warn => "warning",
                Level::Error => "error",
            };
            json!({
                "id": lint.code,
                "name": lint.name,
                "shortDescription": { "text": lint.description },
                // Code scanning requires it.
                "fullDescription": { "text": explanation(lint) },
                "help": { "text": explanation(lint) },
                "defaultConfiguration": { "level": level },
            })
        })
        .collect();
    let root = std::env::current_dir().ok();
    let results: Vec<Value> = records
        .iter()
        .map(|r| {
            json!({
                "ruleId": r.code,
                "level": r.severity,
                "message": { "text": r.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri(&r.path, root.as_deref()) },
                        "region": {
                            "startLine": r.start.0,
                            "startColumn": r.start.1,
                            "endLine": r.end.0,
                            "endColumn": r.end.1,
                        },
                    },
                }],
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "papyrus",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/Kiyozz/papyrus-tools",
                    "rules": rules,
                },
            },
            "results": results,
        }],
    })
}

/// The URI of `path`: relative to `root` when under it, as code scanning
/// wants, else absolute.
fn uri(path: &Path, root: Option<&Path>) -> String {
    let relative = root.and_then(|root| path.strip_prefix(root).ok());
    let text = relative
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut out = String::new();
    if relative.is_none() {
        out.push_str(if text.starts_with('/') {
            "file://"
        } else {
            "file:///"
        });
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char);
            }
            // The drive of a Windows path.
            b':' if relative.is_none() => out.push(':'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

const fn severity(diagnostic: &Diagnostic) -> &'static str {
    match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// The line and column of `offset`, from 1, in characters.
fn position(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[start..].chars().count() + 1)
}

/// `diagnostic` on one line, its path absolute.
fn short(path: &Path, source: &str, diagnostic: &Diagnostic) -> String {
    let (line, column) = position(source, usize::from(diagnostic.range.start()));
    let (end_line, end_column) = position(source, usize::from(diagnostic.range.end()));
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let severity = severity(diagnostic);
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
