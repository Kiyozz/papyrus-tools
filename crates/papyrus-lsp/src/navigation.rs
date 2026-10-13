//! Cross-file features: definition, hover, completion, references, rename,
//! workspace symbols, signature help, inlay hints. The model answers in files and byte ranges; this
//! module turns them into LSP types.

use std::collections::HashMap;

use papyrus_hir::{
    CompletionKind, Database, ItemKind, NavTarget, ParameterHints, Project, Reference, Semantics,
    SourceFile,
};
use papyrus_syntax::{TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, Documentation, Hover, HoverContents, InlayHint,
    InlayHintKind, InlayHintLabel, Location, MarkupContent, MarkupKind, ParameterInformation,
    ParameterLabel, Range, SignatureHelp, SignatureInformation, SymbolInformation, SymbolKind,
    TextEdit, Uri, WorkspaceEdit,
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

    pub(crate) fn signature_help(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<SignatureHelp> {
        let help = Semantics::new(self.db, project).signature_help(file, offset)?;
        let (label, ranges) = help.signature.label();
        let unit = |i: usize| match self.columns {
            Columns::Utf8 => i as u32,
            Columns::Utf16 => label[..i].encode_utf16().count() as u32,
        };
        let parameters: Vec<ParameterInformation> = ranges
            .iter()
            .map(|&(start, end)| ParameterInformation {
                label: ParameterLabel::LabelOffsets([unit(start), unit(end)]),
                documentation: None,
            })
            .collect();
        // Past the last parameter, none is highlighted.
        let active = help.active.unwrap_or(parameters.len()) as u32;
        let documentation = help.signature.doc.clone().map(|value| {
            Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            })
        });
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation,
                parameters: Some(parameters),
                active_parameter: Some(active),
            }],
            active_signature: Some(0),
            active_parameter: Some(active),
        })
    }

    pub(crate) fn inlay_hints(
        &self,
        project: Project,
        file: SourceFile,
        range: TextRange,
        mode: ParameterHints,
    ) -> Vec<InlayHint> {
        let map = self.map(file);
        Semantics::new(self.db, project)
            .parameter_hints(file, range, mode)
            .into_iter()
            .map(|hint| InlayHint {
                position: map.position(hint.offset),
                label: InlayHintLabel::String(format!("{}:", hint.label)),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: Some(true),
                data: None,
            })
            .collect()
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
