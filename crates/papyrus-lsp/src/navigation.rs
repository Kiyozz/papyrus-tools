//! Cross-file features: definition, hover, completion, references, rename,
//! workspace symbols, signature help, inlay hints, highlights,
//! implementations, call hierarchy, reference lenses. The model answers in
//! files and byte ranges; this module turns them into LSP types.

use std::collections::HashMap;
use std::path::Path;

use papyrus_hir::{
    CallItem, CompletionKind, Database, ItemKind, NavTarget, ParameterHints, Project, Reference,
    Semantics, SourceFile,
};
use papyrus_syntax::{TextRange, TextSize};
use tower_lsp_server::ls_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, CodeLens, Command,
    CompletionItem, CompletionItemKind, DocumentChangeOperation, DocumentChanges,
    DocumentHighlight, DocumentHighlightKind, Documentation, Hover, HoverContents, InlayHint,
    InlayHintKind, InlayHintLabel, Location, MarkupContent, MarkupKind, OneOf,
    OptionalVersionedTextDocumentIdentifier, ParameterInformation, ParameterLabel, Position, Range,
    RenameFile, ResourceOp, SignatureHelp, SignatureInformation, SymbolInformation, SymbolKind,
    TextDocumentEdit, TextEdit, TypeHierarchyItem, Uri, WorkspaceEdit,
};

use crate::position::{Columns, LineMap};

/// Text edits by document.
pub(crate) type Changes = HashMap<Uri, Vec<TextEdit>>;

/// The LSP kind of an item.
fn symbol_kind(kind: ItemKind) -> SymbolKind {
    match kind {
        ItemKind::Variable | ItemKind::Guard => SymbolKind::VARIABLE,
        ItemKind::Property => SymbolKind::PROPERTY,
        ItemKind::Function => SymbolKind::FUNCTION,
        ItemKind::Event | ItemKind::CustomEvent => SymbolKind::EVENT,
        ItemKind::State => SymbolKind::NAMESPACE,
        ItemKind::Struct => SymbolKind::STRUCT,
        ItemKind::Field => SymbolKind::FIELD,
        ItemKind::Group => SymbolKind::PACKAGE,
    }
}

/// What the features need: the model, and how to name its files.
pub(crate) struct Navigator<'a> {
    pub(crate) db: &'a Database,
    pub(crate) columns: Columns,
    /// The client renames files in a workspace edit.
    pub(crate) can_rename: bool,
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
                // `value = `, first in the list.
                insert_text: (c.kind == CompletionKind::NamedArg).then(|| format!("{} ", c.label)),
                sort_text: (c.kind == CompletionKind::NamedArg).then(|| format!("0{}", c.label)),
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
                    CompletionKind::NamedArg => CompletionItemKind::FIELD,
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

    /// The declaration of the name at `offset` renamed to the case of this
    /// use, when the use follows the naming convention the declaration
    /// breaks: the action's title and its edits.
    pub(crate) fn rename_declaration(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<(String, WorkspaceEdit)> {
        let new_name = Semantics::new(self.db, project).case_rename(file, offset)?;
        let (edit, _) = self.rename(project, file, offset, &new_name).ok()?;
        Some((format!("Rename the declaration to `{new_name}`"), edit))
    }

    /// The name at `offset` renamed to `new_name`, the convention's, across
    /// the project: the action's title and its edits. Nothing if the file
    /// already holds that name: the rename would merge the two.
    pub(crate) fn rename_to_convention(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
        old_name: &str,
        new_name: &str,
    ) -> Option<(String, WorkspaceEdit)> {
        let root = Semantics::new(self.db, project).root(file);
        let taken = !old_name.eq_ignore_ascii_case(new_name)
            && root
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| token.text().eq_ignore_ascii_case(new_name));
        if taken {
            return None;
        }
        let (edit, warning) = self.rename(project, file, offset, new_name).ok()?;
        let note = if warning.is_some() {
            " (plugins bind it by name)"
        } else {
            ""
        };
        Some((format!("Rename to `{new_name}`{note}"), edit))
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
        let changes = self.changes(&edit.edits, new_name)?;
        let workspace_edit = match &edit.moved {
            Some((script, path)) => self.with_file_moved(*script, path, changes)?,
            None => WorkspaceEdit {
                changes: Some(changes),
                ..WorkspaceEdit::default()
            },
        };
        Ok((workspace_edit, edit.warning))
    }

    /// `new_name` at every reference, by file.
    fn changes(&self, references: &[Reference], new_name: &str) -> Result<Changes, String> {
        let mut changes = Changes::new();
        for reference in references {
            let uri = (self.uri)(reference.file)
                .ok_or_else(|| format!("{} has no URI", reference.file.path(self.db).display()))?;
            changes.entry(uri).or_default().push(TextEdit::new(
                self.range(reference.file, reference.range),
                new_name.to_owned(),
            ));
        }
        Ok(changes)
    }

    /// `changes`, then the file of `script` moved: the edits name it by its
    /// old URI.
    fn with_file_moved(
        &self,
        script: SourceFile,
        new_path: &Path,
        changes: Changes,
    ) -> Result<WorkspaceEdit, String> {
        if !self.can_rename {
            return Err(
                "this editor cannot rename files from the server: rename the file instead"
                    .to_owned(),
            );
        }
        let path = script.path(self.db);
        let old_uri = (self.uri)(script).ok_or_else(|| format!("{} has no URI", path.display()))?;
        let new_uri = Uri::from_file_path(new_path)
            .ok_or_else(|| format!("{} has no URI", new_path.display()))?;
        let mut operations: Vec<DocumentChangeOperation> = changes
            .into_iter()
            .map(|(uri, edits)| {
                DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
                    edits: edits.into_iter().map(OneOf::Left).collect(),
                })
            })
            .collect();
        operations.push(DocumentChangeOperation::Op(ResourceOp::Rename(
            RenameFile {
                old_uri,
                new_uri,
                options: None,
                annotation_id: None,
            },
        )));
        Ok(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Operations(operations)),
            ..WorkspaceEdit::default()
        })
    }

    /// The script of a file about to go to `path`, renamed with it: its
    /// edits and the warning to show.
    pub(crate) fn file_moved(
        &self,
        project: Project,
        file: SourceFile,
        path: &Path,
    ) -> Option<(Changes, Option<String>)> {
        let sema = Semantics::new(self.db, project);
        let items = sema.items(file);
        let name = sema.script_name_at(file, path)?;
        if items.name == name {
            return None;
        }
        let edit = sema.rename(file, items.name_range.start(), &name).ok()?;
        Some((self.changes(&edit.edits, &name).ok()?, edit.warning))
    }

    fn type_item(&self, project: Project, file: SourceFile) -> Option<TypeHierarchyItem> {
        let items = Semantics::new(self.db, project).items(file);
        let whole = TextRange::up_to(TextSize::of(file.text(self.db)));
        Some(TypeHierarchyItem {
            name: items.name.clone(),
            kind: SymbolKind::CLASS,
            tags: None,
            detail: items.parent.clone(),
            uri: (self.uri)(file)?,
            range: self.range(file, whole),
            selection_range: self.range(file, items.name_range),
            data: None,
        })
    }

    pub(crate) fn prepare_type_hierarchy(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<TypeHierarchyItem>> {
        let script = Semantics::new(self.db, project).script_at(file, offset)?;
        Some(vec![self.type_item(project, script)?])
    }

    pub(crate) fn supertypes(&self, project: Project, file: SourceFile) -> Vec<TypeHierarchyItem> {
        let found = Semantics::new(self.db, project).supertypes(file);
        found
            .into_iter()
            .filter_map(|file| self.type_item(project, file))
            .collect()
    }

    pub(crate) fn subtypes(&self, project: Project, file: SourceFile) -> Vec<TypeHierarchyItem> {
        let found = Semantics::new(self.db, project).subtypes(file);
        found
            .into_iter()
            .filter_map(|file| self.type_item(project, file))
            .collect()
    }

    #[allow(deprecated)] // `SymbolInformation::deprecated`, required by the type.
    pub(crate) fn symbols(&self, projects: &[Project], query: &str) -> Vec<SymbolInformation> {
        papyrus_hir::workspace_symbols(self.db, projects, query)
            .into_iter()
            .filter_map(|symbol| {
                Some(SymbolInformation {
                    kind: match symbol.kind {
                        papyrus_hir::SymbolKind::Script => SymbolKind::CLASS,
                        papyrus_hir::SymbolKind::Item(kind) => symbol_kind(kind),
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

    pub(crate) fn highlights(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<DocumentHighlight>> {
        let found = Semantics::new(self.db, project).highlights(file, offset)?;
        let map = self.map(file);
        Some(
            found
                .into_iter()
                .map(|h| DocumentHighlight {
                    range: map.range(h.range),
                    kind: Some(if h.write {
                        DocumentHighlightKind::WRITE
                    } else {
                        DocumentHighlightKind::READ
                    }),
                })
                .collect(),
        )
    }

    pub(crate) fn implementations(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<Location>> {
        let found = Semantics::new(self.db, project).implementations(file, offset)?;
        Some(
            found
                .into_iter()
                .filter_map(|t| self.location(t.file, t.focus_range))
                .collect(),
        )
    }

    fn call_item(&self, item: CallItem) -> Option<CallHierarchyItem> {
        let file = item.target.file;
        Some(CallHierarchyItem {
            name: item.name,
            kind: symbol_kind(item.kind),
            tags: None,
            detail: Some(item.detail),
            uri: (self.uri)(file)?,
            range: self.range(file, item.target.full_range),
            selection_range: self.range(file, item.target.focus_range),
            data: None,
        })
    }

    pub(crate) fn prepare_call_hierarchy(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<CallHierarchyItem>> {
        let item = Semantics::new(self.db, project).prepare_call_hierarchy(file, offset)?;
        Some(vec![self.call_item(item)?])
    }

    /// Who calls the item declared at `offset`.
    pub(crate) fn incoming_calls(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<CallHierarchyIncomingCall>> {
        let sema = Semantics::new(self.db, project);
        let def = sema.def_at(file, offset)?;
        Some(
            sema.incoming_calls(&def)
                .into_iter()
                .filter_map(|call| {
                    let map = self.map(call.item.target.file);
                    Some(CallHierarchyIncomingCall {
                        from_ranges: call.ranges.iter().map(|r| map.range(*r)).collect(),
                        from: self.call_item(call.item)?,
                    })
                })
                .collect(),
        )
    }

    /// Whom the item declared at `offset` calls.
    pub(crate) fn outgoing_calls(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
    ) -> Option<Vec<CallHierarchyOutgoingCall>> {
        let sema = Semantics::new(self.db, project);
        let def = sema.def_at(file, offset)?;
        let map = self.map(file);
        Some(
            sema.outgoing_calls(&def)
                .into_iter()
                .filter_map(|call| {
                    Some(CallHierarchyOutgoingCall {
                        from_ranges: call.ranges.iter().map(|r| map.range(*r)).collect(),
                        to: self.call_item(call.item)?,
                    })
                })
                .collect(),
        )
    }

    /// A lens above each function, property, script variable, struct and
    /// custom event of `file`, resolved later: counting references reads
    /// the whole project.
    pub(crate) fn code_lenses(
        &self,
        project: Project,
        file: SourceFile,
        uri: &Uri,
    ) -> Vec<CodeLens> {
        let sema = Semantics::new(self.db, project);
        let map = self.map(file);
        sema.items(file)
            .items
            .iter()
            .filter(|i| {
                matches!(
                    i.kind,
                    ItemKind::Function
                        | ItemKind::Property
                        | ItemKind::Variable
                        | ItemKind::Struct
                        | ItemKind::CustomEvent
                )
            })
            .map(|i| CodeLens {
                range: map.range(i.name_range),
                command: None,
                data: Some(serde_json::json!({ "uri": uri.as_str() })),
            })
            .collect()
    }

    /// The references of the item named at `offset`, as a lens command.
    pub(crate) fn lens_command(
        &self,
        project: Project,
        file: SourceFile,
        offset: TextSize,
        uri: &Uri,
        position: Position,
    ) -> Command {
        let found = Semantics::new(self.db, project)
            .references(file, offset, false)
            .unwrap_or_default();
        let locations = self.locations(found);
        let count = locations.len();
        Command {
            title: if count == 1 {
                "1 reference".to_owned()
            } else {
                format!("{count} references")
            },
            // The client shows them; `editor.action.showReferences` wants
            // its own types, which the extension converts to.
            command: "papyrus.showReferences".to_owned(),
            arguments: Some(vec![
                serde_json::json!(uri.as_str()),
                serde_json::json!(position),
                serde_json::json!(locations),
            ]),
        }
    }
}
