//! Definitions, hovers, completions, references and renames across files.

use papyrus_dialect::GameId;
use papyrus_hir::{
    CompletionKind, Database, Def, ItemKind, Project, Semantics, SourceFile, SymbolKind,
    workspace_symbols,
};
use papyrus_syntax::{TextRange, TextSize};
use salsa::Setter;

/// A project of `files`, `(name, text)`; `$0` marks the cursor in one of
/// them. Names starting with `lib/` are imported.
struct Fixture {
    db: Database,
    project: Project,
    files: Vec<(String, SourceFile)>,
    cursor: Option<(SourceFile, TextSize)>,
}

impl Fixture {
    fn new(game: GameId, files: &[(&str, &str)]) -> Fixture {
        let db = Database::default();
        let mut own = Vec::new();
        let mut imports = Vec::new();
        let mut all = Vec::new();
        let mut cursor = None;
        for (name, text) in files {
            let offset = text.find("$0");
            let text = text.replace("$0", "");
            let file = SourceFile::new(&db, (*name).into(), text);
            if let Some(offset) = offset {
                cursor = Some((file, TextSize::new(offset as u32)));
            }
            if name.starts_with("lib/") {
                imports.push(file);
            } else {
                own.push(file);
            }
            all.push(((*name).to_owned(), file));
        }
        let project = Project::new(&db, own, imports, Some(game));
        Fixture {
            db,
            project,
            files: all,
            cursor,
        }
    }

    fn sema(&self) -> Semantics<'_> {
        Semantics::new(&self.db, self.project)
    }

    fn file(&self, name: &str) -> SourceFile {
        self.files.iter().find(|(n, _)| n == name).unwrap().1
    }

    fn cursor(&self) -> (SourceFile, TextSize) {
        self.cursor.expect("no $0 in the fixture")
    }

    /// The text of `range` in `file`.
    fn text(&self, file: SourceFile, range: TextRange) -> &str {
        &file.text(&self.db)[range]
    }

    fn name_of(&self, file: SourceFile) -> &str {
        &self.files.iter().find(|(_, f)| *f == file).unwrap().0
    }

    /// Where the cursor's definition is: `file:text`.
    fn definition(&self) -> Option<String> {
        let (file, offset) = self.cursor();
        let target = self.sema().definition(file, offset)?;
        Some(format!(
            "{}:{}",
            self.name_of(target.file),
            self.text(target.file, target.focus_range)
        ))
    }

    fn labels(&self) -> Vec<String> {
        let (file, offset) = self.cursor();
        self.sema()
            .completions(file, offset)
            .into_iter()
            .map(|c| c.label)
            .collect()
    }
}

const BASE: &str = "ScriptName MyMod:Base extends Quest\n\
    { The base of every quest }\n\
    int Property Count Auto\n\
    { How many }\n\
    int secret = 0\n\
    Function Greet(string asWho = \"you\")\n\
    { Says hello }\n\
    EndFunction\n";

#[test]
fn a_call_reaches_the_inherited_function() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\nEvent OnInit()\n\tGre$0et()\nEndEvent\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Greet"));
}

#[test]
fn a_member_follows_the_declared_type() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "User.psc",
                "ScriptName User extends ObjectReference\n\
                 Event OnActivate(ObjectReference akActionRef)\n\
                 \tMyMod:Base q = GetLinkedRef() as MyMod:Base\n\
                 \tq.Co$0unt += 1\n\
                 EndEvent\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Count"));
}

#[test]
fn calls_chain_through_return_types() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "User.psc",
                "ScriptName User extends Quest\n\
                 MyMod:Base Function Find()\nEndFunction\n\
                 Function Run()\n\tFind().Gr$0eet()\nEndFunction\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Greet"));
}

#[test]
fn locals_shadow_members_and_parameters_resolve() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            "ScriptName A\nint count\nFunction Run(int aiTimes)\n\tint count = aiTimes\n\tcou$0nt += 1\nEndFunction\n",
        )],
    );
    let (file, offset) = f.cursor();
    let def = f.sema().def_at(file, offset).unwrap();
    assert!(matches!(def, Def::Local { param: false, .. }), "{def:?}");
    let target = f.sema().nav_target(&def).unwrap();
    assert_eq!(f.text(file, target.full_range), "int count = aiTimes");
}

#[test]
fn natives_resolve_without_a_location() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            "ScriptName A extends ObjectReference\nimport Utility\nEvent OnInit()\n\tWa$0it(1.0)\nEndEvent\n",
        )],
    );
    let (file, offset) = f.cursor();
    let def = f.sema().def_at(file, offset).unwrap();
    assert!(
        matches!(def, Def::NativeFunction { script, .. } if script.name == "Utility"),
        "{def:?}"
    );
    assert_eq!(f.definition(), None);
    let hover = f.sema().hover(file, offset).unwrap();
    assert_eq!(
        hover.signature,
        "Function Wait(float afSeconds) Global Native"
    );
    assert_eq!(hover.origin.as_deref(), Some("Utility"));
}

#[test]
fn static_calls_and_self_and_parent() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\n\
                 Function Greet(string asWho = \"you\")\n\tparent.Gre$0et(asWho)\n\tDebug.Trace(self.Count)\nEndFunction\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Greet"));

    let child = f.file("Child.psc");
    let text = child.text(&f.db);
    let at = |needle: &str| TextSize::new(text.find(needle).unwrap() as u32 + 1);
    let trace = f.sema().def_at(child, at("Trace")).unwrap();
    assert!(matches!(trace, Def::NativeFunction { script, .. } if script.name == "Debug"));
    let count = f.sema().def_at(child, at("Count")).unwrap();
    assert!(matches!(count, Def::Item { file, .. } if file == f.file("Base.psc")));
}

#[test]
fn struct_fields_resolve_in_fallout_4() {
    let f = Fixture::new(
        GameId::Fallout4,
        &[(
            "A.psc",
            "ScriptName A\nStruct Entry\n\tint Hits\nEndStruct\n\
             Function Run()\n\tEntry e = new Entry\n\te.Hi$0ts = 1\nEndFunction\n",
        )],
    );
    assert_eq!(f.definition().as_deref(), Some("A.psc:Hits"));
}

#[test]
fn hover_shows_the_signature_and_docstring() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\nEvent OnInit()\n\tGreet()\n\tCou$0nt = 1\nEndEvent\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let hover = f.sema().hover(file, offset).unwrap();
    assert_eq!(hover.signature, "int Property Count Auto");
    assert_eq!(hover.doc.as_deref(), Some("How many"));
    assert_eq!(hover.origin.as_deref(), Some("MyMod:Base"));
    assert_eq!(f.text(file, hover.range), "Count");
}

#[test]
fn member_completion_lists_the_chain() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "User.psc",
                "ScriptName User extends Quest\nFunction Run(MyMod:Base akQuest)\n\takQuest.$0\nEndFunction\n",
            ),
        ],
    );
    let labels = f.labels();
    for expected in ["Count", "Greet", "Start", "GetStage", "RegisterForUpdate"] {
        assert!(
            labels.iter().any(|l| l == expected),
            "{expected} in {labels:?}"
        );
    }
    // Another script's variables are private.
    assert!(!labels.iter().any(|l| l == "secret"), "{labels:?}");
}

#[test]
fn static_completion_lists_global_functions() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            "ScriptName A\nFunction Run()\n\tDebug.Tr$0\nEndFunction\n",
        )],
    );
    let labels = f.labels();
    assert!(labels.iter().any(|l| l == "Trace"), "{labels:?}");
    assert!(labels.iter().any(|l| l == "MessageBox"), "{labels:?}");
}

#[test]
fn general_completion_lists_locals_members_and_types() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\nint mine\nFunction Run(int aiTimes)\n\tint total = 0\n\t$0\nEndFunction\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let completions = f.sema().completions(file, offset);
    let kind = |label: &str| {
        completions
            .iter()
            .find(|c| c.label == label)
            .map(|c| c.kind)
    };
    assert_eq!(kind("total"), Some(CompletionKind::Local));
    assert_eq!(kind("aiTimes"), Some(CompletionKind::Local));
    assert_eq!(kind("mine"), Some(CompletionKind::Variable));
    assert_eq!(kind("Count"), Some(CompletionKind::Property));
    assert_eq!(kind("Greet"), Some(CompletionKind::Function));
    assert_eq!(kind("secret"), None);
    assert_eq!(kind("MyMod:Base"), Some(CompletionKind::Script));
    assert_eq!(kind("Actor"), Some(CompletionKind::Script));
}

#[test]
fn type_completion_after_extends() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[("Base.psc", BASE), ("A.psc", "ScriptName A extends $0\n")],
    );
    let labels = f.labels();
    assert!(labels.iter().any(|l| l == "MyMod:Base"));
    assert!(labels.iter().any(|l| l == "ObjectReference"));
    assert!(!labels.iter().any(|l| l == "self"));
}

#[test]
fn references_join_overrides_and_calls() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\n\
                 Function Greet(string asWho = \"you\")\n\tparent.Greet(asWho)\nEndFunction\n\
                 State Busy\n\tFunction Greet(string asWho = \"you\")\n\tEndFunction\nEndState\n",
            ),
            (
                "User.psc",
                "ScriptName User extends Quest\nFunction Run(MyMod:Base akQuest)\n\takQuest.Gre$0et()\nEndFunction\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let mut found: Vec<String> = f
        .sema()
        .references(file, offset, true)
        .unwrap()
        .into_iter()
        .map(|r| {
            assert_eq!(f.text(r.file, r.range), "Greet");
            format!(
                "{}{}",
                f.name_of(r.file),
                if r.declaration { " (decl)" } else { "" }
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            "Base.psc (decl)",
            "Child.psc",
            "Child.psc (decl)",
            "Child.psc (decl)",
            "User.psc"
        ]
    );
}

#[test]
fn references_of_a_script_include_types() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "User.psc",
                "ScriptName User extends MyMod:Base\nMyMod:Base[] all\nFunction Run()\n\tMyMod:Ba$0se b = self as MyMod:Base\nEndFunction\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let found = f.sema().references(file, offset, true).unwrap();
    assert_eq!(found.len(), 5, "{found:?}");
    assert!(
        found
            .iter()
            .all(|r| f.text(r.file, r.range) == "MyMod:Base")
    );
}

#[test]
fn rename_a_local() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            "ScriptName A\nFunction Run(int aiTimes)\n\tint n = aiTimes\n\tn += 1\n\tDebug.Trace(n$0)\nEndFunction\nFunction Other()\n\tint n\nEndFunction\n",
        )],
    );
    let (file, offset) = f.cursor();
    let edit = f.sema().rename(file, offset, "count").unwrap();
    assert_eq!(edit.edits.len(), 3);
    assert_eq!(edit.warning, None);
}

#[test]
fn rename_a_property_warns() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\nEvent OnInit()\n\tCou$0nt = 1\nEndEvent\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let edit = f.sema().rename(file, offset, "Total").unwrap();
    assert_eq!(edit.edits.len(), 2);
    assert!(edit.warning.unwrap().contains("Creation Kit"));
}

#[test]
fn some_renames_are_refused() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "lib/Lib.psc",
                "ScriptName Lib extends Quest\nFunction Shared()\nEndFunction\n",
            ),
            (
                "A.psc",
                "ScriptName A extends Lib\nEvent OnInit()\n\tShared()\n\tRegisterForUpdate(1.0)\nEndEvent\n",
            ),
        ],
    );
    let file = f.file("A.psc");
    let text = file.text(&f.db).to_owned();
    let at = |needle: &str| TextSize::new(text.find(needle).unwrap() as u32 + 1);
    let sema = f.sema();
    let error = |needle: &str, name: &str| sema.rename(file, at(needle), name).unwrap_err().0;
    assert!(error("Shared", "Mine").contains("imported"));
    assert!(error("RegisterForUpdate", "Mine").contains("native"));
    assert!(error("OnInit", "Start").contains("event"));
    assert!(error("A extends", "B").contains("file"));
    assert!(
        sema.rename(file, at("Shared"), "1abc")
            .unwrap_err()
            .0
            .contains("valid")
    );
}

#[test]
fn imported_scripts_are_followed() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "lib/Lib.psc",
                "ScriptName Lib extends Quest\nFunction Shared()\nEndFunction\n",
            ),
            (
                "A.psc",
                "ScriptName A extends Lib\nEvent OnInit()\n\tSha$0red()\nEndEvent\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("lib/Lib.psc:Shared"));
}

#[test]
fn workspace_symbols_skip_imports() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "lib/Lib.psc",
                "ScriptName Lib\nFunction Greeting()\nEndFunction\n",
            ),
            ("Base.psc", BASE),
        ],
    );
    let found = workspace_symbols(&f.db, &[f.project], "gre");
    let names: Vec<(&str, SymbolKind)> = found.iter().map(|s| (s.name.as_str(), s.kind)).collect();
    assert_eq!(names, [("Greet", SymbolKind::Item(ItemKind::Function))]);
}

#[test]
fn edits_are_seen() {
    let mut f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            (
                "Child.psc",
                "ScriptName Child extends MyMod:Base\nEvent OnInit()\n\tGre$0et()\nEndEvent\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Greet"));
    let base = f.file("Base.psc");
    let renamed = BASE.replace("Function Greet", "Function Welcome");
    base.set_text(&mut f.db).to(renamed);
    assert_eq!(f.definition(), None);
    let restored = BASE.replace("ScriptName MyMod:Base", "ScriptName MyMod:Base\n");
    base.set_text(&mut f.db).to(restored);
    assert_eq!(f.definition().as_deref(), Some("Base.psc:Greet"));
}

#[test]
fn short_names_resolve_in_the_namespace_and_imports() {
    let f = Fixture::new(
        GameId::Fallout4,
        &[
            (
                "Data.psc",
                "ScriptName MyMod:Data\nStruct Rating\n\tActorValue Value\nEndStruct\n",
            ),
            (
                "Manager.psc",
                "ScriptName MyMod:Manager extends Quest\nimport MyMod:Data\nRating[] Property Ratings Auto\n",
            ),
            (
                "User.psc",
                "ScriptName MyMod:User extends Quest\nManager Property Owner Auto\n\
                 Function Run()\n\tOwner.Ratings[0].Val$0ue = None\nEndFunction\n",
            ),
        ],
    );
    assert_eq!(f.definition().as_deref(), Some("Data.psc:Value"));
}
