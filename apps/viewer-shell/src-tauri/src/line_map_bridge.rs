use line_index::LineIndex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LineOffset {
    pub line: usize,
    pub offset: usize,
}

#[tauri::command]
pub fn line_to_offset(markdown: String, line: usize) -> Result<LineOffset, String> {
    let index = LineIndex::new(&markdown);
    let offset = index
        .line_start(line)
        .ok_or_else(|| format!("line {line} is outside the document"))?;
    Ok(LineOffset { line, offset })
}

#[cfg(test)]
mod tests {
    use super::line_to_offset;

    #[test]
    fn maps_one_based_line_to_byte_offset() {
        let mapped = line_to_offset("one\r\ntwo\nthree".to_string(), 3)
            .expect("line should map");

        assert_eq!(mapped.line, 3);
        assert_eq!(mapped.offset, 9);
    }

    #[test]
    fn rejects_missing_line() {
        let error = line_to_offset("one".to_string(), 2).expect_err("line should be missing");

        assert!(error.contains("outside the document"));
    }
}
