//! Definitions, hovers, completions, references and renames across files.

use papyrus_dialect::GameId;
use papyrus_hir::{
    CompletionKind, Database, Def, ItemKind, ParameterHints, Project, Semantics, SourceFile,
    SymbolKind, workspace_symbols,
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

/// Papyrus ignores case: a member written `test` still reaches `Test`.
#[test]
fn navigation_and_completion_ignore_case() {
    const LIB: &str = "ScriptName TestScript Hidden\nFunction Test() global\nEndFunction\n";
    let main =
        |body: &str| format!("ScriptName A extends Quest\nFunction Run()\n\t{body}\nEndFunction\n");
    let call = main("testScript.te$0st()");
    let f = Fixture::new(GameId::Skyrim, &[("A.psc", &call), ("TestScript.psc", LIB)]);
    assert_eq!(f.definition().as_deref(), Some("TestScript.psc:Test"));
    let receiver = main("testsc$0ript.test()");
    let f = Fixture::new(
        GameId::Skyrim,
        &[("A.psc", &receiver), ("TestScript.psc", LIB)],
    );
    assert_eq!(f.definition().as_deref(), Some("TestScript.psc:TestScript"));
    let members = main("testscript.$0");
    let f = Fixture::new(
        GameId::Skyrim,
        &[("A.psc", &members), ("TestScript.psc", LIB)],
    );
    assert!(f.labels().iter().any(|l| l == "Test"), "{:?}", f.labels());
}

#[test]
fn parameters_complete_as_named_arguments() {
    let lib = "ScriptName Test Hidden\n\
               int Function WithParam(int value) global native\n\
               Function Take(int aiCount, float afScale = 1.0, Actor akWho = None) global\nEndFunction\n";
    let named = |body: &str| {
        let main = format!(
            "ScriptName A extends Quest\nFunction Run()\n\tint value = 1\n\t{body}\nEndFunction\n"
        );
        let f = Fixture::new(GameId::Skyrim, &[("A.psc", &main), ("Test.psc", lib)]);
        let (file, offset) = f.cursor();
        let completions = f.sema().completions(file, offset);
        // The local of the same name stays proposed.
        assert!(
            completions
                .iter()
                .any(|c| c.label == "value" && c.kind == CompletionKind::Local),
            "{completions:?}"
        );
        completions
            .into_iter()
            .filter(|c| c.kind == CompletionKind::NamedArg)
            .map(|c| (c.label, c.detail.unwrap_or_default()))
            .collect::<Vec<_>>()
    };
    let pair = |label: &str, detail: &str| (label.to_owned(), detail.to_owned());
    assert_eq!(named("Test.WithParam(val$0)"), [pair("value =", "int")]);
    assert_eq!(
        named("Test.Take(1, akWho = None, $0)"),
        [pair("afScale =", "float = 1.0")]
    );
    // Within a value, no name.
    assert_eq!(named("Test.Take(1 + val$0)"), []);
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
fn an_edit_cancels_a_search_on_a_copy() {
    let mut fixture = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "A.psc",
                "ScriptName A extends Quest\nFunction $0Greet()\nEndFunction\n",
            ),
            (
                "B.psc",
                "ScriptName B extends A\nEvent OnInit()\n\tGreet()\nEndEvent\n",
            ),
        ],
    );
    let (file, offset) = fixture.cursor();
    let def = fixture.sema().def_at(file, offset).unwrap();
    let copy = fixture.db.clone();
    let project = fixture.project;
    // Searches until cancelled: the edit below must stop it to land.
    let search = std::thread::spawn(move || {
        salsa::Cancelled::catch(|| {
            loop {
                Semantics::new(&copy, project).references_of(&def);
            }
        })
    });
    let b = fixture.file("B.psc");
    b.set_text(&mut fixture.db)
        .to("ScriptName B extends A\n".to_owned());
    assert!(search.join().unwrap().is_err());
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
    assert!(error("Lib\n", "Mine").contains("imported"));
    assert!(error("A extends", "Lib").contains("already exists"));
    assert!(
        sema.rename(file, at("Shared"), "1abc")
            .unwrap_err()
            .0
            .contains("valid")
    );
}

#[test]
fn a_script_is_renamed_everywhere_with_its_file() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "Base.psc",
                "ScriptName Base extends Quest\nFunction Greet() Global\nEndFunction\n",
            ),
            (
                "Child.psc",
                "ScriptName Child extends Base\nBase Property other Auto\nEvent OnInit()\n\tBase.Greet()\n\tQuest q = other as Base\nEndEvent\n",
            ),
        ],
    );
    let (base, child) = (f.file("Base.psc"), f.file("Child.psc"));
    let edit = f.sema().rename(base, TextSize::new(12), "Root").unwrap();
    assert_eq!(edit.script, Some(base));
    assert!(edit.warning.unwrap().contains("Creation Kit"));
    let in_child = edit.edits.iter().filter(|e| e.file == child).count();
    assert_eq!(in_child, 4);
    assert_eq!(edit.edits.len(), 5);
}

#[test]
fn the_type_hierarchy_follows_extends() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", "ScriptName Base extends Quest\n"),
            ("Child.psc", "ScriptName Child extends Base\n"),
            ("Grand.psc", "ScriptName Grand extends Child\n"),
            ("Other.psc", "ScriptName Other extends Base\n"),
        ],
    );
    let (base, child) = (f.file("Base.psc"), f.file("Child.psc"));
    let sema = f.sema();
    assert_eq!(sema.supertypes(child), [base]);
    // Quest has no source.
    assert!(sema.supertypes(base).is_empty());
    let mut subtypes: Vec<_> = sema
        .subtypes(base)
        .iter()
        .map(|&file| sema.items(file).name.clone())
        .collect();
    subtypes.sort();
    assert_eq!(subtypes, ["Child", "Other"]);
    // On the parent's name, the parent; anywhere else, the file's script.
    assert_eq!(sema.script_at(child, TextSize::new(26)), Some(base));
    assert_eq!(sema.script_at(child, TextSize::new(0)), Some(child));
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

// ── Signature help and inlay hints ────────────────────────────────────────

const TAKE: &str = "ScriptName A extends ObjectReference\n\
    Function Take(int aiCount, float afScale = 1.0)\nEndFunction\n\
    int Function Count()\nEndFunction\n";

/// The label and active parameter at the cursor of `body`, in an event of
/// [`TAKE`].
fn help(body: &str) -> Option<(String, Option<usize>)> {
    let text = format!("{TAKE}Event OnInit()\n{body}\nEndEvent\n");
    let fixture = Fixture::new(GameId::Skyrim, &[("A.psc", &text)]);
    let (file, offset) = fixture.cursor();
    let help = fixture.sema().signature_help(file, offset)?;
    Some((help.signature.label().0, help.active))
}

#[test]
fn signature_help_follows_the_cursor() {
    let take = "Take(int aiCount, float afScale = 1.0)".to_owned();
    assert_eq!(help("\tTake($0"), Some((take.clone(), Some(0))));
    assert_eq!(help("\tTake(1, $0"), Some((take.clone(), Some(1))));
    assert_eq!(help("\tTake(1, 2.0, $0)"), Some((take.clone(), None)));
    assert_eq!(help("\tTake(afScale = $0)"), Some((take.clone(), Some(1))));
    // The innermost call wins; after `)`, the outer one.
    assert_eq!(
        help("\tTake(Count($0))"),
        Some(("int Count()".to_owned(), None))
    );
    assert_eq!(help("\tTake(Count()$0)"), Some((take, Some(0))));
    assert_eq!(help("\tTake(1)$0"), None);

    let (label, active) = help("\tDebug.Trace(\"x\", $0").unwrap();
    assert_eq!(label, "Trace(string asTextToPrint, int aiSeverity = 0)");
    assert_eq!(active, Some(1));
}

#[test]
fn signature_labels_give_each_parameter_its_range() {
    let fixture = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            &format!("{TAKE}Event OnInit()\n\tTake($0\nEndEvent\n"),
        )],
    );
    let (file, offset) = fixture.cursor();
    let help = fixture.sema().signature_help(file, offset).unwrap();
    let (label, ranges) = help.signature.label();
    let params: Vec<&str> = ranges.iter().map(|&(s, e)| &label[s..e]).collect();
    assert_eq!(params, ["int aiCount", "float afScale = 1.0"]);
}

#[test]
fn parameter_hints_name_literals_by_default() {
    let text = format!(
        "{TAKE}Event OnInit()\n\tint count = 2\n\tTake(count, 2.0)\n\tTake(-1)\n\tDisable(true)\n\tTake(Count(), afScale = 1.0)\nEndEvent\n"
    );
    let fixture = Fixture::new(GameId::Skyrim, &[("A.psc", &text)]);
    let file = fixture.file("A.psc");
    let hints = |mode| -> Vec<(String, String)> {
        let range = TextRange::up_to(TextSize::of(text.as_str()));
        fixture
            .sema()
            .parameter_hints(file, range, mode)
            .into_iter()
            .map(|h| {
                let rest = &text[usize::from(h.offset)..];
                let value = rest.split([',', ')']).next().unwrap().to_owned();
                (h.label, value)
            })
            .collect()
    };
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert_eq!(
        hints(ParameterHints::Literals),
        [
            pair("afScale", "2.0"),
            pair("aiCount", "-1"),
            pair("abFadeOut", "true"),
        ]
    );
    // `count` already says `aiCount`.
    assert_eq!(
        hints(ParameterHints::All),
        [
            pair("afScale", "2.0"),
            pair("aiCount", "-1"),
            pair("abFadeOut", "true"),
            pair("aiCount", "Count("),
        ]
    );
    assert_eq!(hints(ParameterHints::None), []);
}

// ── Hierarchy ─────────────────────────────────────────────────────────────

const CHILD: &str = "ScriptName Child extends MyMod:Base
Function Greet(string asWho = \"you\")
\tparent.Greet(asWho)
\tCount += 1
EndFunction
State Busy
\tFunction Greet(string asWho = \"you\")
\tEndFunction
EndState
";

#[test]
fn highlights_tell_writes_from_reads() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[(
            "A.psc",
            "ScriptName A extends Quest\nint count\nFunction Run()\n\tco$0unt = 1\n\tcount += count\n\tDebug.Trace(self.count)\nEndFunction\n",
        )],
    );
    let (file, offset) = f.cursor();
    let found: Vec<(u32, bool)> = f
        .sema()
        .highlights(file, offset)
        .unwrap()
        .into_iter()
        .map(|h| {
            assert_eq!(f.text(file, h.range), "count");
            (u32::from(h.range.start()), h.write)
        })
        .collect();
    let writes: Vec<bool> = found.iter().map(|(_, w)| *w).collect();
    assert_eq!(writes, [true, true, true, false, false]);
}

#[test]
fn implementations_are_the_overrides() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "Base.psc",
                &BASE.replace("Function Greet", "Function Gr$0eet"),
            ),
            ("Child.psc", CHILD),
            (
                "Other.psc",
                "ScriptName Other extends Quest\nFunction Greet()\nEndFunction\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let found: Vec<(String, u32)> = f
        .sema()
        .implementations(file, offset)
        .unwrap()
        .into_iter()
        .map(|t| {
            assert_eq!(f.text(t.file, t.focus_range), "Greet");
            (
                f.name_of(t.file).to_owned(),
                u32::from(t.focus_range.start()),
            )
        })
        .collect();
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Child.psc", "Child.psc"]);
}

#[test]
fn the_overrides_of_a_native_event() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            (
                "A.psc",
                "ScriptName A extends ObjectReference\nEvent OnInit()\n\tOnAct$0ivate(None)\nEndEvent\n",
            ),
            (
                "B.psc",
                "ScriptName B extends ObjectReference\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n",
            ),
            (
                "C.psc",
                "ScriptName C extends Actor\nEvent OnActivate(ObjectReference akActionRef)\nEndEvent\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let mut names: Vec<String> = f
        .sema()
        .implementations(file, offset)
        .unwrap()
        .into_iter()
        .map(|t| f.name_of(t.file).to_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["B.psc", "C.psc"]);
}

#[test]
fn calls_come_in_and_go_out() {
    let f = Fixture::new(
        GameId::Skyrim,
        &[
            ("Base.psc", BASE),
            ("Child.psc", CHILD),
            (
                "User.psc",
                "ScriptName User extends Quest\nFunction Run(MyMod:Base akQuest)\n\takQuest.Gre$0et()\n\takQuest.Greet(\"me\")\n\tHelp()\nEndFunction\nFunction Help()\n\tDebug.Trace(\"x\")\nEndFunction\nint Property Value\n\tint Function Get()\n\t\tHelp()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n",
            ),
        ],
    );
    let (file, offset) = f.cursor();
    let sema = f.sema();
    let greet = sema.prepare_call_hierarchy(file, offset).unwrap();
    assert_eq!(
        (greet.name.as_str(), greet.kind),
        ("Greet", ItemKind::Function)
    );

    let mut incoming: Vec<(String, usize)> = sema
        .incoming_calls(&greet.def)
        .into_iter()
        .map(|c| {
            (
                format!("{}:{}", f.name_of(c.item.target.file), c.item.name),
                c.ranges.len(),
            )
        })
        .collect();
    incoming.sort();
    assert_eq!(
        incoming,
        [
            ("Child.psc:Greet".to_owned(), 1),
            ("User.psc:Run".to_owned(), 2)
        ]
    );

    let user = f.file("User.psc");
    let run = sema
        .incoming_calls(&greet.def)
        .into_iter()
        .find(|c| c.item.name == "Run")
        .unwrap()
        .item;
    let outgoing: Vec<(String, usize)> = sema
        .outgoing_calls(&run.def)
        .into_iter()
        .map(|c| (c.item.name, c.ranges.len()))
        .collect();
    // `Debug.Trace` is native: no source to go to.
    assert_eq!(outgoing, [("Greet".to_owned(), 2), ("Help".to_owned(), 1)]);

    let help = sema
        .outgoing_calls(&run.def)
        .into_iter()
        .find(|c| c.item.name == "Help")
        .unwrap()
        .item;
    let callers: Vec<(String, ItemKind)> = sema
        .incoming_calls(&help.def)
        .into_iter()
        .map(|c| (c.item.name, c.item.kind))
        .collect();
    assert_eq!(
        callers,
        [
            ("Run".to_owned(), ItemKind::Function),
            ("Value".to_owned(), ItemKind::Property)
        ]
    );
    assert_eq!(help.target.file, user);
}
