//! Reading Papyrus sources: configuration, decoding and file discovery.
//!
//! The only crate of the toolchain that touches the disk. It only reads what
//! it is pointed at: no registry, no guessed installation path.

#![forbid(unsafe_code)]

pub mod config;
pub mod detect;
pub mod discover;
pub mod encoding;
pub mod flags;

pub use config::{Config, ConfigError, IndentSetting, Level};
pub use discover::{DiscoverOptions, discover};
pub use encoding::{Decoded, SourceEncoding, decode, encode};
