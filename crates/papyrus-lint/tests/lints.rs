//! The rules, on chosen cases.

use papyrus_dialect::{Dialect, GameId};
use papyrus_lint::{Context, Levels, ScriptSummary, Workspace, apply_fixes, run, vanilla_flags};
use papyrus_project::{Level, decode};

/// `(code, targeted text)` of each diagnostic.
fn lint(source: &str, game: Option<GameId>) -> Vec<(&'static str, String)> {
    diagnostics(source, game)
        .into_iter()
        .map(|d| (d.lint.code, source[d.range].to_owned()))
        .collect()
}

fn diagnostics(source: &str, game: Option<GameId>) -> Vec<papyrus_lint::Diagnostic> {
    diagnostics_in(source, game, &Workspace::default())
}

fn diagnostics_in(
    source: &str,
    game: Option<GameId>,
    workspace: &Workspace,
) -> Vec<papyrus_lint::Diagnostic> {
    let decoded = decode(source.as_bytes());
    let dialect = game.map(Dialect::new).unwrap_or_default();
    let parse = papyrus_syntax::parse(&decoded.text, dialect);
    assert!(parse.ok(), "{:?}", parse.errors());
    let flags = vanilla_flags(game);
    let context = Context {
        decoded: &decoded,
        root: &parse.syntax(),
        game,
        flags: &flags,
        workspace,
    };
    run(&context, &Levels::default())
}

fn fixed(source: &str) -> String {
    apply_fixes(source, &diagnostics(source, None)).expect("safe fix")
}

// ── Naming ────────────────────────────────────────────────────────────────

#[test]
fn le_style_go_ne_declenche_rien() {
    let source = "\
ScriptName T
Actor Property myActor Auto
int hitTotal
Guard countGuard
Struct Entry
\tint value
EndStruct
CustomEvent OnDone
Function DoThing(Actor akTarget, int aiCount = 1)
\tint hitCount = 0
\tmyActor.AddItem(akTarget)
EndFunction
Event OnInit()
EndEvent
Auto State Waiting
EndState
";
    assert_eq!(lint(source, Some(GameId::Starfield)), []);
}

#[test]
fn chaque_role_a_sa_convention() {
    let source = "\
ScriptName T
Actor Property MyActor Auto
int Hit_Total
Struct entry
\tint Value
EndStruct
Function do_thing(Actor Target)
\tint HitCount = 0
EndFunction
Event onInit()
EndEvent
";
    assert_eq!(
        lint(source, Some(GameId::Fallout4)),
        [
            ("PAP102", "MyActor".into()),
            ("PAP102", "Hit_Total".into()),
            ("PAP103", "entry".into()),
            ("PAP102", "Value".into()),
            ("PAP103", "do_thing".into()),
            ("PAP101", "Target".into()),
            ("PAP101", "HitCount".into()),
            ("PAP103", "onInit".into()),
        ]
    );
}

#[test]
fn les_accesseurs_de_property_echappent_au_nommage() {
    let source = "ScriptName T\nint Property count\n\tint Function get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n";
    assert_eq!(lint(source, None), []);
}

#[test]
fn le_message_explique_l_absence_de_correctif() {
    let d = diagnostics("ScriptName T\nActor Property MyActor Auto\n", None);
    assert!(d[0].message.contains("`myActor`"));
    assert!(d[0].message.contains(".esp"));
    assert!(d[0].fix.is_none());
}

// ── Fix ───────────────────────────────────────────────────────────────────

#[test]
fn le_correctif_renomme_la_locale_et_ses_usages() {
    let source = "\
ScriptName T
Function F()
\tint HitCount = 0
\tHitCount += 1
\tDebug.Trace(hitcount)
EndFunction
";
    assert_eq!(
        fixed(source),
        "\
ScriptName T
Function F()
\tint hitCount = 0
\thitCount += 1
\tDebug.Trace(hitCount)
EndFunction
"
    );
}

#[test]
fn le_correctif_ne_touche_ni_membres_ni_arguments_nommes_ni_appels() {
    let source = "\
ScriptName T
Function F(int Count)
\tCount = other.Count
\tFoo(Count = Count)
\tCount()
EndFunction
";
    assert_eq!(
        fixed(source),
        "\
ScriptName T
Function F(int count)
\tcount = other.Count
\tFoo(Count = count)
\tCount()
EndFunction
"
    );
}

#[test]
fn le_correctif_reste_dans_sa_fonction() {
    let source = "\
ScriptName T
Function A()
\tint Total
EndFunction
Function B()
\tTotal = 1
EndFunction
";
    assert!(fixed(source).contains("Function B()\n\tTotal = 1"));
}

#[test]
fn un_renommage_au_dela_de_la_casse_n_a_pas_de_correctif() {
    let d = diagnostics(
        "ScriptName T\nFunction F()\n\tint hit_count\nEndFunction\n",
        None,
    );
    assert_eq!(d.len(), 1);
    assert!(d[0].fix.is_none());
    assert!(d[0].message.contains("by hand"));
}

// ── Suppression and levels ────────────────────────────────────────────────

#[test]
fn allow_dans_le_code() {
    let source = "\
ScriptName T
Function F()
\tint HP ; papyrus: allow(naming/locals)
\t; papyrus: allow(PAP101)
\tint XP
\tint MP
EndFunction
";
    assert_eq!(lint(source, None), [("PAP101", "MP".into())]);
}

#[test]
fn les_niveaux_viennent_de_la_configuration() {
    let config = [
        ("naming/locals".to_owned(), Level::Allow),
        ("PAP103".to_owned(), Level::Error),
    ]
    .into_iter()
    .collect();
    let levels = Levels::from_config(&config).unwrap();
    assert_eq!(levels.get(&papyrus_lint::NAMING_LOCALS), Level::Allow);
    assert_eq!(levels.get(&papyrus_lint::NAMING_FUNCTIONS), Level::Error);
    assert_eq!(levels.get(&papyrus_lint::NAMING_VARIABLES), Level::Warn);

    let unknown = [("naming/lokals".to_owned(), Level::Allow)]
        .into_iter()
        .collect();
    assert!(Levels::from_config(&unknown).is_err());
}

// ── Flags ─────────────────────────────────────────────────────────────────

#[test]
fn un_flag_hors_de_sa_place_est_signale() {
    let source = "\
ScriptName t Hidden Conditional
int property x Mandatory
EndProperty
Function F() Conditional
EndFunction
Group g Mandatory
EndGroup
";
    let found: Vec<_> = lint(source, Some(GameId::Fallout4))
        .into_iter()
        .filter(|(code, _)| *code == "PAP002")
        .collect();
    assert_eq!(
        found,
        [
            ("PAP002", "Conditional".into()),
            ("PAP002", "Mandatory".into())
        ]
    );
}

#[test]
fn une_property_auto_accepte_les_flags_de_variable() {
    // Fallout 4's vanilla writes it: the hidden variable carries the flag.
    let source = "ScriptName t\nActorValue Property pod Auto Conditional Const\n";
    assert_eq!(lint(source, Some(GameId::Fallout4)), []);
}

#[test]
fn skyrim_n_a_pas_hidden_sur_les_champs() {
    // A real difference between the two `.flg` files; Skyrim has no structs
    // anyway, so it is checked on the table.
    let skyrim = vanilla_flags(Some(GameId::Skyrim));
    let hidden = skyrim.iter().find(|d| d.name == "Hidden").unwrap();
    assert_eq!(hidden.targets.len(), 2);
}

// ── Encoding ──────────────────────────────────────────────────────────────

#[test]
fn l_encodage_n_est_verifie_qu_avec_une_cible() {
    let source = "ScriptName t\nstring property s = \"café\" Auto\n";
    assert_eq!(lint(source, None), []);
    assert_eq!(lint(source, Some(GameId::Skyrim)), [("PAP001", "é".into())]);
}

#[test]
fn un_nom_inconnu_dans_allow_est_signale_avec_une_suggestion() {
    let source =
        "ScriptName T\nFunction F()\n\tint HP ; papyrus: allow(naming/lokals)\nEndFunction\n";
    let found = diagnostics(source, None);
    // The failed suppression lets the targeted lint through…
    assert!(found.iter().any(|d| d.lint.code == "PAP101"));
    // … and the typo is reported, where it is.
    let unknown = found
        .iter()
        .find(|d| d.lint.code == "PAP003")
        .expect("PAP003");
    assert_eq!(&source[unknown.range], "naming/lokals");
    assert!(
        unknown.message.contains("`naming/locals`"),
        "{}",
        unknown.message
    );
}

#[test]
fn la_configuration_suggere_aussi() {
    let config = [("PAP10".to_owned(), Level::Allow)].into_iter().collect();
    let error = Levels::from_config(&config).unwrap_err();
    assert!(error.contains("did you mean"), "{error}");
}

// ── Events ────────────────────────────────────────────────────────────────

fn events(source: &str, game: Option<GameId>, workspace: &Workspace) -> Vec<String> {
    diagnostics_in(source, game, workspace)
        .into_iter()
        .filter(|d| d.lint.code == "PAP004")
        .map(|d| source[d.range].to_owned())
        .collect()
}

#[test]
fn un_evenement_mal_orthographie_est_signale() {
    let source = "ScriptName T extends ObjectReference\nEvent OnActivatee(ObjectReference akActionRef)\nEndEvent\n";
    let found = diagnostics(source, Some(GameId::Skyrim));
    let event = found
        .iter()
        .find(|d| d.lint.code == "PAP004")
        .expect("PAP004");
    assert_eq!(&source[event.range], "OnActivatee");
    assert!(event.message.contains("`OnActivate`"), "{}", event.message);
    // Renaming changes the meaning of the code: suggested, never applied.
    assert!(event.fix.is_none());
    let suggestion = event.suggestion.as_ref().expect("suggestion");
    assert_eq!(suggestion.edits[0].text, "OnActivate");
}

#[test]
fn un_script_recoit_les_evenements_des_types_derives() {
    // Attached to an actor, an `extends ObjectReference` script receives `OnDeath`.
    let source = "ScriptName T extends ObjectReference\nEvent OnDeath(Actor akKiller)\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        Vec::<String>::new()
    );
    // A magic effect receives its actor's.
    let source = "ScriptName T extends ActiveMagicEffect\nEvent OnEnterBleedout()\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        Vec::<String>::new()
    );
}

#[test]
fn un_evenement_qu_aucun_objet_porteur_ne_recoit_est_signale() {
    let source = "ScriptName T extends Quest\nEvent OnCombatStateChanged(Actor akTarget, int aeCombatState)\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        ["OnCombatStateChanged"]
    );
}

#[test]
fn sans_certitude_le_lint_se_tait() {
    let none = Vec::<String>::new();
    let typo = "ScriptName T extends ObjectReference\nEvent OnActivatee()\nEndEvent\n";
    // No target.
    assert_eq!(events(typo, None, &Workspace::default()), none);
    // Unknown parent.
    let source = "ScriptName T extends MyBase\nEvent OnActivatee()\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        none
    );
    // Callback of a mod event.
    let source = "ScriptName T extends ObjectReference\nEvent OnInit()\n\
                  RegisterForModEvent(\"MyMod_Done\", \"OnActivatee\")\nEndEvent\n\
                  Event OnActivatee(string eventName, string strArg, float numArg, Form sender)\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        none
    );
    // Name unrelated to any known event.
    let source = "ScriptName T extends ObjectReference\nEvent OnMyModReady()\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &Workspace::default()),
        none
    );
}

#[test]
fn la_chaine_extends_traverse_les_scripts_du_projet() {
    let base = "ScriptName MyBase extends ObjectReference\nEvent OnRitualDone()\nEndEvent\n";
    let parse = papyrus_syntax::parse(base, Dialect::new(GameId::Skyrim));
    let mut workspace = Workspace::default();
    workspace.insert(ScriptSummary::of(&parse.syntax()).expect("header"));

    let source = "ScriptName T extends MyBase\nEvent OnRitualDome()\nEndEvent\nEvent OnActivatee()\nEndEvent\nEvent OnRitualDone()\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Skyrim), &workspace),
        ["OnRitualDome", "OnActivatee"]
    );
}

#[test]
fn les_evenements_distants_sont_verifies() {
    let source = "ScriptName T\nEvent ObjectReference.OnActivatee(ObjectReference akSender, ObjectReference akActionRef)\nEndEvent\nEvent ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)\nEndEvent\n";
    assert_eq!(
        events(source, Some(GameId::Fallout4), &Workspace::default()),
        ["OnActivatee"]
    );
}
