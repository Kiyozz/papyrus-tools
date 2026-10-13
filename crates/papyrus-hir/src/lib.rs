//! The semantic model of Papyrus: what each script declares, across files,
//! and what each name in a script refers to.
//!
//! The model is a [salsa](https://docs.rs/salsa) database. Its inputs are
//! the text of each file ([`SourceFile`]) and the composition of each
//! project ([`Project`]); everything else is derived and memoized: the tree
//! of a file, what it declares, the index of a project's scripts. Editing a
//! function body re-parses that file only; the index is rebuilt only when a
//! script's name changes.
//!
//! On top of it, [`Semantics`] resolves names — locals, members inherited
//! through `extends`, the native scripts of the game, the global functions of
//! `import`ed scripts — and types expressions from their declarations alone.
//! What it cannot know is unknown, never an error.
//!
//! # Example
//!
//! ```
//! use papyrus_dialect::GameId;
//! use papyrus_hir::{Database, Def, Project, Semantics, SourceFile};
//! use papyrus_syntax::TextSize;
//!
//! let mut db = Database::default();
//! let base = SourceFile::new(&db, "Base.psc".into(), "ScriptName Base extends Quest\nFunction Greet()\nEndFunction\n".into());
//! let text = "ScriptName Child extends Base\nEvent OnInit()\n\tGreet()\nEndEvent\n";
//! let child = SourceFile::new(&db, "Child.psc".into(), text.into());
//! let project = Project::new(&db, vec![base, child], vec![], Some(GameId::Skyrim));
//!
//! let sema = Semantics::new(&db, project);
//! let offset = TextSize::new(text.find("Greet").unwrap() as u32);
//! let def = sema.def_at(child, offset).unwrap();
//! assert!(matches!(def, Def::Item { file, .. } if file == base));
//! ```

#![forbid(unsafe_code)]

mod builtins;
mod calls;
mod check;
mod hierarchy;
mod hints;
mod ide;
mod items;
mod mentions;
mod semantics;
mod ty;
mod types;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

use papyrus_dialect::{Dialect, GameId};
use papyrus_syntax::Parse;

pub use calls::{Signature, SignatureParam};
pub use check::{Diagnostic, Fix, Problem, Replacement};
pub use hierarchy::{Call, CallItem, Highlight};
pub use hints::{InlayHint, ParameterHints, SignatureHelp};
pub use ide::{
    Completion, CompletionKind, Hover, NavTarget, Reference, RenameEdit, RenameError, Symbol,
    SymbolKind, workspace_symbols,
};
pub use items::{Item, ItemKind, Param, ScriptItems};
pub use semantics::{Def, ScriptRef, Semantics};
pub use ty::Ty;

/// A script's text.
#[salsa::input(debug)]
pub struct SourceFile {
    /// Where it lives; used to name it, never read.
    #[returns(ref)]
    pub path: PathBuf,
    /// The text, decoded.
    #[returns(deref)]
    pub text: String,
}

/// A project: its own scripts, the scripts of its `imports` folders, its
/// target.
#[salsa::input(debug)]
pub struct Project {
    /// The scripts it checks.
    #[returns(deref)]
    pub files: Vec<SourceFile>,
    /// The scripts of its `imports` folders: read, never changed.
    #[returns(deref)]
    pub imports: Vec<SourceFile>,
    /// Its target, configured or inferred; `None` when unknown.
    #[returns(copy)]
    pub game: Option<GameId>,
    /// `skse = true`: Skyrim's native scripts include SKSE's additions.
    /// Also on when the project or its imports declare the `SKSE` script.
    #[default]
    #[returns(copy)]
    pub skse: bool,
}

/// The database.
#[salsa::db]
#[derive(Clone, Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

#[salsa::db]
impl salsa::Database for Database {}

/// The dialect a file is read with: its project's target, or the widest
/// syntax without one.
fn dialect(game: Option<GameId>) -> Dialect {
    Dialect::new(game.unwrap_or(GameId::Starfield))
}

/// The tree of `file`, read for `game`.
#[salsa::tracked(returns(ref))]
pub fn parse(db: &dyn salsa::Database, file: SourceFile, game: Option<GameId>) -> Parse {
    papyrus_syntax::parse(file.text(db), dialect(game))
}

/// What `file` declares.
#[salsa::tracked(returns(ref))]
pub fn script_items(
    db: &dyn salsa::Database,
    file: SourceFile,
    game: Option<GameId>,
) -> ScriptItems {
    ScriptItems::lower(&parse(db, file, game).syntax())
}

/// The name `file` declares, lowercased; empty without a header.
///
/// Separate from [`script_items`] so that the index survives edits that
/// keep the name.
#[salsa::tracked(returns(ref))]
fn script_key(db: &dyn salsa::Database, file: SourceFile, game: Option<GameId>) -> String {
    script_items(db, file, game).name.to_ascii_lowercase()
}

/// The scripts of a project, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptIndex {
    by_name: HashMap<String, SourceFile>,
    imported: HashSet<SourceFile>,
}

impl ScriptIndex {
    /// The script `name`, case-insensitively.
    pub fn get(&self, name: &str) -> Option<SourceFile> {
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }

    /// True if `file` comes from an `imports` folder.
    pub fn is_imported(&self, file: SourceFile) -> bool {
        self.imported.contains(&file)
    }

    /// Every indexed script.
    pub fn files(&self) -> impl Iterator<Item = SourceFile> + '_ {
        self.by_name.values().copied()
    }
}

/// The index of a project's scripts. Its own scripts take precedence over
/// imported ones of the same name.
#[salsa::tracked(returns(ref))]
pub fn script_index(db: &dyn salsa::Database, project: Project) -> ScriptIndex {
    let game = project.game(db);
    let mut index = ScriptIndex::default();
    for &file in project.files(db) {
        let key = script_key(db, file, game);
        if !key.is_empty() {
            index.by_name.entry(key.clone()).or_insert(file);
        }
    }
    for &file in project.imports(db) {
        let key = script_key(db, file, game);
        if !key.is_empty() && !index.by_name.contains_key(key) {
            index.by_name.insert(key.clone(), file);
            index.imported.insert(file);
        }
    }
    index
}
