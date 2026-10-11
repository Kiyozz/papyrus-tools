//! Les tables générées disent-elles vrai ?

use papyrus_dialect::GameId;
use papyrus_stdlib::{ancestors, find_event, find_function, script, scripts};

const GAMES: [GameId; 3] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield];

#[test]
fn les_tables_sont_triees_pour_la_recherche() {
    for game in GAMES {
        let names: Vec<String> = scripts(game)
            .iter()
            .map(|s| s.name.to_ascii_lowercase())
            .collect();
        assert!(names.windows(2).all(|w| w[0] < w[1]), "{game}");
        for s in scripts(game) {
            for list in [s.functions, s.events] {
                let names: Vec<String> = list.iter().map(|f| f.name.to_ascii_lowercase()).collect();
                assert!(names.windows(2).all(|w| w[0] < w[1]), "{game} {}", s.name);
            }
        }
    }
}

#[test]
fn la_recherche_ignore_la_casse() {
    for game in GAMES {
        assert!(script(game, "objectreference").is_some(), "{game}");
        assert!(script(game, "OBJECTREFERENCE").is_some(), "{game}");
        assert!(script(game, "PasUnScript").is_none(), "{game}");
    }
}

#[test]
fn actor_herite_d_objectreference() {
    for game in GAMES {
        let chain: Vec<&str> = ancestors(game, "Actor").map(|s| s.name).collect();
        assert_eq!(chain[0], "Actor", "{game}");
        assert!(
            chain
                .iter()
                .any(|n| n.eq_ignore_ascii_case("ObjectReference")),
            "{game} : {chain:?}"
        );
    }
}

#[test]
fn un_evenement_herite_se_retrouve() {
    // `OnActivate` est déclaré sur ObjectReference ; un script qui étend
    // Actor le reçoit.
    for game in GAMES {
        let event = find_event(game, "Actor", "onactivate").expect("OnActivate");
        assert_eq!(event.name, "OnActivate", "{game}");
        assert_eq!(event.params[0].ty, "ObjectReference", "{game}");
        assert!(find_event(game, "Actor", "OnActivatee").is_none(), "{game}");
    }
}

#[test]
fn skyrim_declare_onupdate_sur_form() {
    assert!(find_event(GameId::Skyrim, "Quest", "OnUpdate").is_some());
}

#[test]
fn les_fonctions_natives_gardent_leurs_flags() {
    let wait = find_function(GameId::Skyrim, "Utility", "Wait").expect("Utility.Wait");
    assert!(wait.global && wait.native);
    assert_eq!(wait.params[0].ty, "float");
}

#[test]
fn fallout76_reprend_fallout4() {
    assert_eq!(scripts(GameId::Fallout76), scripts(GameId::Fallout4));
}

#[test]
fn les_index_de_noms_distinguent_les_jeux() {
    use papyrus_stdlib::{has_event, has_function};
    // Skyrim met à jour par `OnUpdate`, Fallout 4 et Starfield par des timers.
    assert!(has_event(GameId::Skyrim, "onupdate"));
    assert!(!has_event(GameId::Fallout4, "OnUpdate"));
    assert!(has_function(GameId::Skyrim, "RegisterForSingleUpdate"));
    assert!(has_function(GameId::Fallout4, "StartTimer"));
    assert!(!has_function(GameId::Skyrim, "StartTimer"));
}
