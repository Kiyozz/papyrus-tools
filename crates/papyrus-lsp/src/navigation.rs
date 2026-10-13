//! Cross-file features: definition, hover, completion, references, rename,
//! workspace symbols. The model answers in files and byte ranges; this
//! module turns them into LSP types.

use std::collections::HashMap;

use papyrus_hir::{
    CompletionKind, Database, ItemKind, NavTarget, Project, Reference, Semantics, SourceFile,
};
use papyrus_syntax::{TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, Hover, HoverContents, Location, MarkupContent, MarkupKind,
    Range, SymbolInformation, SymbolKind, TextEdit, Uri, WorkspaceEdit,
};

use crate::position::{Columns, LineMap};

/// What the features need: the model, and how to name its files.
pub(crate) struct Navigator<'a> {
    pub(crate) db: &'a Database,
    pub(crate) columns: Columns,
    /// The URI of a file, `None` if it cannot be named.
    pub(crate) uri: &'a dyn Fn(SourceFile) -> Option<Uri>,
}

impl Navigator<'_> {
    fn map(&self, file: SourceFile) -> LineMap {
        LineMap::new(file.text(self.db), self.columns)
    }

    fn range(&self, file: SourceFile, range: TextRange) -> Range {
        self.map(file).range(range)
    }

    fn location(&self, file: SourceFile, range: TextRange) -> Option<Location> {
        Some(Location::new((self.uri)(file)?, self.range(file, range)))
    }

    pub(crate) fn definition(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Location> {
        let NavTarget {
            file, focus_range, ..
        } = Semantics::new(self.db, project).definition(file, offset)?;
        self.location(file, focus_range)
    }

    pub(crate) fn hover(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Hover> {
        let hover = Semantics::new(self.db, project).hover(file, offset)?;
        let mut value = format!("```papyrus\n{}\n```", hover.signature);
        if let Some(doc) = &hover.doc {
            value.push_str("\n\n");
            value.push_str(doc);
        }
        if let Some(origin) = &hover.origin {
            value.push_str(&format!("\n\n*{origin}*"));
        }
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(self.range(file, hover.range)),
        })
    }

    pub(crate) fn completion(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Vec<CompletionItem> {
        Semantics::new(self.db, project)
            .completions(file, offset)
            .into_iter()
            .map(|c| CompletionItem {
                kind: Some(match c.kind {
                    CompletionKind::Local | CompletionKind::Variable => {
                        CompletionItemKind::VARIABLE
                    }
                    CompletionKind::Property => CompletionItemKind::PROPERTY,
                    CompletionKind::Function => CompletionItemKind::FUNCTION,
                    CompletionKind::Event | CompletionKind::CustomEvent => {
                        CompletionItemKind::EVENT
                    }
                    CompletionKind::Field => CompletionItemKind::FIELD,
                    CompletionKind::Struct => CompletionItemKind::STRUCT,
                    CompletionKind::Script => CompletionItemKind::CLASS,
                    CompletionKind::Type => CompletionItemKind::TYPE_PARAMETER,
                    CompletionKind::Keyword => CompletionItemKind::KEYWORD,
                }),
                detail: c.detail,
                label: c.label,
                ..CompletionItem::default()
            })
            .collect()
    }

    fn locations(&self, references: Vec<Reference>) -> Vec<Location> {
        references
            .into_iter()
            .filter_map(|r| self.location(r.file, r.range))
            .collect()
    }

    pub(crate) fn references(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
        let found =
            Semantics::new(self.db, project).references(file, offset, include_declaration)?;
        Some(self.locations(found))
    }

    /// The range of the name to rename and its current text.
    pub(crate) fn prepare_rename(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Result<(Range, String), String> {
        let sema = Semantics::new(self.db, project);
        let (range, def) = sema.prepare_rename(file, offset).map_err(|e| e.0)?;
        Ok((self.range(file, range), sema.def_name(&def).to_owned()))
    }

    /// The edits of a rename, and the warning to show.
    pub(crate) fn rename(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
        new_name: &str,
    ) -> Result<(WorkspaceEdit, Option<String>), String> {
        let edit = Semantics::new(self.db, project)
            .rename(file, offset, new_name)
            .map_err(|e| e.0)?;
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for reference in edit.edits {
            let uri = (self.uri)(reference.file)
                .ok_or_else(|| format!("{} has no URI", reference.file.path(self.db).display()))?;
            changes.entry(uri).or_default().push(TextEdit::new(
                self.range(reference.file, reference.range),
                edit.new_name.clone(),
            ));
        }
        Ok((
            WorkspaceEdit {
                changes: Some(changes),
                ..WorkspaceEdit::default()
            },
            edit.warning,
        ))
    }

    #[allow(deprecated)] // `SymbolInformation::deprecated`, required by the type.
    pub(crate) fn symbols(&self, projects: &[Project], query: &str) -> Vec<SymbolInformation> {
        papyrus_hir::workspace_symbols(self.db, projects, query)
            .into_iter()
            .filter_map(|symbol| {
                Some(SymbolInformation {
                    kind: match symbol.kind {
                        papyrus_hir::SymbolKind::Script => SymbolKind::CLASS,
                        papyrus_hir::SymbolKind::Item(kind) => match kind {
                            ItemKind::Variable | ItemKind::Guard => SymbolKind::VARIABLE,
                            ItemKind::Property => SymbolKind::PROPERTY,
                            ItemKind::Function => SymbolKind::FUNCTION,
                            ItemKind::Event | ItemKind::CustomEvent => SymbolKind::EVENT,
                            ItemKind::State => SymbolKind::NAMESPACE,
                            ItemKind::Struct => SymbolKind::STRUCT,
                            ItemKind::Field => SymbolKind::FIELD,
                            ItemKind::Group => SymbolKind::PACKAGE,
                        },
                    },
                    location: self.location(symbol.file, symbol.range)?,
                    name: symbol.name,
                    container_name: symbol.container,
                    tags: None,
                    deprecated: None,
                })
            })
            .collect()
    }
}
