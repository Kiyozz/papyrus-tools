//! Do the generated tables tell the truth?

use papyrus_dialect::GameId;
use papyrus_stdlib::{ancestors, find_event, find_function, script, scripts};

const GAMES: [GameId; 3] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield];

#[test]
fn the_tables_are_sorted_for_lookup() {
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
fn lookup_ignores_case() {
    for game in GAMES {
        assert!(script(game, "objectreference").is_some(), "{game}");
        assert!(script(game, "OBJECTREFERENCE").is_some(), "{game}");
        assert!(script(game, "PasUnScript").is_none(), "{game}");
    }
}

#[test]
fn actor_inherits_from_objectreference() {
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
fn an_inherited_event_is_found() {
    // `OnActivate` is declared on ObjectReference; a script extending Actor
    // receives it.
    for game in GAMES {
        let event = find_event(game, "Actor", "onactivate").expect("OnActivate");
        assert_eq!(event.name, "OnActivate", "{game}");
        assert_eq!(event.params[0].ty, "ObjectReference", "{game}");
        assert!(find_event(game, "Actor", "OnActivatee").is_none(), "{game}");
    }
}

#[test]
fn skyrim_declares_onupdate_on_form() {
    assert!(find_event(GameId::Skyrim, "Quest", "OnUpdate").is_some());
}

#[test]
fn native_functions_keep_their_flags() {
    let wait = find_function(GameId::Skyrim, "Utility", "Wait").expect("Utility.Wait");
    assert!(wait.global && wait.native);
    assert_eq!(wait.params[0].ty, "float");
}

#[test]
fn fallout76_reuses_fallout4() {
    assert_eq!(scripts(GameId::Fallout76), scripts(GameId::Fallout4));
}

#[test]
fn name_indexes_tell_the_games_apart() {
    use papyrus_stdlib::{has_event, has_function};
    // Skyrim updates through `OnUpdate`, Fallout 4 and Starfield through timers.
    assert!(has_event(GameId::Skyrim, "onupdate"));
    assert!(!has_event(GameId::Fallout4, "OnUpdate"));
    assert!(has_function(GameId::Skyrim, "RegisterForSingleUpdate"));
    assert!(has_function(GameId::Fallout4, "StartTimer"));
    assert!(!has_function(GameId::Skyrim, "StartTimer"));
}
