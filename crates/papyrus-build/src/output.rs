//! What the compilers print.
//!
//! The Creation Kit's compiler, on any game:
//!
//! ```text
//! C:\Mod\Scripts\Source\Foo.psc(3,5): type mismatch while assigning to a int (cast missing or types unrelated)
//! <unknown>(0,0): unable to locate script Bar
//! ```
//!
//! Lines from 1, columns from 0 in characters, a tab counting as one.
//! Caprica:
//!
//! ```text
//! C:\Mod\Scripts\Source\Foo.psc (3, 6:7): Error: Unknown type 'Foo'!
//! C:\Mod\Scripts\Source\Foo.psc (8, 2:9): Warning W4001: ...
//! ```
//!
//! Lines and columns from 1, columns in bytes, with the end of the span.
//! The path is relative to the folder compiled.

use std::path::PathBuf;

use papyrus_syntax::{TextRange, TextSize};

use crate::CompilerKind;

/// How serious a message is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The script does not compile.
    Error,
    /// It compiles, with a remark.
    Warning,
}

/// Where in its line a message points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// A character, from 0: the Creation Kit's compiler.
    Char(u32),
    /// A span of bytes, from 1, end excluded: Caprica.
    Bytes(u32, u32),
}

/// A problem a compiler reported, in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The file, as printed.
    pub path: PathBuf,
    /// The line, from 1.
    pub line: u32,
    /// Where in the line.
    pub position: Position,
    /// Error or warning.
    pub severity: Severity,
    /// What the compiler says.
    pub text: String,
    /// Caprica's warning number, `W4001`.
    pub code: Option<String>,
}

/// The located messages of a compiler's output, and the other lines worth
/// showing.
pub fn parse(kind: CompilerKind, output: &str) -> (Vec<Message>, Vec<String>) {
    let mut messages = Vec::new();
    let mut other = Vec::new();
    for line in output.lines().map(str::trim_end) {
        if line.is_empty() || noise(line) {
            continue;
        }
        let message = match kind {
            CompilerKind::PapyrusCompiler => papyrus_compiler(line),
            CompilerKind::Caprica => caprica(line),
        };
        match message {
            Some(message) => messages.push(message),
            None => other.push(general(line).to_owned()),
        }
    }
    (messages, other)
}

/// Progress and summaries, which the summary of `papyrus build` replaces.
fn noise(line: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "Papyrus Compiler Version",
        "Copyright (C)",
        "Starting ",
        "Compiling ",
        "No output generated for",
        "Batch compile of",
        "Failed on ",
        "Compilation succeeded",
        "Assembly succeeded",
        "0 error(s), 0 warning(s)",
        "Compilation of '",
        "Adding import directory",
        "Adding current working directory",
        "Importing files...",
        "Imported ",
    ];
    PREFIXES.iter().any(|p| line.starts_with(p))
}

/// A line without a file: `<unknown>(0,0): ` dropped.
fn general(line: &str) -> &str {
    line.strip_prefix("<unknown>(0,0): ").unwrap_or(line)
}

/// `path(line,column): message`.
fn papyrus_compiler(line: &str) -> Option<Message> {
    let (location, text) = line.split_once("): ")?;
    let (path, numbers) = location.rsplit_once('(')?;
    if path == "<unknown>" || path.is_empty() {
        return None;
    }
    let (line_number, column) = numbers.split_once(',')?;
    let (severity, text) = match text.strip_prefix("warning: ") {
        Some(rest) => (Severity::Warning, rest),
        None => (Severity::Error, text),
    };
    Some(Message {
        path: PathBuf::from(path),
        line: line_number.trim().parse().ok()?,
        position: Position::Char(column.trim().parse().ok()?),
        severity,
        text: text.to_owned(),
        code: None,
    })
}

/// `path (line, start:end): Error: message`, or `Warning W1234: message`.
fn caprica(line: &str) -> Option<Message> {
    let (location, rest) = line.split_once("): ")?;
    let (path, numbers) = location.rsplit_once(" (")?;
    let (line_number, span) = numbers.split_once(", ")?;
    let (start, end) = span.split_once(':')?;
    let (kind, text) = rest.split_once(": ")?;
    let (severity, code) = match kind.split_once(' ') {
        Some((word, code)) if code.starts_with('W') => (word, Some(code.to_owned())),
        _ => (kind, None),
    };
    let severity = match severity {
        "Warning" => Severity::Warning,
        "Error" | "Fatal Error" | "Fatal" => Severity::Error,
        _ => return None,
    };
    Some(Message {
        path: PathBuf::from(path),
        line: line_number.trim().parse().ok()?,
        position: Position::Bytes(start.trim().parse().ok()?, end.trim().parse().ok()?),
        severity,
        text: text.to_owned(),
        code,
    })
}

/// The range a message points to in `source`: Caprica's span, or the word
/// at the Creation Kit's column.
pub fn range_of(source: &str, line: u32, position: Position) -> TextRange {
    let start_of_line = source
        .split_inclusive('\n')
        .take(line.saturating_sub(1) as usize)
        .map(str::len)
        .sum::<usize>()
        .min(source.len());
    let text = &source[start_of_line..];
    let text = &text[..text.find(['\r', '\n']).unwrap_or(text.len())];
    let clamp = |offset: usize| {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    let (start, end) = match position {
        Position::Bytes(start, end) => {
            let start = clamp(start.saturating_sub(1) as usize);
            (start, clamp(end.saturating_sub(1) as usize).max(start))
        }
        Position::Char(column) => {
            let start = text
                .char_indices()
                .nth(column as usize)
                .map_or(text.len(), |(i, _)| i);
            let word = text[start..]
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(text.len() - start);
            (start, start + word)
        }
    };
    let at = |offset: usize| TextSize::new((start_of_line + offset) as u32);
    TextRange::new(at(start), at(end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_creation_kit_compiler_is_read() {
        let output = "Starting 2 compile threads for 2 files...\n\
            Compiling \"BuildTest\"...\n\
            C:\\Mod\\Source\\BuildTest.psc(3,5): type mismatch while assigning to a int (cast missing or types unrelated)\n\
            C:\\Mod\\Source\\BuildTest.psc(4,1): Foo is not a function or does not exist\n\
            <unknown>(0,0): unable to locate script NoSuch\n\
            No output generated for BuildTest.psc, compilation failed.\n\
            Batch compile of 2 files finished. 1 succeeded, 1 failed.\n\
            Failed on BuildTest.psc\n";
        let (messages, other) = parse(CompilerKind::PapyrusCompiler, output);
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].path,
            PathBuf::from("C:\\Mod\\Source\\BuildTest.psc")
        );
        assert_eq!(
            (messages[0].line, messages[0].position),
            (3, Position::Char(5))
        );
        assert_eq!(messages[1].text, "Foo is not a function or does not exist");
        assert_eq!(other, ["unable to locate script NoSuch"]);
    }

    #[test]
    fn caprica_is_read() {
        let output = "Importing files...\n\
            Imported 14326 files.\n\
            C:\\Mod\\Foo.psc (3, 6:7): Error: Unknown type 'Foo'!\n\
            C:\\Mod\\Foo.psc (8, 2:9): Warning W4001: The value is never used.\n\
            Compilation of 'C:\\Mod\\Foo.psc' failed; 1 warnings and 1 errors were encountered.\n\
            Fatal Error: Unable to find the flags file!\n";
        let (messages, other) = parse(CompilerKind::Caprica, output);
        assert_eq!(messages.len(), 2);
        assert_eq!(
            (messages[0].line, messages[0].position, messages[0].severity),
            (3, Position::Bytes(6, 7), Severity::Error)
        );
        assert_eq!(messages[0].text, "Unknown type 'Foo'!");
        assert_eq!(messages[1].severity, Severity::Warning);
        assert_eq!(messages[1].code.as_deref(), Some("W4001"));
        assert_eq!(other, ["Fatal Error: Unable to find the flags file!"]);
    }

    #[test]
    fn positions_become_ranges() {
        let source = "ScriptName A\r\nFunction F()\r\n\tint y = \"é\"\r\nEndFunction\r\n";
        let range = range_of(source, 3, Position::Char(5));
        assert_eq!(&source[range], "y");
        let range = range_of(source, 2, Position::Bytes(10, 11));
        assert_eq!(&source[range], "F");
        // Past the end: clamped to the line.
        let range = range_of(source, 9, Position::Char(40));
        assert!(usize::from(range.start()) <= source.len());
    }
}
