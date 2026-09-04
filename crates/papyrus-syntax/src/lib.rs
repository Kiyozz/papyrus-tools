//! Syntaxe du langage Papyrus : vocabulaire de l'arbre et lexer.
//!
//! Ce crate ne fait aucune entrée-sortie et ne dépend d'aucune base de données.
//! [`lex_with_dialect`] est une fonction pure du texte et de la cible, ce qui la
//! rend cacheable telle quelle et permettra d'introduire l'incrémentalité plus
//! tard sans toucher à l'architecture.
//!
//! # Exemple
//!
//! ```
//! use papyrus_dialect::{Dialect, GameId};
//! use papyrus_syntax::{SyntaxKind, lex_with_dialect};
//!
//! let source = "Function Foo()\n\tint x = 1\nEndFunction\n";
//! let tokens = lex_with_dialect(source, Dialect::new(GameId::Skyrim));
//!
//! assert_eq!(tokens[0].kind, SyntaxKind::FUNCTION_KW);
//! assert_eq!(tokens[0].text(source), "Function");
//!
//! // La source se reconstruit à l'identique.
//! let rebuilt: String = tokens.iter().map(|t| t.text(source)).collect();
//! assert_eq!(rebuilt, source);
//! ```

#![forbid(unsafe_code)]

pub mod keywords;
pub mod lexer;
mod syntax_kind;
pub mod token_source;

pub use lexer::{Token, lex};
pub use syntax_kind::SyntaxKind;
pub use token_source::{lex_with_dialect, reserved_later};
