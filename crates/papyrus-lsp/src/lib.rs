//! The Papyrus language server: diagnostics, outline, folding, highlighting,
//! formatting and fixes, definitions, hovers, completion, references,
//! renames, workspace symbols, signature help and parameter name hints, for
//! VSCode, IntelliJ (through LSP4IJ) and any editor that speaks LSP.
//!
//! ```sh
//! papyrus lsp
//! ```
//!
//! On startup, the projects of the workspace folders are indexed in the
//! background, with the scripts of their `imports` folders: enough to follow
//! `extends` and calls from one file to another and to deduce the target
//! game when papyrus.toml does not say it. A window may hold several
//! projects, each with its own `papyrus.toml`: they are kept apart.
//!
//! Every script read feeds a `papyrus_hir` database; open documents replace
//! their on-disk text there as they are edited.
//!
//! The parameter name hints follow `inlayHints.parameterNames`, `none`,
//! `literals` (the default) or `all`, in the initialization options or in
//! the `papyrus` section of the configuration.

#![forbid(unsafe_code)]

mod analysis;
mod navigation;
mod position;
mod semantic;
mod symbols;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use papyrus_dialect::{Dialect, GameId};
use papyrus_hir::{Database, ParameterHints, Semantics, SourceFile};
use papyrus_lint::{Model, ScriptSummary, Workspace};
use papyrus_project::{ConfigLocator, decode};
use papyrus_syntax::{TextRange, TextSize};
use salsa::Setter;
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use analysis::{Analysis, Document, Settings, analyze};
use navigation::Navigator;
use position::{Columns, LineMap};

/// What the server knows about one project: the folder of a `papyrus.toml`,
/// or a workspace folder without one.
///
/// Projects are sealed: a document only sees the scripts of its own
/// project, and only its scripts vote on its game.
#[derive(Debug, Default)]
struct Project {
    /// Its scripts. Open documents replace their on-disk version there.
    workspace: Workspace,
    /// The target deduced from its scripts.
    detected: Option<GameId>,
    /// The target its papyrus.toml sets.
    configured: Option<GameId>,
    /// Its model: own scripts, imported ones, target.
    model: Option<papyrus_hir::Project>,
}

/// The server's shared state.
#[derive(Debug, Default)]
struct State {
    columns: Columns,
    /// The workspace folders, absolute.
    folders: Vec<PathBuf>,
    documents: HashMap<Uri, Document>,
    /// The projects, by root.
    projects: HashMap<PathBuf, Project>,
    /// The script name each open document declared in its project.
    declared: HashMap<Uri, String>,
    locator: ConfigLocator,
    /// The client can watch files at the server's request.
    can_watch: bool,
    /// The semantic model.
    db: Database,
    /// The model's file of each script read, by path; a document without a
    /// path is keyed by its URI.
    sources: HashMap<PathBuf, SourceFile>,
    /// Which arguments get the name of their parameter.
    hints: ParameterHints,
}

/// `inlayHints.parameterNames` in `settings`, or in its `papyrus` section.
fn parameter_hints(settings: &serde_json::Value) -> Option<ParameterHints> {
    let section = settings.get("papyrus").unwrap_or(settings);
    match section.pointer("/inlayHints/parameterNames")?.as_str()? {
        "none" => Some(ParameterHints::None),
        "literals" => Some(ParameterHints::Literals),
        "all" => Some(ParameterHints::All),
        _ => None,
    }
}

/// The key of a document in [`State::sources`].
fn key(uri: &Uri, document: &Document) -> PathBuf {
    document
        .path
        .clone()
        .unwrap_or_else(|| PathBuf::from(uri.as_str()))
}

impl State {
    /// The root of the project `path` belongs to: the folder of the
    /// `papyrus.toml` covering it, else the workspace folder holding it,
    /// else its own folder.
    fn root_of(&mut self, path: Option<&Path>) -> PathBuf {
        let Some(path) = path else {
            return PathBuf::new();
        };
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
        if let Some(config) = self.locator.locate(&path) {
            return config.parent().unwrap_or(&config).to_owned();
        }
        self.folders
            .iter()
            .filter(|folder| path.starts_with(folder))
            .max_by_key(|folder| folder.as_os_str().len())
            .cloned()
            .or_else(|| path.parent().map(Path::to_owned))
            .unwrap_or_default()
    }

    /// The model's file for `path`, created or brought up to `text`.
    fn source(&mut self, path: &Path, text: String) -> SourceFile {
        match self.sources.get(path) {
            Some(&file) => {
                if file.text(&self.db) != text {
                    file.set_text(&mut self.db).to(text);
                }
                file
            }
            None => {
                let file = SourceFile::new(&self.db, path.to_owned(), text);
                self.sources.insert(path.to_owned(), file);
                file
            }
        }
    }

    /// The target of a project: papyrus.toml's, else the inferred one.
    fn target(project: &Project) -> Option<GameId> {
        project.configured.or(project.detected)
    }

    /// Makes `file` one of the scripts of the project at `root`.
    fn attach(&mut self, root: &Path, file: SourceFile) {
        let project = self.projects.entry(root.to_owned()).or_default();
        let game = State::target(project);
        match project.model {
            Some(model) => {
                if !model.files(&self.db).contains(&file) {
                    let mut files = model.files(&self.db).to_vec();
                    files.push(file);
                    model.set_files(&mut self.db).to(files);
                }
            }
            None => {
                project.model = Some(papyrus_hir::Project::new(
                    &self.db,
                    vec![file],
                    Vec::new(),
                    game,
                ));
            }
        }
    }

    /// Feeds the text of document `uri` to the model.
    fn feed(&mut self, uri: &Uri) {
        let Some(document) = self.documents.get(uri) else {
            return;
        };
        let (path, root, text) = (
            key(uri, document),
            document.project.clone(),
            document.decoded.text.clone(),
        );
        let file = self.source(&path, text);
        self.attach(&root, file);
    }

    /// The model's file of document `uri`, and its project's model.
    fn model(&self, uri: &Uri) -> Option<(SourceFile, papyrus_hir::Project, &Document)> {
        let document = self.documents.get(uri)?;
        let file = *self.sources.get(&key(uri, document))?;
        let model = self.projects.get(&document.project)?.model?;
        Some((file, model, document))
    }

    /// The model's file of `uri`, open or not, and its project's model.
    fn locate(&self, uri: &Uri) -> Option<(SourceFile, papyrus_hir::Project)> {
        if let Some((file, model, _)) = self.model(uri) {
            return Some((file, model));
        }
        let file = *self.sources.get(&uri.to_file_path()?.into_owned())?;
        let model =
            self.projects.values().filter_map(|p| p.model).find(|m| {
                m.files(&self.db).contains(&file) || m.imports(&self.db).contains(&file)
            })?;
        Some((file, model))
    }

    /// The URI of a file of the model.
    fn uri_of(&self, file: SourceFile) -> Option<Uri> {
        let path = file.path(&self.db);
        self.documents
            .iter()
            .find(|(uri, document)| key(uri, document) == *path)
            .map(|(uri, _)| uri.clone())
            .or_else(|| Uri::from_file_path(path))
    }

    /// Runs `f` with the model of document `uri` and the offset of
    /// `position` in it.
    fn navigate<T>(
        &self,
        uri: &Uri,
        position: Position,
        f: impl FnOnce(&Navigator<'_>, papyrus_hir::Project, SourceFile, TextSize) -> T,
    ) -> Option<T> {
        let (file, model) = self.locate(uri)?;
        let offset = LineMap::new(file.text(&self.db), self.columns).offset(position)?;
        let uri_of = |file| self.uri_of(file);
        let navigator = Navigator {
            db: &self.db,
            columns: self.columns,
            uri: &uri_of,
        };
        Some(f(&navigator, model, file, offset))
    }

    /// Analyzes `document` within its project, for its tree.
    fn analyze(&self, document: &Document) -> Analysis {
        let empty = Project::default();
        let project = self.projects.get(&document.project).unwrap_or(&empty);
        analyze(document, &project.workspace, project.detected, None)
    }

    /// Analyzes document `uri` within its project and its semantic model,
    /// for its diagnostics.
    fn lint(&self, uri: &Uri, document: &Document) -> Analysis {
        let empty = Project::default();
        let project = self.projects.get(&document.project).unwrap_or(&empty);
        let model = self.model(uri).map(|(file, model, _)| Model {
            semantics: Semantics::new(&self.db, model),
            file,
        });
        analyze(document, &project.workspace, project.detected, model)
    }

    /// Updates the summary of document `uri` in its project; true if it
    /// changed, and the other documents of the project need another look.
    fn declare(&mut self, uri: &Uri, summary: Option<ScriptSummary>) -> bool {
        let Some(root) = self.documents.get(uri).map(|d| d.project.clone()) else {
            return false;
        };
        let previous = self.declared.get(uri).cloned();
        let workspace = &mut self.projects.entry(root).or_default().workspace;
        match summary {
            Some(summary) => {
                if workspace.get(&summary.name) == Some(&summary)
                    && previous.as_deref() == Some(summary.name.as_str())
                {
                    return false;
                }
                if let Some(previous) = previous.filter(|p| !p.eq_ignore_ascii_case(&summary.name))
                {
                    workspace.remove(&previous);
                }
                self.declared.insert(uri.clone(), summary.name.clone());
                workspace.insert(summary);
                true
            }
            None => {
                if let Some(previous) = self.declared.remove(uri) {
                    workspace.remove(&previous);
                    return true;
                }
                false
            }
        }
    }

    /// The diagnostics of `uri`.
    fn diagnostics(&self, uri: &Uri) -> Option<(Uri, Vec<Diagnostic>, i32)> {
        let document = self.documents.get(uri)?;
        let analysis = self.lint(uri, document);
        let map = LineMap::new(&document.decoded.text, self.columns);
        Some((
            uri.clone(),
            analysis::diagnostics(document, &analysis, &map),
            document.version,
        ))
    }

    /// Reanalyzes `uri` after a change, and the other open documents of its
    /// project: a signature they call may have changed.
    fn refresh(&mut self, uri: &Uri) -> Vec<(Uri, Vec<Diagnostic>, i32)> {
        let Some(document) = self.documents.get(uri) else {
            return Vec::new();
        };
        let root = document.project.clone();
        let parse = papyrus_syntax::parse(&document.decoded.text, Dialect::new(GameId::Starfield));
        self.declare(uri, ScriptSummary::of(&parse.syntax()));
        self.documents
            .iter()
            .filter(|(_, d)| d.project == root)
            .filter_map(|(uri, _)| self.diagnostics(uri))
            .collect()
    }

    /// The diagnostics of every open document.
    fn all(&self) -> Vec<(Uri, Vec<Diagnostic>, i32)> {
        self.documents
            .keys()
            .filter_map(|uri| self.diagnostics(uri))
            .collect()
    }
}

/// What indexing one project found.
struct Scanned {
    root: PathBuf,
    summaries: Vec<ScriptSummary>,
    detected: Option<GameId>,
    configured: Option<GameId>,
    /// `skse = true` in its papyrus.toml.
    skse: bool,
    /// Its scripts and their text.
    scripts: Vec<(PathBuf, String)>,
    /// The scripts of its `imports` folders and their text.
    imported: Vec<(PathBuf, String)>,
}

/// The text of a script on disk.
fn read(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| decode(&bytes).text)
}

/// Indexes the projects under `folders`, as `papyrus check` would; a folder
/// that cannot be read is reported and skipped.
fn scan(folders: &[PathBuf]) -> (Vec<Scanned>, Vec<String>) {
    let widest = Dialect::new(GameId::Starfield);
    let mut scanned = Vec::new();
    let mut errors = Vec::new();
    for folder in folders {
        let projects = match papyrus_project::projects(std::slice::from_ref(folder), false) {
            Ok(projects) => projects,
            Err(error) => {
                errors.push(error.to_string());
                continue;
            }
        };
        for project in projects {
            let mut summaries = Vec::new();
            let mut verdicts = Vec::new();
            let mut scripts = Vec::new();
            for path in project.scripts {
                let Some(text) = read(&path) else {
                    continue;
                };
                let root = papyrus_syntax::parse(&text, widest).syntax();
                verdicts.push(papyrus_project::detect::evidence(&root).verdict());
                summaries.extend(ScriptSummary::of(&root));
                scripts.push((path, text));
            }
            // Imported scripts relay `extends`, but do not vote on the game.
            let mut imported = Vec::new();
            for path in project.imported {
                let Some(text) = read(&path) else {
                    continue;
                };
                let root = papyrus_syntax::parse(&text, widest).syntax();
                let summary = ScriptSummary::of(&root);
                if summary.as_ref().is_some_and(|s| {
                    !summaries
                        .iter()
                        .any(|o| o.name.eq_ignore_ascii_case(&s.name))
                }) {
                    summaries.extend(summary);
                }
                imported.push((path, text));
            }
            scanned.push(Scanned {
                root: project.root,
                summaries,
                detected: papyrus_project::detect::majority(verdicts).map(|(game, _, _)| game),
                configured: project.config.game,
                skse: project.config.skse,
                scripts,
                imported,
            });
        }
    }
    (scanned, errors)
}

/// The summary of a script's text.
fn summary_of(text: &str) -> Option<ScriptSummary> {
    let root = papyrus_syntax::parse(text, Dialect::new(GameId::Starfield)).syntax();
    ScriptSummary::of(&root)
}

/// The server.
#[derive(Debug, Clone)]
pub struct Backend {
    client: Client,
    state: Arc<Mutex<State>>,
}

impl Backend {
    /// A server answering `client`.
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            state: Arc::default(),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic under the lock leaves a consistent state: every update is
        // made in one block.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn publish(&self, batch: Vec<(Uri, Vec<Diagnostic>, i32)>) {
        for (uri, diagnostics, version) in batch {
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }

    /// Indexes the projects of the workspace folders in the background, then
    /// republishes every open document.
    fn rescan(&self) {
        let folders = self.state().folders.clone();
        if folders.is_empty() {
            return;
        }
        let backend = self.clone();
        tokio::spawn(async move {
            let Ok((scanned, errors)) = tokio::task::spawn_blocking(move || scan(&folders)).await
            else {
                return;
            };
            let mut log: Vec<String> = errors
                .into_iter()
                .map(|e| format!("papyrus: {e}"))
                .collect();
            let batch = {
                let mut state = backend.state();
                // Open documents take precedence over the disk.
                let open: HashMap<PathBuf, String> = state
                    .documents
                    .iter()
                    .map(|(uri, d)| (key(uri, d), d.decoded.text.clone()))
                    .collect();
                let mut projects: HashMap<PathBuf, Project> = HashMap::new();
                for project in scanned {
                    let target = match project.detected {
                        Some(game) => format!("inferred target {}", game.display_name()),
                        None => "unknown target".to_owned(),
                    };
                    log.push(format!(
                        "papyrus: {}: {} scripts indexed, {} imported, {target}",
                        project.root.display(),
                        project.scripts.len(),
                        project.imported.len()
                    ));
                    let mut workspace = Workspace::default();
                    for summary in project.summaries {
                        workspace.insert(summary);
                    }
                    let mut feed = |list: Vec<(PathBuf, String)>| -> Vec<SourceFile> {
                        list.into_iter()
                            .map(|(path, text)| {
                                let text = open.get(&path).cloned().unwrap_or(text);
                                state.source(&path, text)
                            })
                            .collect()
                    };
                    let files = feed(project.scripts);
                    let imports = feed(project.imported);
                    let game = project.configured.or(project.detected);
                    let model = papyrus_hir::Project::new(&state.db, files, imports, game);
                    model.set_skse(&mut state.db).to(project.skse);
                    projects.insert(
                        project.root,
                        Project {
                            workspace,
                            detected: project.detected,
                            configured: project.configured,
                            model: Some(model),
                        },
                    );
                }
                // Open documents take precedence over the disk.
                for (uri, name) in &state.declared {
                    let Some(document) = state.documents.get(uri) else {
                        continue;
                    };
                    let open = state
                        .projects
                        .get(&document.project)
                        .and_then(|p| p.workspace.get(name));
                    if let Some(open) = open {
                        projects
                            .entry(document.project.clone())
                            .or_default()
                            .workspace
                            .insert(open.clone());
                    }
                }
                state.projects = projects;
                // Open documents outside every scanned project keep a model.
                let uris: Vec<Uri> = state.documents.keys().cloned().collect();
                for uri in &uris {
                    state.feed(uri);
                }
                state.all()
            };
            for line in log {
                backend.client.log_message(MessageType::INFO, line).await;
            }
            backend.publish(batch).await;
        });
    }

    /// Runs `f` on document `uri` and the view of its lines.
    fn with_document<T>(
        &self,
        uri: &Uri,
        f: impl FnOnce(&State, &Document, &LineMap) -> T,
    ) -> Option<T> {
        let state = self.state();
        let document = state.documents.get(uri)?;
        let map = LineMap::new(&document.decoded.text, state.columns);
        Some(f(&state, document, &map))
    }
}

fn file_path(uri: &Uri) -> Option<PathBuf> {
    (uri.scheme().as_str() == "file")
        .then(|| uri.to_file_path().map(|p| p.into_owned()))
        .flatten()
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let utf8 = params
            .capabilities
            .general
            .as_ref()
            .and_then(|g| g.position_encodings.as_ref())
            .is_some_and(|e| e.contains(&PositionEncodingKind::UTF8));
        let mut folders: Vec<PathBuf> = params
            .workspace_folders
            .iter()
            .flatten()
            .filter_map(|f| file_path(&f.uri))
            .collect();
        #[allow(deprecated)] // Clients without multiple folders.
        if folders.is_empty() {
            folders.extend(params.root_uri.as_ref().and_then(file_path));
        }
        for folder in &mut folders {
            if let Ok(absolute) = std::path::absolute(&*folder) {
                *folder = absolute;
            }
        }
        let can_watch = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.did_change_watched_files.as_ref())
            .and_then(|w| w.dynamic_registration)
            .unwrap_or(false);
        {
            let mut state = self.state();
            if let Some(hints) = params
                .initialization_options
                .as_ref()
                .and_then(parameter_hints)
            {
                state.hints = hints;
            }
            state.columns = if utf8 { Columns::Utf8 } else { Columns::Utf16 };
            state.folders = folders;
            state.can_watch = can_watch;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(if utf8 {
                    PositionEncodingKind::UTF8
                } else {
                    PositionEncodingKind::UTF16
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..TextDocumentSyncOptions::default()
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                inlay_hint_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_owned()]),
                    ..CompletionOptions::default()
                }),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(true),
                }),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic::legend(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..SemanticTokensOptions::default()
                        },
                    ),
                ),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::SOURCE_FIX_ALL,
                        ]),
                        ..CodeActionOptions::default()
                    },
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "papyrus".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // The configuration, the flags, and scripts modified outside the
        // editor: the server asks to be notified, whatever the editor.
        let can_watch = self.state().can_watch;
        if can_watch {
            let watchers = ["**/papyrus.toml", "**/*.flg", "**/*.psc"]
                .into_iter()
                .map(|glob| FileSystemWatcher {
                    glob_pattern: GlobPattern::String(glob.to_owned()),
                    kind: None,
                })
                .collect();
            let options = DidChangeWatchedFilesRegistrationOptions { watchers };
            let registration = Registration {
                id: "papyrus-watch".to_owned(),
                method: "workspace/didChangeWatchedFiles".to_owned(),
                register_options: serde_json::to_value(options).ok(),
            };
            if let Err(error) = self.client.register_capability(vec![registration]).await {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("papyrus: file watching refused ({error})"),
                    )
                    .await;
            }
        }
        self.rescan();
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let item = params.text_document;
        let batch = {
            let mut state = self.state();
            let path = file_path(&item.uri);
            let mut document = Document::new(item.text, item.version, path.clone());
            document.project = state.root_of(path.as_deref());
            state.documents.insert(item.uri.clone(), document);
            state.feed(&item.uri);
            state.refresh(&item.uri)
        };
        self.publish(batch).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // Full sync: the last change is the text.
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        let batch = {
            let mut state = self.state();
            let Some(document) = state.documents.get_mut(&uri) else {
                return;
            };
            document.decoded.text = change.text;
            document.version = params.text_document.version;
            state.feed(&uri);
            state.refresh(&uri)
        };
        self.publish(batch).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let batch = {
            let mut state = self.state();
            let Some(document) = state.documents.get_mut(&uri) else {
                return;
            };
            // The editor may have changed the encoding when saving.
            document.reload_encoding();
            state.diagnostics(&uri).into_iter().collect()
        };
        self.publish(batch).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut state = self.state();
            // The file stays in the index, with its text on disk: it is still
            // part of its project.
            if let Some(document) = state.documents.remove(&uri) {
                if let Some(text) = document.path.as_deref().and_then(read) {
                    let path = key(&uri, &document);
                    state.source(&path, text);
                }
            }
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let (settings_changed, batch) = {
            let mut state = self.state();
            let mut settings_changed = false;
            for change in &params.changes {
                let Some(path) = file_path(&change.uri) else {
                    continue;
                };
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if path
                    .file_name()
                    .is_some_and(|n| n == papyrus_project::config::FILE_NAME)
                    || extension.eq_ignore_ascii_case("flg")
                {
                    settings_changed = true;
                } else if extension.eq_ignore_ascii_case("psc")
                    && !state.documents.contains_key(&change.uri)
                {
                    // A script modified outside the editor.
                    if let Some(text) = read(&path) {
                        let root = state.root_of(Some(&path));
                        if let Some(summary) = summary_of(&text) {
                            state
                                .projects
                                .entry(root.clone())
                                .or_default()
                                .workspace
                                .insert(summary);
                        }
                        let known = state.sources.contains_key(&path);
                        let file = state.source(&path, text);
                        // An imported script is already part of its project.
                        if !known {
                            state.attach(&root, file);
                        }
                    }
                }
            }
            if settings_changed {
                // A new or removed papyrus.toml moves project borders.
                state.locator = ConfigLocator::default();
                let uris: Vec<Uri> = state.documents.keys().cloned().collect();
                for uri in uris {
                    let path = state.documents[&uri].path.clone();
                    let root = state.root_of(path.as_deref());
                    if let Some(document) = state.documents.get_mut(&uri) {
                        document.settings = Settings::load(path.as_deref());
                        document.project = root;
                    }
                }
            }
            (settings_changed, state.all())
        };
        self.publish(batch).await;
        if settings_changed {
            self.rescan();
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(
            self.with_document(&params.text_document.uri, |state, document, map| {
                let analysis = state.analyze(document);
                DocumentSymbolResponse::Nested(symbols::document_symbols(
                    &analysis.parse.syntax(),
                    map,
                ))
            }),
        )
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        Ok(
            self.with_document(&params.text_document.uri, |state, document, map| {
                let analysis = state.analyze(document);
                symbols::folding_ranges(&analysis.parse.syntax(), map)
            }),
        )
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        Ok(
            self.with_document(&params.text_document.uri, |state, document, map| {
                let analysis = state.analyze(document);
                SemanticTokensResult::Tokens(SemanticTokens {
                    result_id: None,
                    data: semantic::tokens(&analysis.parse.syntax(), map),
                })
            }),
        )
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        Ok(self.with_document(&uri, |state, document, map| {
            let analysis = state.lint(&uri, document);
            let Some(range) = map.text_range(params.range) else {
                return Vec::new();
            };
            analysis::code_actions(&uri, document, &analysis, map, range)
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let at = params.text_document_position_params;
        let state = self.state();
        Ok(state
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.definition(project, file, offset),
            )
            .flatten()
            .map(GotoDefinitionResponse::Scalar))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let at = params.text_document_position_params;
        let state = self.state();
        Ok(state
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.hover(project, file, offset),
            )
            .flatten())
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let at = params.text_document_position_params;
        let state = self.state();
        Ok(state
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.signature_help(project, file, offset),
            )
            .flatten())
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let state = self.state();
        let Some((file, model, document)) = state.model(&params.text_document.uri) else {
            return Ok(None);
        };
        let text = &document.decoded.text;
        let map = LineMap::new(text, state.columns);
        // A range past the end, as some editors send, runs to the end.
        let start = map.offset(params.range.start).unwrap_or_default();
        let end = map
            .offset(params.range.end)
            .unwrap_or_else(|| TextSize::of(text.as_str()));
        let range = TextRange::new(start, end.max(start));
        let uri_of = |file| state.uri_of(file);
        let navigator = Navigator {
            db: &state.db,
            columns: state.columns,
            uri: &uri_of,
        };
        Ok(Some(navigator.inlay_hints(model, file, range, state.hints)))
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let Some(hints) = parameter_hints(&params.settings) else {
            return;
        };
        self.state().hints = hints;
        // Clients that cannot refresh ask again on their own.
        let _ = self.client.inlay_hint_refresh().await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let at = params.text_document_position;
        let state = self.state();
        Ok(state
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.completion(project, file, offset),
            )
            .map(CompletionResponse::Array))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let at = params.text_document_position;
        let include = params.context.include_declaration;
        let state = self.state();
        Ok(state
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.references(project, file, offset, include),
            )
            .flatten())
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let at = params.text_document_position_params;
        Ok(self
            .state()
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.highlights(project, file, offset),
            )
            .flatten())
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let at = params.text_document_position_params;
        Ok(self
            .state()
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.implementations(project, file, offset),
            )
            .flatten()
            .map(GotoImplementationResponse::Array))
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let at = params.text_document_position_params;
        Ok(self
            .state()
            .navigate(
                &at.text_document.uri,
                at.position,
                |nav, project, file, offset| nav.prepare_call_hierarchy(project, file, offset),
            )
            .flatten())
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let item = params.item;
        Ok(self
            .state()
            .navigate(
                &item.uri,
                item.selection_range.start,
                |nav, project, file, offset| nav.incoming_calls(project, file, offset),
            )
            .flatten())
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let item = params.item;
        Ok(self
            .state()
            .navigate(
                &item.uri,
                item.selection_range.start,
                |nav, project, file, offset| nav.outgoing_calls(project, file, offset),
            )
            .flatten())
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let state = self.state();
        Ok(
            state.navigate(&uri, Position::new(0, 0), |nav, project, file, _| {
                nav.code_lenses(project, file, &uri)
            }),
        )
    }

    async fn code_lens_resolve(&self, lens: CodeLens) -> Result<CodeLens> {
        let uri = lens
            .data
            .as_ref()
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .and_then(|u| u.parse::<Uri>().ok());
        let Some(uri) = uri else {
            return Ok(lens);
        };
        let position = lens.range.start;
        let command = self
            .state()
            .navigate(&uri, position, |nav, project, file, offset| {
                nav.lens_command(project, file, offset, &uri, position)
            });
        Ok(CodeLens { command, ..lens })
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let result = self.state().navigate(
            &params.text_document.uri,
            params.position,
            |nav, project, file, offset| nav.prepare_rename(project, file, offset),
        );
        match result {
            None => Ok(None),
            Some(Ok((range, placeholder))) => {
                Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
                    range,
                    placeholder,
                }))
            }
            Some(Err(message)) => Err(refused(message)),
        }
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let at = params.text_document_position;
        let result = self.state().navigate(
            &at.text_document.uri,
            at.position,
            |nav, project, file, offset| nav.rename(project, file, offset, &params.new_name),
        );
        match result {
            None => Ok(None),
            Some(Ok((edit, warning))) => {
                if let Some(warning) = warning {
                    self.client
                        .show_message(MessageType::WARNING, format!("papyrus: {warning}"))
                        .await;
                }
                Ok(Some(edit))
            }
            Some(Err(message)) => Err(refused(message)),
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        let state = self.state();
        let projects: Vec<papyrus_hir::Project> =
            state.projects.values().filter_map(|p| p.model).collect();
        let uri_of = |file| state.uri_of(file);
        let navigator = Navigator {
            db: &state.db,
            columns: state.columns,
            uri: &uri_of,
        };
        Ok(Some(WorkspaceSymbolResponse::Flat(
            navigator.symbols(&projects, &params.query),
        )))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let result = self.with_document(&params.text_document.uri, |state, document, map| {
            let analysis = state.analyze(document);
            analysis::format(document, &analysis, map)
        });
        match result {
            None => Ok(None),
            Some(Ok(edits)) => Ok(edits),
            Some(Err(message)) => {
                let mut error = Error::internal_error();
                error.message = message.into();
                Err(error)
            }
        }
    }
}

/// A request the server declines, with the reason for the user.
fn refused(message: String) -> Error {
    let mut error = Error::invalid_request();
    error.message = message.into();
    error
}

/// Serves the protocol on `input` and `output` until `exit`.
pub async fn serve<I, O>(input: I, output: O)
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite,
{
    let (service, socket) = LspService::new(Backend::new);
    Server::new(input, output, socket).serve(service).await;
}

/// Serves the protocol on standard input and output: `papyrus lsp`.
pub fn run() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(tokio::io::stdin(), tokio::io::stdout()));
    Ok(())
}
