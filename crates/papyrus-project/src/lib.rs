//! Lecture des sources Papyrus : décodage et découverte des fichiers.
//!
//! Le seul crate de la toolchain qui touche au disque. Il ne lit que ce qu'on
//! lui désigne : ni registre, ni chemin d'installation deviné.

#![forbid(unsafe_code)]

pub mod discover;
pub mod encoding;

pub use discover::{DiscoverOptions, discover};
pub use encoding::{Decoded, SourceEncoding, decode, encode};
