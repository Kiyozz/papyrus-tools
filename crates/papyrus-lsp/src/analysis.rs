//! An open document: its settings, its tree, its diagnostics, what can be
//! fixed in it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatError, FormatOptions, Indent, KeywordCase};
use papyrus_lint::{Levels, Model, Workspace, apply_fixes, vanilla_flags};
use papyrus_project::edit::{self as config, Change};
use papyrus_project::flags::{FlagDef, parse_flags};
use papyrus_project::{Config, Decoded, IndentSetting, Level, SourceEncoding, decode};
use papyrus_syntax::{Parse, TextRange, TextSize};
use similar::{DiffTag, TextDiff};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CreateFile, CreateFileOptions, Diagnostic,
    DiagnosticSeverity, DiagnosticTag, DocumentChangeOperation, DocumentChanges, NumberOrString,
    OneOf, OptionalVersionedTextDocumentIdentifier, Position, Range, ResourceOp, TextDocumentEdit,
    TextEdit, Uri, WorkspaceEdit,
};

use crate::position::{Columns, LineMap};

/// The diagnostics source, as the editor displays it.
const SOURCE: &str = "papyrus";

/// The settings of a document, from the `papyrus.toml` covering it.
#[derive(Debug, Default)]
pub(crate) struct Settings {
    /// `game` in papyrus.toml.
    pub(crate) game: Option<GameId>,
    /// The `.flg` named by papyrus.toml; without it, the game's.
    pub(crate) flags: Option<Vec<FlagDef>>,
    pub(crate) levels: Levels,
    pub(crate) indent: Indent,
    pub(crate) keywords: KeywordCase,
    /// Why papyrus.toml could not be read: the defaults apply, and the
    /// document says so.
    pub(crate) error: Option<String>,
    /// The papyrus.toml these settings come from.
    pub(crate) config: Option<PathBuf>,
}

impl Settings {
    /// The settings that apply to `path`.
    pub(crate) fn load(path: Option<&Path>) -> Settings {
        let Some(path) = path else {
            return Settings::default();
        };
        match Settings::try_load(path) {
            Ok(settings) => settings,
            Err(error) => Settings {
                error: Some(error),
                ..Settings::default()
            },
        }
    }

    fn try_load(path: &Path) -> Result<Settings, String> {
        let config = Config::discover(path).map_err(|e| e.to_string())?;
        let in_config = |message: String| match &config.path {
            Some(path) => format!("{}: {message}", path.display()),
            None => message,
        };
        let flags = match &config.flags {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                Some(parse_flags(&text).map_err(|e| format!("{}: {e}", path.display()))?)
            }
            None => None,
        };
        Ok(Settings {
            game: config.game,
            flags,
            levels: Levels::from_config(&config.lints).map_err(in_config)?,
            indent: match config.indent {
                Some(IndentSetting::Spaces(n)) => Indent::Spaces(n),
                Some(IndentSetting::Tab) => Indent::Tabs,
                None => Indent::default(),
            },
            keywords: if config.pascal_keywords {
                KeywordCase::Pascal
            } else {
                KeywordCase::Lower
            },
            error: None,
            config: config.path,
        })
    }
}

/// A document open in the editor.
#[derive(Debug)]
pub(crate) struct Document {
    /// The editor's text, and the encoding of the file on disk.
    pub(crate) decoded: Decoded,
    pub(crate) version: i32,
    pub(crate) path: Option<PathBuf>,
    pub(crate) settings: Settings,
    /// The root of its project.
    pub(crate) project: PathBuf,
}

impl Document {
    pub(crate) fn new(text: String, version: i32, path: Option<PathBuf>) -> Document {
        let settings = Settings::load(path.as_deref());
        let mut document = Document {
            decoded: Decoded {
                text,
                encoding: SourceEncoding::Utf8,
                lossy: false,
            },
            version,
            path,
            settings,
            project: PathBuf::new(),
        };
        document.reload_encoding();
        document
    }

    /// Rereads the encoding of the file on disk, which the editor does not
    /// send.
    ///
    /// An ASCII file, or a missing one, will be saved as UTF-8 without BOM
    /// as soon as it contains anything else: that is the editors' default.
    pub(crate) fn reload_encoding(&mut self) {
        let on_disk = self
            .path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| decode(&bytes).encoding);
        self.decoded.encoding = match on_disk {
            Some(SourceEncoding::Ascii) | None => SourceEncoding::Utf8,
            Some(encoding) => encoding,
        };
    }
}

/// The tree and diagnostics of a document.
#[derive(Debug)]
pub(crate) struct Analysis {
    pub(crate) parse: Parse,
    pub(crate) game: Option<GameId>,
    pub(crate) lints: Vec<papyrus_lint::Diagnostic>,
}

/// Analyzes `document`. `detected`: the target deduced from the project's
/// scripts; `model`: the document in its project's semantic model.
///
/// The target comes from papyrus.toml, else from the project, else from the
/// document itself. Without any clue, Starfield's syntax applies and
/// game-specific lints stay quiet, as on the command line.
pub(crate) fn analyze(
    document: &Document,
    workspace: &Workspace,
    detected: Option<GameId>,
    model: Option<Model<'_>>,
) -> Analysis {
    let text = &document.decoded.text;
    let widest = Dialect::new(GameId::Starfield);
    let (game, parse) = match document.settings.game.or(detected) {
        Some(game) => (Some(game), papyrus_syntax::parse(text, Dialect::new(game))),
        None => {
            let parse = papyrus_syntax::parse(text, widest);
            match papyrus_project::detect::evidence(&parse.syntax()).verdict() {
                Some(game) if Dialect::new(game) != widest => {
                    (Some(game), papyrus_syntax::parse(text, Dialect::new(game)))
                }
                game => (game, parse),
            }
        }
    };
    let flags = match &document.settings.flags {
        Some(flags) => flags.clone(),
        None => vanilla_flags(game),
    };
    let root = parse.syntax();
    let context = papyrus_lint::Context {
        decoded: &document.decoded,
        root: &root,
        game,
        flags: &flags,
        workspace,
        model,
    };
    let lints = papyrus_lint::run(&context, &document.settings.levels);
    Analysis { parse, game, lints }
}

pub(crate) fn lint_diagnostic(lint: &papyrus_lint::Diagnostic, map: &LineMap) -> Diagnostic {
    Diagnostic {
        range: map.range(lint.range),
        severity: Some(if lint.level == Level::Error {
            DiagnosticSeverity::ERROR
        } else {
            DiagnosticSeverity::WARNING
        }),
        code: Some(NumberOrString::String(lint.lint.code.to_owned())),
        source: Some(SOURCE.to_owned()),
        message: format!("{} [{}]", lint.message, lint.lint.name),
        // Dead code, which editors fade.
        tags: [
            papyrus_lint::CODE_UNREACHABLE.code,
            papyrus_lint::LOCALS_UNUSED.code,
            papyrus_lint::PARAMS_UNUSED.code,
        ]
        .contains(&lint.lint.code)
        .then(|| vec![DiagnosticTag::UNNECESSARY]),
        ..Diagnostic::default()
    }
}

/// The diagnostics to publish for `document`.
pub(crate) fn diagnostics(
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(error) = &document.settings.error {
        out.push(Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("config".to_owned())),
            source: Some(SOURCE.to_owned()),
            message: format!("{error} — default settings applied"),
            ..Diagnostic::default()
        });
    }
    out.extend(analysis.parse.errors().iter().map(|e| Diagnostic {
        range: map.range(e.range()),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("syntax".to_owned())),
        source: Some(SOURCE.to_owned()),
        message: e.message().to_owned(),
        ..Diagnostic::default()
    }));
    out.extend(analysis.lints.iter().map(|d| lint_diagnostic(d, map)));
    out
}

fn edit(uri: &Uri, edits: Vec<TextEdit>) -> WorkspaceEdit {
    WorkspaceEdit {
        changes: Some(HashMap::from([(uri.clone(), edits)])),
        ..WorkspaceEdit::default()
    }
}

fn text_edits(edits: &[papyrus_lint::Edit], map: &LineMap) -> Vec<TextEdit> {
    edits
        .iter()
        .map(|e| TextEdit::new(map.range(e.range), e.text.clone()))
        .collect()
}

/// The indentation of the line containing `offset`.
fn indentation<'a>(text: &'a str, map: &LineMap, offset: TextSize) -> (TextSize, &'a str) {
    let start = map.line_start(map.line(offset)).unwrap_or_default();
    let line = &text[usize::from(start)..];
    let width = line.len() - line.trim_start_matches([' ', '\t']).len();
    (start, &line[..width])
}

/// How the client takes the edits of papyrus.toml.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConfigEdits {
    pub(crate) columns: Columns,
    /// The client creates files: without a papyrus.toml, one is created.
    pub(crate) can_create: bool,
}

impl ConfigEdits {
    /// `change` made to the papyrus.toml of `document`, or to a new one at
    /// the root of its project; `None` if the file cannot be read or
    /// parsed.
    fn edit(
        &self,
        document: &Document,
        game: Option<GameId>,
        change: Change<'_>,
    ) -> Option<WorkspaceEdit> {
        if let Some(path) = &document.settings.config {
            let text = std::fs::read_to_string(path).ok()?;
            let new = config::apply(&text, change)?;
            let uri = Uri::from_file_path(path)?;
            return Some(edit(&uri, vec![minimal_edit(&text, &new, self.columns)]));
        }
        document.path.as_ref()?;
        if !self.can_create {
            return None;
        }
        let path = document.project.join(papyrus_project::config::FILE_NAME);
        let uri = Uri::from_file_path(&path)?;
        let text = config::apply(&config::template(game), change)?;
        let create = DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
            uri: uri.clone(),
            options: Some(CreateFileOptions {
                overwrite: Some(false),
                ignore_if_exists: Some(true),
            }),
            annotation_id: None,
        }));
        let fill = DocumentChangeOperation::Edit(TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
            edits: vec![OneOf::Left(TextEdit::new(Range::default(), text))],
        });
        Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Operations(vec![create, fill])),
            ..WorkspaceEdit::default()
        })
    }
}

/// The edit turning `old` into `new`: what lies between their common
/// prefix and suffix.
fn minimal_edit(old: &str, new: &str, columns: Columns) -> TextEdit {
    let prefix = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, a), b)| a != b)
        .map_or(old.len().min(new.len()), |((i, _), _)| i);
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum::<usize>();
    let map = LineMap::new(old, columns);
    let range = TextRange::new(
        TextSize::from(prefix as u32),
        TextSize::from((old.len() - suffix) as u32),
    );
    TextEdit::new(map.range(range), new[prefix..new.len() - suffix].to_owned())
}

/// The actions offered on `range`: safe fixes, suggestions, suppression on
/// the line or in papyrus.toml, and fixing the whole file.
pub(crate) fn code_actions(
    uri: &Uri,
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
    range: TextRange,
    config: ConfigEdits,
) -> Vec<CodeActionOrCommand> {
    let text = &document.decoded.text;
    let mut actions = Vec::new();
    // Each lint once in papyrus.toml, whatever its diagnostics in range.
    let mut configured = Vec::new();
    for lint in &analysis.lints {
        if lint.range.intersect(range).is_none() {
            continue;
        }
        let diagnostic = lint_diagnostic(lint, map);
        let mut quickfix = |title: String, edits: Vec<TextEdit>, preferred: bool| {
            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(edit(uri, edits)),
                is_preferred: Some(preferred),
                ..CodeAction::default()
            }));
        };
        if let Some(fix) = &lint.fix {
            let title = match fix.first() {
                Some(first) => format!("Rename to `{}`", first.text),
                None => "Fix".to_owned(),
            };
            quickfix(title, text_edits(fix, map), true);
        }
        // Never preferred, never part of "fix all": the user picks it.
        if let Some(fix) = &lint.unsafe_fix {
            if let Some(first) = fix.first() {
                let title = format!(
                    "Rename to `{}` (unsafe: plugins bind properties by name)",
                    first.text
                );
                quickfix(title, text_edits(fix, map), false);
            }
        }
        for suggestion in &lint.suggestions {
            quickfix(
                suggestion.title.clone(),
                text_edits(&suggestion.edits, map),
                false,
            );
        }
        let (start, indent) = indentation(text, map, lint.range.start());
        let at = map.position(start);
        quickfix(
            format!("Allow `{}` on this line", lint.lint.name),
            vec![TextEdit::new(
                Range::new(at, at),
                format!("{indent}; papyrus: allow({})\n", lint.lint.name),
            )],
            false,
        );

        if configured.contains(&lint.lint.code) {
            continue;
        }
        configured.push(lint.lint.code);
        let mut in_config = |title: String, change: Change<'_>| {
            if let Some(edit) = config.edit(document, analysis.game, change) {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diagnostic.clone()]),
                    edit: Some(edit),
                    ..CodeAction::default()
                }));
            }
        };
        if lint.lint.code == papyrus_lint::MEMBERS_SKSE.code {
            in_config("Enable SKSE in papyrus.toml".to_owned(), Change::EnableSkse);
        }
        let name = lint.lint.name;
        let level = |level| Change::Lint {
            code: lint.lint.code,
            name,
            level,
        };
        in_config(
            format!("Allow `{name}` in papyrus.toml"),
            level(Level::Allow),
        );
        if lint.level == Level::Warn {
            in_config(
                format!("Make `{name}` an error in papyrus.toml"),
                level(Level::Error),
            );
        } else {
            in_config(
                format!("Make `{name}` a warning in papyrus.toml"),
                level(Level::Warn),
            );
        }
    }

    // Every safe fix of the file at once: `source.fixAll`, which editors can
    // apply on save.
    let fixable = analysis.lints.iter().any(|d| d.fix.is_some());
    if fixable && analysis.parse.ok() && !document.decoded.lossy {
        if let Some(fixed) = apply_fixes(text, &analysis.lints) {
            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Apply all safe papyrus fixes".to_owned(),
                kind: Some(CodeActionKind::SOURCE_FIX_ALL),
                edit: Some(edit(uri, vec![whole(text, map, fixed)])),
                ..CodeAction::default()
            }));
        }
    }
    actions
}

/// Replaces the whole text.
fn whole(text: &str, map: &LineMap, new_text: String) -> TextEdit {
    let end = map.position(TextSize::of(text));
    TextEdit::new(Range::new(Position::new(0, 0), end), new_text)
}

/// The formatted text of a document, `None` if it already is.
fn formatted(document: &Document, analysis: &Analysis) -> Result<Option<String>, String> {
    let text = &document.decoded.text;
    let dialect = analysis.game.map(Dialect::new).unwrap_or_default();
    let options = FormatOptions {
        indent: document.settings.indent,
        keywords: document.settings.keywords,
    };
    match papyrus_fmt::format(text, dialect, &options) {
        Ok(formatted) if formatted == *text => Ok(None),
        Ok(formatted) => Ok(Some(formatted)),
        // Syntax errors are already displayed: nothing to add.
        Err(FormatError::Syntax(_)) => Ok(None),
        Err(error @ FormatError::Unsafe { .. }) => Err(format!("{error} — please report this bug")),
    }
}

/// The formatted document, or `None` if it already is.
pub(crate) fn format(
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
) -> Result<Option<Vec<TextEdit>>, String> {
    let text = &document.decoded.text;
    Ok(formatted(document, analysis)?.map(|new| vec![whole(text, map, new)]))
}

/// What formatting changes on the lines of `range`: the formatter works on
/// whole scripts, so the changes elsewhere are left out.
pub(crate) fn format_range(
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
    range: Range,
) -> Result<Option<Vec<TextEdit>>, String> {
    let (Some(start), Some(end)) = (map.offset(range.start), map.offset(range.end)) else {
        return Ok(None);
    };
    let text = &document.decoded.text;
    let Some(new) = formatted(document, analysis)? else {
        return Ok(None);
    };
    let selection = usize::from(start)..usize::from(end);
    Ok(Some(edits_within(text, &new, map, &selection)))
}

/// The changes from `old` to `new` on the lines `selection` touches.
fn edits_within(
    old: &str,
    new: &str,
    map: &LineMap,
    selection: &std::ops::Range<usize>,
) -> Vec<TextEdit> {
    let diff = TextDiff::from_lines(old, new);
    let new_lines = diff.new_slices();
    let mut starts = vec![0];
    for line in diff.old_slices() {
        starts.push(starts[starts.len() - 1] + line.len());
    }
    let mut edits = Vec::new();
    let mut change = |old_lines: std::ops::Range<usize>, new: std::ops::Range<usize>| {
        let (from, to) = (starts[old_lines.start], starts[old_lines.end]);
        let touched = if from == to {
            // Lines added: at the selection's edge or in it.
            selection.contains(&from) || from == selection.end
        } else {
            from < selection.end.max(selection.start + 1) && selection.start < to
        };
        if touched {
            let range = TextRange::new(TextSize::new(from as u32), TextSize::new(to as u32));
            edits.push(TextEdit::new(map.range(range), new_lines[new].concat()));
        }
    };
    for op in diff.ops() {
        let (old_lines, new) = (op.old_range(), op.new_range());
        if op.tag() == DiffTag::Equal {
            continue;
        }
        // Lines replaced one for one are as many changes: neighbours stay
        // out of the selection.
        if old_lines.len() == new.len() {
            for (old_line, new_line) in old_lines.zip(new) {
                change(old_line..old_line + 1, new_line..new_line + 1);
            }
        } else {
            change(old_lines, new);
        }
    }
    edits
}
