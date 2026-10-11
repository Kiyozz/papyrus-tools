//! Le serveur LSP de Papyrus : diagnostics, plan, repli, coloration,
//! formatage et correctifs, pour VSCode, IntelliJ (via LSP4IJ) et tout
//! éditeur qui parle LSP.
//!
//! ```sh
//! papyrus lsp
//! ```
//!
//! À l'ouverture, les scripts des dossiers du projet sont indexés en tâche de
//! fond : de quoi suivre `extends` d'un fichier à l'autre et déduire le jeu
//! visé quand papyrus.toml ne le dit pas.

#![forbid(unsafe_code)]

mod analysis;
mod position;
mod semantic;
mod symbols;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use papyrus_dialect::{Dialect, GameId};
use papyrus_lint::{ScriptSummary, Workspace};
use papyrus_project::{Config, DiscoverOptions, decode, discover};
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};

use analysis::{Document, Settings, analyze};
use position::{Columns, LineMap};

/// L'état partagé du serveur.
#[derive(Debug, Default)]
struct State {
    columns: Columns,
    folders: Vec<PathBuf>,
    documents: HashMap<Uri, Document>,
    /// Les scripts du projet. Ceux des documents ouverts y remplacent la
    /// version du disque.
    workspace: Workspace,
    /// Le nom de script que chaque document ouvert a déclaré dans
    /// `workspace`.
    declared: HashMap<Uri, String>,
    /// La cible déduite des scripts du projet.
    detected: Option<GameId>,
    /// Le client sait surveiller des fichiers à la demande du serveur.
    can_watch: bool,
}

impl State {
    /// Met à jour le résumé du document `uri` dans l'index ; vrai s'il a
    /// changé, et que les autres documents sont à revoir.
    fn declare(&mut self, uri: &Uri, summary: Option<ScriptSummary>) -> bool {
        let previous = self.declared.get(uri).cloned();
        match summary {
            Some(summary) => {
                if self.workspace.get(&summary.name) == Some(&summary)
                    && previous.as_deref() == Some(summary.name.as_str())
                {
                    return false;
                }
                if let Some(previous) = previous.filter(|p| !p.eq_ignore_ascii_case(&summary.name))
                {
                    self.workspace.remove(&previous);
                }
                self.declared.insert(uri.clone(), summary.name.clone());
                self.workspace.insert(summary);
                true
            }
            None => {
                if let Some(previous) = self.declared.remove(uri) {
                    self.workspace.remove(&previous);
                    return true;
                }
                false
            }
        }
    }

    /// Les diagnostics de `uri`.
    fn diagnostics(&self, uri: &Uri) -> Option<(Uri, Vec<Diagnostic>, i32)> {
        let document = self.documents.get(uri)?;
        let analysis = analyze(document, &self.workspace, self.detected);
        let map = LineMap::new(&document.decoded.text, self.columns);
        Some((
            uri.clone(),
            analysis::diagnostics(document, &analysis, &map),
            document.version,
        ))
    }

    /// Réanalyse `uri` après une modification, et les autres documents
    /// ouverts si ce qu'il déclare a changé.
    fn refresh(&mut self, uri: &Uri) -> Vec<(Uri, Vec<Diagnostic>, i32)> {
        let Some(document) = self.documents.get(uri) else {
            return Vec::new();
        };
        let parse = papyrus_syntax::parse(&document.decoded.text, Dialect::new(GameId::Starfield));
        let changed = self.declare(uri, ScriptSummary::of(&parse.syntax()));
        if changed {
            self.all()
        } else {
            self.diagnostics(uri).into_iter().collect()
        }
    }

    /// Les diagnostics de tous les documents ouverts.
    fn all(&self) -> Vec<(Uri, Vec<Diagnostic>, i32)> {
        self.documents
            .keys()
            .filter_map(|uri| self.diagnostics(uri))
            .collect()
    }
}

/// Ce que l'indexation du projet a trouvé.
struct Scan {
    summaries: Vec<ScriptSummary>,
    detected: Option<GameId>,
    files: usize,
}

/// Indexe les scripts de `folders`, comme `papyrus check` le ferait.
fn scan(folders: &[PathBuf]) -> Scan {
    let widest = Dialect::new(GameId::Starfield);
    let mut summaries = Vec::new();
    let mut verdicts = Vec::new();
    let mut files = 0;
    for folder in folders {
        let config = Config::discover(folder).unwrap_or_default();
        let options = DiscoverOptions {
            include_fragments: config.include_fragments,
        };
        let Ok(scripts) = discover(std::slice::from_ref(folder), options) else {
            continue;
        };
        for path in scripts {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            files += 1;
            let root = papyrus_syntax::parse(&decode(&bytes).text, widest).syntax();
            verdicts.push(papyrus_project::detect::evidence(&root).verdict());
            summaries.extend(ScriptSummary::of(&root));
        }
    }
    Scan {
        summaries,
        detected: papyrus_project::detect::majority(verdicts).map(|(game, _, _)| game),
        files,
    }
}

/// Le serveur.
#[derive(Debug, Clone)]
pub struct Backend {
    client: Client,
    state: Arc<Mutex<State>>,
}

impl Backend {
    /// Un serveur qui répond à `client`.
    pub fn new(client: Client) -> Backend {
        Backend {
            client,
            state: Arc::default(),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // Un panic sous le verrou laisse un état cohérent : chaque mise à
        // jour est faite d'un bloc.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn publish(&self, batch: Vec<(Uri, Vec<Diagnostic>, i32)>) {
        for (uri, diagnostics, version) in batch {
            self.client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        }
    }

    /// Exécute `f` sur le document `uri` et la vue de ses lignes.
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
        #[allow(deprecated)] // Les clients sans dossiers multiples.
        if folders.is_empty() {
            folders.extend(params.root_uri.as_ref().and_then(file_path));
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
        let (folders, can_watch) = {
            let state = self.state();
            (state.folders.clone(), state.can_watch)
        };
        // La configuration, les flags, et les scripts modifiés hors de
        // l'éditeur : le serveur demande à en être prévenu, quel que soit
        // l'éditeur.
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
                        format!("papyrus : surveillance des fichiers refusée ({error})"),
                    )
                    .await;
            }
        }
        if folders.is_empty() {
            return;
        }
        let backend = self.clone();
        tokio::spawn(async move {
            let Ok(scan) = tokio::task::spawn_blocking(move || scan(&folders)).await else {
                return;
            };
            let (count, files, detected) = (scan.summaries.len(), scan.files, scan.detected);
            let batch = {
                let mut state = backend.state();
                let mut workspace = Workspace::default();
                for summary in scan.summaries {
                    workspace.insert(summary);
                }
                // Les documents ouverts priment sur le disque.
                for name in state.declared.values() {
                    if let Some(open) = state.workspace.get(name) {
                        workspace.insert(open.clone());
                    }
                }
                state.workspace = workspace;
                state.detected = scan.detected;
                state.all()
            };
            let target = match detected {
                Some(game) => format!("cible déduite : {}", game.display_name()),
                None => "cible inconnue".to_owned(),
            };
            backend
                .client
                .log_message(
                    MessageType::INFO,
                    format!("papyrus : {count} scripts indexés sur {files} fichiers, {target}"),
                )
                .await;
            backend.publish(batch).await;
        });
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let item = params.text_document;
        let batch = {
            let mut state = self.state();
            let document = Document::new(item.text, item.version, file_path(&item.uri));
            state.documents.insert(item.uri.clone(), document);
            state.refresh(&item.uri)
        };
        self.publish(batch).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        // Synchronisation complète : le dernier changement est le texte.
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
            // L'éditeur a pu changer l'encodage en enregistrant.
            document.reload_encoding();
            state.diagnostics(&uri).into_iter().collect()
        };
        self.publish(batch).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.state().documents.remove(&uri);
        // Le fichier reste dans l'index : il fait toujours partie du projet.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let batch = {
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
                    // Un script modifié hors de l'éditeur.
                    let summary = std::fs::read(&path).ok().and_then(|bytes| {
                        let root = papyrus_syntax::parse(
                            &decode(&bytes).text,
                            Dialect::new(GameId::Starfield),
                        )
                        .syntax();
                        ScriptSummary::of(&root)
                    });
                    if let Some(summary) = summary {
                        state.workspace.insert(summary);
                    }
                }
            }
            if settings_changed {
                for document in state.documents.values_mut() {
                    document.settings = Settings::load(document.path.as_deref());
                }
            }
            state.all()
        };
        self.publish(batch).await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(
            self.with_document(&params.text_document.uri, |state, document, map| {
                let analysis = analyze(document, &state.workspace, state.detected);
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
                let analysis = analyze(document, &state.workspace, state.detected);
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
                let analysis = analyze(document, &state.workspace, state.detected);
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
            let analysis = analyze(document, &state.workspace, state.detected);
            let Some(range) = map.text_range(params.range) else {
                return Vec::new();
            };
            analysis::code_actions(&uri, document, &analysis, map, range)
        }))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let result = self.with_document(&params.text_document.uri, |state, document, map| {
            let analysis = analyze(document, &state.workspace, state.detected);
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

/// Sert le protocole sur `input` et `output` jusqu'à `exit`.
pub async fn serve<I, O>(input: I, output: O)
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite,
{
    let (service, socket) = LspService::new(Backend::new);
    Server::new(input, output, socket).serve(service).await;
}

/// Sert le protocole sur l'entrée et la sortie standard : `papyrus lsp`.
pub fn run() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(tokio::io::stdin(), tokio::io::stdout()));
    Ok(())
}
