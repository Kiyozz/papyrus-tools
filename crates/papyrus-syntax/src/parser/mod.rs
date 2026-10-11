//! Le parser : des tokens vers un arbre syntaxique concret.
//!
//! Construit sur le modèle de `rust-analyzer`. La grammaire ne voit que les
//! tokens significatifs et émet des [`Event`] plats ; [`tree_builder`] les
//! rejoue en réinjectant la trivia pour obtenir un arbre `rowan` qui reproduit
//! la source octet pour octet.
//!
//! # Garanties
//!
//! - **Toujours un arbre.** Une source invalide donne un arbre complet,
//!   parsemé de nœuds [`SyntaxKind::ERROR_TREE`], et une liste d'erreurs.
//! - **Sans perte.** `parse(src).syntax().to_string() == src`, quelle que soit
//!   l'entrée.
//! - **Récupération locale.** Une erreur ne déborde pas de sa ligne, ni un bloc
//!   mal fermé de sa déclaration.

mod event;
mod grammar;
mod token_set;
mod tree_builder;

use std::cell::Cell;
use std::fmt;

use papyrus_dialect::Dialect;
use rowan::GreenNode;
use text_size::{TextRange, TextSize};

use crate::lexer::Token;
use crate::syntax_node::SyntaxNode;
use crate::{SyntaxKind, lex_with_dialect};

use event::Event;
use token_set::TokenSet;

/// Un passage que le parser n'a pas su lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
    range: TextRange,
}

impl ParseError {
    /// Ce qui ne va pas, en une phrase.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Où, en octets dans la source.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.range, self.message)
    }
}

/// Le résultat d'une analyse : un arbre, toujours, et les erreurs rencontrées.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    /// La racine de l'arbre, de kind [`SyntaxKind::SOURCE_FILE`].
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// L'arbre vert, immuable et partageable entre threads.
    pub fn green(&self) -> &GreenNode {
        &self.green
    }

    /// Les erreurs, dans l'ordre de la source.
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// Vrai si la source a été lue sans erreur.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Analyse `source` pour la cible `dialect`.
///
/// C'est le point d'entrée normal du parsing.
pub fn parse(source: &str, dialect: Dialect) -> Parse {
    let tokens = lex_with_dialect(source, dialect);
    parse_tokens(source, &tokens, dialect)
}

/// Analyse des tokens déjà produits par [`lex_with_dialect`].
///
/// # Panics
///
/// Si `tokens` n'est pas le découpage de `source`.
pub fn parse_tokens(source: &str, tokens: &[Token], dialect: Dialect) -> Parse {
    let mut p = Parser::new(tokens, TextSize::of(source), dialect);
    grammar::source_file(&mut p);
    let Parser { events, errors, .. } = p;
    let green = tree_builder::build(source, tokens, events);
    let mut errors = errors;
    errors.sort_by_key(|e| e.range.start());
    Parse { green, errors }
}

/// Pas d'analyse autorisés sans consommer de token.
///
/// Toute boucle de la grammaire doit avancer ; si elle tourne sur place, c'est
/// un bug, et mieux vaut le voir que geler l'éditeur.
const FUEL: u32 = 256;

/// L'état du parser : position dans les tokens significatifs, événements
/// émis, erreurs.
#[derive(Debug)]
pub(crate) struct Parser {
    /// Kinds des tokens significatifs, trivia retirée, suivis de `EOF`.
    kinds: Vec<SyntaxKind>,
    /// Plages correspondantes, pour situer les erreurs.
    ranges: Vec<TextRange>,
    pos: usize,
    events: Vec<Event>,
    errors: Vec<ParseError>,
    dialect: Dialect,
    fuel: Cell<u32>,
}

impl Parser {
    fn new(tokens: &[Token], end: TextSize, dialect: Dialect) -> Parser {
        let (mut kinds, mut ranges): (Vec<_>, Vec<_>) = tokens
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, t.range))
            .unzip();
        kinds.push(SyntaxKind::EOF);
        ranges.push(TextRange::empty(end));

        Parser {
            kinds,
            ranges,
            pos: 0,
            events: Vec::new(),
            errors: Vec::new(),
            dialect,
            fuel: Cell::new(FUEL),
        }
    }

    /// La cible de l'analyse.
    pub(crate) fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// Le `n`-ième token significatif à venir, `EOF` au-delà de la fin.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let fuel = self.fuel.get();
        assert!(fuel != 0, "parser bloqué à l'index {}", self.pos);
        self.fuel.set(fuel - 1);

        self.kinds
            .get(self.pos + n)
            .copied()
            .unwrap_or(SyntaxKind::EOF)
    }

    /// Le token courant.
    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// Vrai si le token courant est `kind`.
    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth(0) == kind
    }

    /// Vrai si le token courant appartient à `set`.
    pub(crate) fn at_ts(&self, set: TokenSet) -> bool {
        set.contains(self.nth(0))
    }

    /// Consomme le token courant s'il est `kind`.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if !self.at(kind) {
            return false;
        }
        self.do_bump(kind);
        true
    }

    /// Consomme le token courant, qui doit être `kind`.
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        assert!(
            self.eat(kind),
            "attendu {kind:?}, trouvé {:?}",
            self.current()
        );
    }

    /// Consomme le token courant, quel qu'il soit. Sans effet sur `EOF`.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current();
        if kind != SyntaxKind::EOF {
            self.do_bump(kind);
        }
    }

    /// Consomme `kind` ou signale son absence, sans rien consommer d'autre.
    pub(crate) fn expect(&mut self, kind: SyntaxKind, what: &str) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(format!("{what} attendu"));
        false
    }

    /// Signale une erreur sur le token courant.
    ///
    /// Une seule par token : quand une expression manque, la fin de ligne qui
    /// n'arrive pas n'est qu'une conséquence, inutile de la signaler aussi.
    pub(crate) fn error(&mut self, message: impl Into<String>) {
        let range = self.current_range();
        if self.errors.last().is_some_and(|e| e.range == range) {
            return;
        }
        self.error_at(range, message);
    }

    /// Signale une erreur sur une plage donnée.
    pub(crate) fn error_at(&mut self, range: TextRange, message: impl Into<String>) {
        self.errors.push(ParseError {
            message: message.into(),
            range,
        });
    }

    /// Plage du token courant.
    pub(crate) fn current_range(&self) -> TextRange {
        self.ranges[self.pos.min(self.ranges.len() - 1)]
    }

    /// Ouvre un nœud dont le kind sera fixé à la complétion.
    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.events.push(Event::tombstone());
        Marker { pos }
    }

    fn do_bump(&mut self, kind: SyntaxKind) {
        self.pos += 1;
        self.fuel.set(FUEL);
        self.events.push(Event::Token { kind });
    }
}

/// Un nœud ouvert, en attente de son kind.
#[derive(Debug)]
#[must_use = "un marqueur doit être complété ou abandonné"]
pub(crate) struct Marker {
    pos: u32,
}

impl Marker {
    /// Ferme le nœud et lui donne son kind.
    pub(crate) fn complete(self, p: &mut Parser, kind: SyntaxKind) -> CompletedMarker {
        match &mut p.events[self.pos as usize] {
            Event::Start { kind: slot, .. } => *slot = kind,
            _ => unreachable!("un marqueur pointe toujours sur un Start"),
        }
        p.events.push(Event::Finish);
        CompletedMarker { pos: self.pos }
    }

    /// Renonce au nœud ; ses enfants remontent dans le parent.
    pub(crate) fn abandon(self, p: &mut Parser) {
        if self.pos as usize == p.events.len() - 1 {
            p.events.pop();
        }
    }
}

/// Un nœud fermé, qu'on peut encore envelopper.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompletedMarker {
    pos: u32,
}

impl CompletedMarker {
    /// Ouvre un nœud qui contiendra celui-ci.
    ///
    /// C'est ce qui permet de lire `a + b` de gauche à droite : on découvre
    /// qu'`a` est l'opérande gauche d'une addition après l'avoir terminé.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        let parent = p.start();
        match &mut p.events[self.pos as usize] {
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some(parent.pos - self.pos);
            }
            _ => unreachable!("un marqueur pointe toujours sur un Start"),
        }
        parent
    }
}
