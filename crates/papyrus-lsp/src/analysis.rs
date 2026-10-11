//! Un document ouvert : ses réglages, son arbre, ses diagnostics, ce qu'on
//! peut y corriger.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use papyrus_dialect::{Dialect, GameId};
use papyrus_fmt::{FormatError, FormatOptions, Indent};
use papyrus_lint::{Levels, Workspace, apply_fixes, vanilla_flags};
use papyrus_project::flags::{FlagDef, parse_flags};
use papyrus_project::{Config, Decoded, IndentSetting, Level, SourceEncoding, decode};
use papyrus_syntax::{Parse, TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Diagnostic, DiagnosticSeverity,
    NumberOrString, Position, Range, TextEdit, Uri, WorkspaceEdit,
};

use crate::position::LineMap;

/// La source des diagnostics, telle que l'éditeur l'affiche.
const SOURCE: &str = "papyrus";

/// Les réglages d'un document, d'après le `papyrus.toml` qui le couvre.
#[derive(Debug, Default)]
pub(crate) struct Settings {
    /// `game` dans papyrus.toml.
    pub(crate) game: Option<GameId>,
    /// Le `.flg` désigné par papyrus.toml ; sans lui, celui du jeu.
    pub(crate) flags: Option<Vec<FlagDef>>,
    pub(crate) levels: Levels,
    pub(crate) indent: Indent,
    /// Pourquoi papyrus.toml n'a pas pu être lu : les réglages par défaut
    /// s'appliquent, et le document le dit.
    pub(crate) error: Option<String>,
}

impl Settings {
    /// Les réglages qui s'appliquent à `path`.
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
            Some(path) => format!("{} : {message}", path.display()),
            None => message,
        };
        let flags = match &config.flags {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("{} : {e}", path.display()))?;
                Some(parse_flags(&text).map_err(|e| format!("{} : {e}", path.display()))?)
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

/// Un document ouvert dans l'éditeur.
#[derive(Debug)]
pub(crate) struct Document {
    /// Le texte de l'éditeur, et l'encodage du fichier sur le disque.
    pub(crate) decoded: Decoded,
    pub(crate) version: i32,
    pub(crate) path: Option<PathBuf>,
    pub(crate) settings: Settings,
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
        };
        document.reload_encoding();
        document
    }

    /// Relit l'encodage du fichier sur le disque, que l'éditeur ne
    /// transmet pas.
    ///
    /// Un fichier ASCII, ou absent, sera enregistré en UTF-8 sans BOM dès
    /// qu'il contiendra autre chose : c'est le défaut des éditeurs.
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

/// L'arbre et les diagnostics d'un document.
#[derive(Debug)]
pub(crate) struct Analysis {
    pub(crate) parse: Parse,
    pub(crate) game: Option<GameId>,
    pub(crate) lints: Vec<papyrus_lint::Diagnostic>,
}

/// Analyse `document`. `detected` : la cible déduite des scripts du projet.
///
/// La cible vient de papyrus.toml, sinon du projet, sinon du document
/// lui-même. Sans indice, la syntaxe de Starfield s'applique et les lints
/// propres au jeu se taisent, comme en ligne de commande.
pub(crate) fn analyze(
    document: &Document,
    workspace: &Workspace,
    detected: Option<GameId>,
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
        ..Diagnostic::default()
    }
}

/// Les diagnostics à publier pour `document`.
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
            message: format!("{error} — réglages par défaut appliqués"),
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

/// L'indentation de la ligne qui contient `offset`.
fn indentation<'a>(text: &'a str, map: &LineMap, offset: TextSize) -> (TextSize, &'a str) {
    let start = map.line_start(map.line(offset)).unwrap_or_default();
    let line = &text[usize::from(start)..];
    let width = line.len() - line.trim_start_matches([' ', '\t']).len();
    (start, &line[..width])
}

/// Les actions proposées sur `range` : correctifs sûrs, suggestions,
/// suppression, et la correction de tout le fichier.
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
                Some(first) => format!("Renommer en `{}`", first.text),
                None => "Corriger".to_owned(),
            };
            quickfix(title, text_edits(fix, map), true);
        }
        if let Some(suggestion) = &lint.suggestion {
            quickfix(
                suggestion.title.clone(),
                text_edits(&suggestion.edits, map),
                false,
            );
        }
        let (start, indent) = indentation(text, map, lint.range.start());
        let at = map.position(start);
        quickfix(
            format!("Autoriser `{}` sur cette ligne", lint.lint.name),
            vec![TextEdit::new(
                Range::new(at, at),
                format!("{indent}; papyrus: allow({})\n", lint.lint.name),
            )],
            false,
        );
    }

    // Tous les correctifs sûrs du fichier, d'un coup : `source.fixAll`, que
    // les éditeurs savent appliquer à l'enregistrement.
    let fixable = analysis.lints.iter().any(|d| d.fix.is_some());
    if fixable && analysis.parse.ok() && !document.decoded.lossy {
        if let Some(fixed) = apply_fixes(text, &analysis.lints) {
            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: "Appliquer tous les correctifs sûrs de papyrus".to_owned(),
                kind: Some(CodeActionKind::SOURCE_FIX_ALL),
                edit: Some(edit(uri, vec![whole(text, map, fixed)])),
                ..CodeAction::default()
            }));
        }
    }
    actions
}

/// Remplace tout le texte.
fn whole(text: &str, map: &LineMap, new_text: String) -> TextEdit {
    let end = map.position(TextSize::of(text));
    TextEdit::new(Range::new(Position::new(0, 0), end), new_text)
}

/// Le document formaté, ou `None` s'il l'est déjà.
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
        // Les erreurs de syntaxe sont déjà affichées : rien à ajouter.
        Err(FormatError::Syntax(_)) => Ok(None),
        Err(error @ FormatError::Unsafe { .. }) => {
            Err(format!("{error} — merci de signaler ce bug"))
        }
    }
}
