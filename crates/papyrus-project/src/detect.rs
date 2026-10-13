//! Guessing the game of a script when nobody said it.
//!
//! Two kinds of clues, read on a tree parsed with the widest dialect:
//!
//! - **syntax**, which is certain: `LockGuard` only exists in Starfield, a
//!   struct, an `Is` or a namespace require at least Fallout 4. It sets a
//!   minimum game;
//! - **names**, which vote: a type, a native function or an engine event
//!   only some games know (`OnUpdate` is specific to Skyrim, `StartTimer`
//!   does not exist before Fallout 4). The `papyrus-stdlib` tables say who
//!   knows them.
//!
//! A script may give nothing away: the verdict is then `None`, and the
//! project decides, by the majority of its decisive files.
//!
//! Mistaking Fallout 4 for Starfield is harmless — same flags, same reading
//! of the encoding, and Starfield's dialect includes Fallout 4's. Only the
//! border with Skyrim matters.

use std::collections::HashSet;

use papyrus_dialect::GameId;
use papyrus_stdlib::{has_event, has_function, script};
use papyrus_syntax::{SyntaxKind, SyntaxNode};

/// The games a clue can point to, in rank order.
const GAMES: [GameId; 3] = [GameId::Skyrim, GameId::Fallout4, GameId::Starfield];

/// What a script gives away about its game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evidence {
    /// The minimum game the syntax requires.
    pub minimum: GameId,
    /// The name votes, per game, in rank order: Skyrim, Fallout 4, Starfield.
    pub votes: [u32; 3],
}

impl Evidence {
    /// The designated game, if unambiguous.
    ///
    /// A tie between Fallout 4 and Starfield resolves to Starfield, harmlessly:
    /// only a tie involving Skyrim stays undecided.
    pub fn verdict(&self) -> Option<GameId> {
        let candidates: Vec<(GameId, u32)> = GAMES
            .into_iter()
            .zip(self.votes)
            .filter(|(game, _)| game.accepts(self.minimum))
            .collect();
        let best = candidates.iter().map(|(_, v)| *v).max().unwrap_or(0);
        if best == 0 {
            // No name votes: only Starfield syntax decides.
            return (self.minimum == GameId::Starfield).then_some(GameId::Starfield);
        }
        let leaders: Vec<GameId> = candidates
            .iter()
            .filter(|(_, v)| *v == best)
            .map(|(g, _)| *g)
            .collect();
        // A one-vote lead is not enough: a Skyrim script may declare an
        // `Event OnStart()` its engine never sends. Two are needed.
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

/// The implicit members of every script.
///
/// Fallout 4 and Starfield declare them in `ScriptObject.psc`; Skyrim knows
/// them without declaring them anywhere. Their absence from Skyrim's tables
/// therefore proves nothing: they do not vote.
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

/// The significant text of a node, without trivia.
fn compact(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_owned())
        .collect()
}

/// The minimum game the syntax of `root` requires.
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
            // `Event Actor.OnDeath(…)`: a remote event.
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

/// The names the script declares itself: a call to one of them says nothing
/// about the game.
fn declared(root: &SyntaxNode) -> HashSet<String> {
    root.descendants()
        .filter(|n| n.kind() == SyntaxKind::FUNCTION_DECL)
        .filter_map(|f| f.children().find(|c| c.kind() == SyntaxKind::NAME))
        .map(|n| compact(&n).to_ascii_lowercase())
        .collect()
}

/// A name that voted, and the games that know it, in rank order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    /// The name, as written.
    pub name: String,
    /// Who knows it: Skyrim, Fallout 4, Starfield.
    pub known: [bool; 3],
}

/// The names of `root` that vote: those only some games know.
pub fn explain(root: &SyntaxNode) -> Vec<Vote> {
    let own = declared(root);
    let mut votes = Vec::new();
    let mut vote = |name: String, known: [bool; 3]| {
        // A name everyone knows, or nobody knows, says nothing.
        let some = known.iter().any(|k| *k);
        let all = known.iter().all(|k| *k);
        if some && !all {
            votes.push(Vote { name, known });
        }
    };

    for node in root.descendants() {
        match node.kind() {
            // A type: `Actor`, `SpaceshipReference`. Qualified names
            // (`DLC01:Foo`) belong to project scripts.
            SyntaxKind::TYPE_REF => {
                let name = compact(&node);
                let name = name.trim_end_matches("[]");
                if !name.contains(':') {
                    vote(name.to_owned(), GAMES.map(|g| script(g, name).is_some()));
                }
            }
            // An engine event: `OnUpdate`, `OnTimer`.
            SyntaxKind::EVENT_DECL => {
                if let Some(name) = node.children().find(|c| c.kind() == SyntaxKind::NAME) {
                    let name = compact(&name);
                    if !is_universal(&name) {
                        let known = GAMES.map(|g| has_event(g, &name));
                        vote(name, known);
                    }
                }
            }
            // A called function: `RegisterForSingleUpdate()`,
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

/// What `root` gives away about its game. The tree must come from the widest
/// dialect, Starfield's.
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

/// The game of a project: the majority of its files' decisive verdicts.
///
/// Also returns the number of files that designated it, and of decisive
/// files overall.
pub fn majority(
    verdicts: impl IntoIterator<Item = Option<GameId>>,
) -> Option<(GameId, usize, usize)> {
    let mut counts = [0usize; 3];
    for game in verdicts.into_iter().flatten() {
        let slot = GAMES
            .iter()
            .position(|g| g.rank() == game.rank())
            .expect("known game");
        counts[slot] += 1;
    }
    let decided: usize = counts.iter().sum();
    let (slot, &best) = counts
        .iter()
        .enumerate()
        // On a tie, the most recent: its dialect includes the others.
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
        // `OnUpdate` votes Skyrim, but a struct rules it out.
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
        // `StartTimer` declared here is not the engine's.
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
