//! Syntax of the Papyrus language: lexer, parser and concrete syntax tree.
//!
//! This crate does no I/O and depends on no database. [`parse`] is a pure
//! function of the text and the target, which makes it cacheable as is and
//! will allow adding incrementality later without touching the architecture.
//!
//! # Example
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
//! // The tree reproduces the source exactly.
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
