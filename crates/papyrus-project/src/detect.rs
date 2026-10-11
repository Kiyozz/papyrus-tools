//! Deviner le jeu d'un script quand personne ne l'a dit.
//!
//! Deux sortes d'indices, lus sur un arbre parsé avec le dialecte le plus
//! large :
//!
//! - **la syntaxe**, qui est certaine : `LockGuard` n'existe qu'en Starfield,
//!   une structure, un `Is` ou un namespace demandent au moins Fallout 4. Elle
//!   fixe un jeu minimal ;
//! - **les noms**, qui votent : un type, une fonction native ou un événement
//!   du moteur que certains jeux seulement connaissent (`OnUpdate` est propre
//!   à Skyrim, `StartTimer` n'existe pas avant Fallout 4). Les tables de
//!   `papyrus-stdlib` disent qui les connaît.
//!
//! Un script peut ne rien trahir : le verdict est alors `None`, et c'est au
//! projet de trancher, à la majorité de ses fichiers décisifs.
//!
//! Confondre Fallout 4 et Starfield est sans conséquence — mêmes flags, même
//! lecture de l'encodage, et le dialecte de Starfield englobe celui de
//! Fallout 4. Seule compte la frontière avec Skyrim.

use std::collections::HashSet;

use papyrus_dialect::GameId;
use papyrus_stdlib::{has_event, has_function, script};
use papyrus_syntax::{SyntaxKind, SyntaxNode};

/// Les jeux qu'un indice peut désigner, dans l'ordre de leur rang.
const GAMES: [GameId; 3] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield];

/// Ce qu'un script trahit de son jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidence {
    /// Le jeu minimal qu'impose la syntaxe.
    pub minimum: GameId,
    /// Les voix des noms, par jeu, dans l'ordre de [`GAMES`].
    pub votes: [u32; 3],
}

impl Evidence {
    /// Le jeu désigné, s'il l'est sans ambiguïté.
    ///
    /// Une égalité entre Fallout 4 et Starfield se résout en faveur de
    /// Starfield, sans conséquence : seule une égalité qui implique Skyrim
    /// reste indécise.
    pub fn verdict(&self) -> Option<GameId> {
        let candidates: Vec<(GameId, u32)> = GAMES
            .into_iter()
            .zip(self.votes)
            .filter(|(game, _)| game.accepts(self.minimum))
            .collect();
        let best = candidates.iter().map(|(_, v)| *v).max().unwrap_or(0);
        if best == 0 {
            // Aucun nom ne vote : seule une syntaxe de Starfield tranche.
            return (self.minimum == GameId::Starfield).then_some(GameId::Starfield);
        }
        let leaders: Vec<GameId> = candidates
            .iter()
            .filter(|(_, v)| *v == best)
            .map(|(g, _)| *g)
            .collect();
        // Une voix d'avance ne suffit pas : un script Skyrim peut déclarer un
        // `Event OnStart()` que son moteur n'émet jamais. Il en faut deux.
        let runner_up = candidates
            .iter()
            .filter(|(_, v)| *v < best)
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(0);
        if best - runner_up < 2 {
            return (self.minimum == GameId::Starfield).then_some(GameId::Starfield);
        }
        match leaders.as_slice() {
            [game] => Some(*game),
            [GameId::Fallout4, GameId::Starfield] => Some(GameId::Starfield),
            _ => None,
        }
    }
}

/// Les membres implicites de tout script.
///
/// Fallout 4 et Starfield les déclarent dans `ScriptObject.psc` ; Skyrim les
/// connaît sans les déclarer nulle part. Leur absence des tables de Skyrim ne
/// prouve donc rien : ils ne votent pas.
const UNIVERSAL: [&str; 5] = [
    "OnInit",
    "OnBeginState",
    "OnEndState",
    "GotoState",
    "GetState",
];

fn is_universal(name: &str) -> bool {
    UNIVERSAL.iter().any(|u| u.eq_ignore_ascii_case(name))
}

/// Le texte significatif d'un nœud, sans trivia.
fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_owned())
        .collect()
}

/// Le jeu minimal qu'impose la syntaxe de `root`.
fn minimum(root: &SyntaxNode) -> GameId {
    let mut minimum = GameId::Skyrim;
    for element in root.descendants_with_tokens() {
        let parent = element.parent().map(|p| p.kind());
        let needs = match (element.kind(), parent) {
            (
                SyntaxKind::LOCK_GUARD_STMT
                | SyntaxKind::TRY_LOCK_GUARD_STMT
                | SyntaxKind::GUARD_DECL,
                _,
            ) => GameId::Starfield,
            (
                SyntaxKind::PRIVATE_KW
                | SyntaxKind::PROTECTED_KW
                | SyntaxKind::SELFONLY_KW
                | SyntaxKind::REQUIRESGUARD_KW
                | SyntaxKind::PROTECTSFUNCTIONLOGIC_KW,
                Some(SyntaxKind::FLAG),
            ) => GameId::Starfield,
            (
                SyntaxKind::STRUCT_DECL | SyntaxKind::GROUP_DECL | SyntaxKind::CUSTOM_EVENT_DECL,
                _,
            ) => GameId::Fallout4,
            (SyntaxKind::IS_KW, Some(SyntaxKind::CAST_EXPR)) => GameId::Fallout4,
            (SyntaxKind::COLON, _) => GameId::Fallout4,
            (
                SyntaxKind::VAR_KW
                | SyntaxKind::CUSTOMEVENTNAME_KW
                | SyntaxKind::SCRIPTEVENTNAME_KW
                | SyntaxKind::STRUCTVARNAME_KW,
                Some(SyntaxKind::TYPE_REF),
            ) => GameId::Fallout4,
            (
                SyntaxKind::CONST_KW
                | SyntaxKind::MANDATORY_KW
                | SyntaxKind::DEBUGONLY_KW
                | SyntaxKind::BETAONLY_KW
                | SyntaxKind::DEFAULT_KW
                | SyntaxKind::COLLAPSED_KW
                | SyntaxKind::COLLAPSEDONREF_KW
                | SyntaxKind::COLLAPSEDONBASE_KW,
                Some(SyntaxKind::FLAG),
            ) => GameId::Fallout4,
            // `Event Actor.OnDeath(…)` : un événement distant.
            (SyntaxKind::TYPE_REF, Some(SyntaxKind::EVENT_DECL)) => GameId::Fallout4,
            _ => continue,
        };
        if needs.rank() > minimum.rank() {
            minimum = needs;
        }
        if minimum == GameId::Starfield {
            break;
        }
    }
    minimum
}

/// Les noms que déclare le script lui-même : un appel à l'un d'eux ne dit
/// rien du jeu.
fn declared(root: &SyntaxNode) -> HashSet<String> {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::FUNCTION_DECL)
        .filter_map(|f| f.children().find(|c| c.kind() == SyntaxKind::NAME))
        .map(|n| compact(&n).to_ascii_lowercase())
        .collect()
}

/// Un nom qui a voté, et les jeux qui le connaissent, dans l'ordre de
/// [`GAMES`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    /// Le nom, tel qu'écrit.
    pub name: String,
    /// Qui le connaît : Skyrim, Fallout 4, Starfield.
    pub known: [bool; 3],
}

/// Les noms de `root` qui votent : ceux que certains jeux seulement
/// connaissent.
pub fn explain(root: &SyntaxNode) -> Vec<Vote> {
    let own = declared(root);
    let mut votes = Vec::new();
    let mut vote = |name: String, known: [bool; 3]| {
        // Un nom que tous connaissent, ou qu'aucun ne connaît, ne dit rien.
        let some = known.iter().any(|k| *k);
        let all = known.iter().all(|k| *k);
        if some && !all {
            votes.push(Vote { name, known });
        }
    };

    for node in root.descendants() {
        match node.kind() {
            // Un type : `Actor`, `SpaceshipReference`. Les noms qualifiés
            // (`DLC01:Foo`) sont ceux de scripts du projet.
            SyntaxKind::TYPE_REF => {
                let name = compact(&node);
                let name = name.trim_end_matches("[]");
                if !name.contains(':') {
                    vote(name.to_owned(), GAMES.map(|g| script(g, name).is_some()));
                }
            }
            // Un événement du moteur : `OnUpdate`, `OnTimer`.
            SyntaxKind::EVENT_DECL => {
                if let Some(name) = node.children().find(|c| c.kind() == SyntaxKind::NAME) {
                    let name = compact(&name);
                    if !is_universal(&name) {
                        let known = GAMES.map(|g| has_event(g, &name));
                        vote(name, known);
                    }
                }
            }
            // Une fonction appelée : `RegisterForSingleUpdate()`,
            // `Game.GetPlayer()`.
            SyntaxKind::CALL_EXPR => {
                let Some(callee) = node.first_child() else {
                    continue;
                };
                let name = match callee.kind() {
                    SyntaxKind::NAME_REF => compact(&callee),
                    SyntaxKind::FIELD_EXPR => match callee.last_child() {
                        Some(member) => compact(&member),
                        None => continue,
                    },
                    _ => continue,
                };
                if !own.contains(&name.to_ascii_lowercase()) && !is_universal(&name) {
                    let known = GAMES.map(|g| has_function(g, &name));
                    vote(name, known);
                }
            }
            _ => {}
        }
    }
    votes
}

/// Ce que `root` trahit de son jeu. L'arbre doit venir du dialecte le plus
/// large, celui de Starfield.
pub fn evidence(root: &SyntaxNode) -> Evidence {
    let mut votes = [0u32; 3];
    for vote in explain(root) {
        for (slot, known) in votes.iter_mut().zip(vote.known) {
            *slot += u32::from(known);
        }
    }
    Evidence {
        minimum: minimum(root),
        votes,
    }
}

/// Le jeu d'un projet : la majorité des verdicts décisifs de ses fichiers.
///
/// Rend aussi le nombre de fichiers qui l'ont désigné, et de fichiers
/// décisifs en tout.
pub fn majority(
    verdicts: impl IntoIterator<Item = Option<GameId>>,
) -> Option<(GameId, usize, usize)> {
    let mut counts = [0usize; 3];
    for game in verdicts.into_iter().flatten() {
        let slot = GAMES
            .iter()
            .position(|g| g.rank() == game.rank())
            .expect("jeu connu");
        counts[slot] += 1;
    }
    let decided: usize = counts.iter().sum();
    let (slot, &best) = counts
        .iter()
        .enumerate()
        // À égalité, le plus récent : son dialecte englobe les autres.
        .max_by_key(|(slot, count)| (**count, *slot))?;
    (best > 0).then_some((GAMES[slot], best, decided))
}

#[cfg(test)]
mod tests {
    use super::*;
    use papyrus_dialect::Dialect;

    fn detect(source: &str) -> Option<GameId> {
        let parse = papyrus_syntax::parse(source, Dialect::new(GameId::Starfield));
        evidence(&parse.syntax()).verdict()
    }

    #[test]
    fn la_syntaxe_de_starfield_tranche_seule() {
        let source =
            "ScriptName T\nGuard g\nFunction F()\n\tLockGuard g\n\tEndLockGuard\nEndFunction\n";
        assert_eq!(detect(source), Some(GameId::Starfield));
    }

    #[test]
    fn onupdate_designe_skyrim() {
        let source = "ScriptName T extends Quest\nEvent OnInit()\n\tRegisterForSingleUpdate(1.0)\nEndEvent\nEvent OnUpdate()\nEndEvent\n";
        assert_eq!(detect(source), Some(GameId::Skyrim));
    }

    #[test]
    fn les_timers_ecartent_skyrim() {
        let source = "ScriptName T extends Quest\nEvent OnInit()\n\tStartTimer(1.0)\nEndEvent\nEvent OnTimer(int aiTimerID)\nEndEvent\n";
        assert_ne!(detect(source), Some(GameId::Skyrim));
        assert!(detect(source).is_some());
    }

    #[test]
    fn la_syntaxe_ecarte_skyrim_malgre_les_noms() {
        // `OnUpdate` vote Skyrim, mais une structure l'exclut.
        let source = "ScriptName T extends Quest\nStruct S\n\tint x\nEndStruct\nEvent OnUpdate()\nEndEvent\n";
        assert_ne!(detect(source), Some(GameId::Skyrim));
    }

    #[test]
    fn un_script_neutre_ne_tranche_pas() {
        let source = "ScriptName T extends Quest\nFunction F()\n\tint x = 1\nEndFunction\n";
        assert_eq!(detect(source), None);
    }

    #[test]
    fn un_appel_a_une_fonction_du_script_ne_vote_pas() {
        // `StartTimer` déclarée ici n'est pas celle du moteur.
        let source = "ScriptName T extends Quest\nFunction StartTimer(float x)\nEndFunction\nEvent OnInit()\n\tStartTimer(1.0)\nEndEvent\n";
        assert_eq!(detect(source), None);
    }

    #[test]
    fn la_majorite_du_projet_l_emporte() {
        use GameId::*;
        assert_eq!(
            majority([Some(Skyrim), None, Some(Skyrim), Some(Fallout4)]),
            Some((Skyrim, 2, 3))
        );
        assert_eq!(majority([None, None]), None);
        assert_eq!(
            majority([Some(Fallout4), Some(Starfield)]),
            Some((Starfield, 1, 2))
        );
    }
}
