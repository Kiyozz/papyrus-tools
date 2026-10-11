//! LSP positions, in lines and columns, and tree offsets.
//!
//! LSP counts columns in UTF-16 by default. A client accepting UTF-8 avoids
//! the conversion: it is the tree's unit.

use line_index::{LineCol, LineIndex, WideEncoding, WideLineCol};
use papyrus_syntax::{TextRange, TextSize};
use tower_lsp_server::ls_types::{Position, Range};

/// The column unit, negotiated at initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Columns {
    Utf8,
    #[default]
    Utf16,
}

/// Converts between offsets and positions in a text.
#[derive(Debug)]
pub(crate) struct LineMap {
    index: LineIndex,
    /// The end of each line, without its line break, `\r` included.
    ends: Box<[TextSize]>,
    columns: Columns,
}

impl LineMap {
    pub(crate) fn new(text: &str, columns: Columns) -> LineMap {
        let mut ends = Vec::new();
        let mut start = 0;
        for line in text.split('\n') {
            let content = line.strip_suffix('\r').unwrap_or(line);
            ends.push(TextSize::from((start + content.len()) as u32));
            start += line.len() + 1;
        }
        LineMap {
            index: LineIndex::new(text),
            ends: ends.into_boxed_slice(),
            columns,
        }
    }

    pub(crate) fn position(&self, offset: TextSize) -> Position {
        let offset = offset.min(self.index.len());
        let Some(utf8) = self.index.try_line_col(offset) else {
            return Position::new(self.line(offset), 0);
        };
        match self.columns {
            Columns::Utf8 => Position::new(utf8.line, utf8.col),
            Columns::Utf16 => {
                let wide = self
                    .index
                    .to_wide(WideEncoding::Utf16, utf8)
                    .unwrap_or(WideLineCol {
                        line: utf8.line,
                        col: utf8.col,
                    });
                Position::new(wide.line, wide.col)
            }
        }
    }

    pub(crate) fn range(&self, range: TextRange) -> Range {
        Range::new(self.position(range.start()), self.position(range.end()))
    }

    /// The offset of a position, clamped to the end of its line.
    pub(crate) fn offset(&self, position: Position) -> Option<TextSize> {
        let utf8 = match self.columns {
            Columns::Utf8 => LineCol {
                line: position.line,
                col: position.character,
            },
            Columns::Utf16 => self.index.to_utf8(
                WideEncoding::Utf16,
                WideLineCol {
                    line: position.line,
                    col: position.character,
                },
            )?,
        };
        let start = self.line_start(utf8.line)?;
        let end = self.ends[utf8.line as usize];
        Some((start + TextSize::from(utf8.col)).min(end))
    }

    pub(crate) fn text_range(&self, range: Range) -> Option<TextRange> {
        let start = self.offset(range.start)?;
        let end = self.offset(range.end)?;
        (start <= end).then(|| TextRange::new(start, end))
    }

    /// The (0-based) line of an offset.
    pub(crate) fn line(&self, offset: TextSize) -> u32 {
        let offset = offset.min(self.index.len());
        match self.index.try_line_col(offset) {
            Some(utf8) => utf8.line,
            // In the middle of a character: only the line matters.
            None => self.ends.partition_point(|&end| end < offset) as u32,
        }
    }

    /// The start of line `line`.
    pub(crate) fn line_start(&self, line: u32) -> Option<TextSize> {
        self.index.line(line).map(|r| r.start())
    }

    /// The end of line `line`, without its line break.
    pub(crate) fn line_end(&self, line: u32) -> Option<TextSize> {
        self.ends.get(line as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_colonnes_suivent_l_unite_negociee() {
        let text = "a = \"é𝄞\" x\nb";
        let x = TextSize::from(text.find(" x").unwrap() as u32 + 1);
        let utf8 = LineMap::new(text, Columns::Utf8);
        let utf16 = LineMap::new(text, Columns::Utf16);
        // `é`: 2 bytes, 1 UTF-16 unit; `𝄞`: 4 bytes, 2 units.
        assert_eq!(utf8.position(x), Position::new(0, 13));
        assert_eq!(utf16.position(x), Position::new(0, 10));
        assert_eq!(utf16.offset(Position::new(0, 10)), Some(x));
        assert_eq!(utf8.offset(Position::new(1, 0)), Some(TextSize::from(15)));
    }

    #[test]
    fn une_position_au_dela_de_la_ligne_est_ramenee() {
        let map = LineMap::new("ab\ncd", Columns::Utf8);
        assert_eq!(map.offset(Position::new(0, 99)), Some(TextSize::from(2)));
        assert_eq!(map.offset(Position::new(5, 0)), None);
        let map = LineMap::new("ab\r\ncd", Columns::Utf16);
        assert_eq!(map.offset(Position::new(0, 99)), Some(TextSize::from(2)));
        assert_eq!(map.line(TextSize::from(3)), 0);
        assert_eq!(map.line(TextSize::from(4)), 1);
        assert_eq!(map.line(TextSize::from(6)), 1);
    }
}
