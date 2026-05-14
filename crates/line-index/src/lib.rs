//! Small source line index used by viewer/editor bridge code.
//!
//! The index stores byte offsets for the start of each 1-based source line.
//! It handles LF, CRLF, and bare CR newlines without normalizing the original
//! text, so callers can map parser source positions back into editor offsets
//! while preserving write-back bytes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    source_len: usize,
    line_starts: Vec<usize>,
    line_ends: Vec<usize>,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut line_starts = vec![0];
        let mut line_ends = Vec::new();
        let mut index = 0;

        while index < bytes.len() {
            match bytes[index] {
                b'\r' => {
                    line_ends.push(index);
                    index += 1;
                    if bytes.get(index) == Some(&b'\n') {
                        index += 1;
                    }
                    line_starts.push(index);
                }
                b'\n' => {
                    line_ends.push(index);
                    index += 1;
                    line_starts.push(index);
                }
                _ => {
                    index += 1;
                }
            }
        }
        line_ends.push(source.len());

        Self {
            source_len: source.len(),
            line_starts,
            line_ends,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line_start(&self, line: usize) -> Option<usize> {
        line.checked_sub(1)
            .and_then(|index| self.line_starts.get(index).copied())
    }

    pub fn line_end(&self, line: usize) -> Option<usize> {
        line.checked_sub(1)
            .and_then(|index| self.line_ends.get(index).copied())
    }

    pub fn line_range(&self, line: usize) -> Option<std::ops::Range<usize>> {
        Some(self.line_start(line)?..self.line_end(line)?)
    }

    pub fn offset_to_line_column(&self, source: &str, offset: usize) -> Option<LineColumn> {
        if source.len() != self.source_len
            || offset > self.source_len
            || !source.is_char_boundary(offset)
        {
            return None;
        }

        let line_index = self.line_starts.partition_point(|start| *start <= offset);
        let line_index = line_index.saturating_sub(1);
        let line_start = self.line_starts[line_index];
        if !source.is_char_boundary(line_start) {
            return None;
        }
        let column = source[line_start..offset].chars().count() + 1;

        Some(LineColumn {
            line: line_index + 1,
            column,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{LineColumn, LineIndex};

    #[test]
    fn maps_mixed_newline_line_starts() {
        let source = "one\r\ntwo\nthree\rfour";
        let index = LineIndex::new(source);

        assert_eq!(index.line_count(), 4);
        assert_eq!(index.line_start(1), Some(0));
        assert_eq!(index.line_start(2), Some(5));
        assert_eq!(index.line_start(3), Some(9));
        assert_eq!(index.line_start(4), Some(15));
        assert_eq!(index.line_start(5), None);
    }

    #[test]
    fn returns_line_ranges_without_newlines() {
        let source = "one\r\ntwo\n";
        let index = LineIndex::new(source);

        assert_eq!(index.line_range(1), Some(0..3));
        assert_eq!(index.line_range(2), Some(5..8));
        assert_eq!(index.line_range(3), Some(9..9));
    }

    #[test]
    fn maps_offsets_to_one_based_line_columns() {
        let source = "a\nbc\n";
        let index = LineIndex::new(source);

        assert_eq!(
            index.offset_to_line_column(source, 0),
            Some(LineColumn { line: 1, column: 1 })
        );
        assert_eq!(
            index.offset_to_line_column(source, 3),
            Some(LineColumn { line: 2, column: 2 })
        );
        assert_eq!(
            index.offset_to_line_column(source, source.len()),
            Some(LineColumn { line: 3, column: 1 })
        );
    }

    #[test]
    fn rejects_offsets_that_are_not_utf8_boundaries() {
        let source = "a\n\u{00E9}";
        let index = LineIndex::new(source);

        assert_eq!(index.offset_to_line_column(source, 3), None);
    }
}
