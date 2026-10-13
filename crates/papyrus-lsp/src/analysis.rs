//! An open document: its settings, its tree, its diagnostics, what can be
//! fixed in it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatError, FormatOptions, Indent};
use papyrus_lint::{Levels, Model, Workspace, apply_fixes, vanilla_flags};
use papyrus_project::flags::{FlagDef, parse_flags};
use papyrus_project::{Config, Decoded, IndentSetting, Level, SourceEncoding, decode};
use papyrus_syntax::{Parse, TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticSeverity, DiagnosticTag,
    NumberOrString, Position, Range, TextEdit, Uri, WorkspaceEdit,
};

use crate::position::LineMap;

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
    /// Why papyrus.toml could not be read: the defaults apply, and the
    /// document says so.
    pub(crate) error: Option<String>,
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
            error: None,
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

fn lint_diagnostic(lint: &papyrus_lint::Diagnostic, map: &LineMap) -> Diagnostic {
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

/// The actions offered on `range`: safe fixes, suggestions, suppression, and
/// fixing the whole file.
pub(crate) fn code_actions(
    uri: &Uri,
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
    range: TextRange,
) -> Vec<CodeActionOrCommand> {
    let text = &document.decoded.text;
    let mut actions = Vec::new();
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

/// The formatted document, or `None` if it already is.
pub(crate) fn format(
    document: &Document,
    analysis: &Analysis,
    map: &LineMap,
) -> Result<Option<Vec<TextEdit>>, String> {
    let text = &document.decoded.text;
    let dialect = analysis.game.map(Dialect::new).unwrap_or_default();
    let options = FormatOptions {
        indent: document.settings.indent,
    };
    match papyrus_fmt::format(text, dialect, &options) {
        Ok(formatted) if formatted == *text => Ok(None),
        Ok(formatted) => Ok(Some(vec![whole(text, map, formatted)])),
        // Syntax errors are already displayed: nothing to add.
        Err(FormatError::Syntax(_)) => Ok(None),
        Err(error @ FormatError::Unsafe { .. }) => Err(format!("{error} — please report this bug")),
    }
}
