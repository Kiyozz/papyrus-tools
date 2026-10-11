//! Syntaxe du langage Papyrus : lexer, parser et arbre syntaxique concret.
//!
//! Ce crate ne fait aucune entrée-sortie et ne dépend d'aucune base de données.
//! [`parse`] est une fonction pure du texte et de la cible, ce qui la rend
//! cacheable telle quelle et permettra d'introduire l'incrémentalité plus tard
//! sans toucher à l'architecture.
//!
//! # Exemple
//!
//! ```
//! use papyrus_dialect::{Dialect, GameId};
//! use papyrus_syntax::{SyntaxKind, parse};
//!
//! let source = "ScriptName Foo\n\nFunction Bar()\n\tint x = 1\nEndFunction\n";
//! let parse = parse(source, Dialect::new(GameId::Skyrim));
//!
//! assert!(parse.ok());
//! let root = parse.syntax();
//! assert_eq!(root.kind(), SyntaxKind::SOURCE_FILE);
//!
//! // L'arbre reproduit la source à l'identique.
//! assert_eq!(root.to_string(), source);
//! ```

#![forbid(unsafe_code)]

pub mod keywords;
pub mod lexer;
mod parser;
mod syntax_kind;
mod syntax_node;
pub mod token_source;

pub use lexer::{Token, lex};
pub use parser::{Parse, ParseError, parse, parse_tokens};
pub use rowan::{NodeOrToken, TextRange, TextSize};
pub use syntax_kind::SyntaxKind;
pub use syntax_node::{
    Papyrus, SyntaxElement, SyntaxElementChildren, SyntaxNode, SyntaxNodeChildren, SyntaxToken,
};
pub use token_source::{lex_with_dialect, reserved_later};
