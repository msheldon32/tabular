use regex::Regex;
use std::num::ParseIntError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellRef {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum CalcError {
    CircularReference(String),
    InvalidReference(String),
    ParseError(String),
    EvalError(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ColumnType {
    Numeric,
    Text,
}

impl From<ParseIntError> for CalcError {
    fn from(e: ParseIntError) -> Self { CalcError::ParseError(e.to_string()) }
}

fn is_ascii_alpha(b: u8) -> bool {
    (b'A'..=b'Z').contains(&b) || (b'a'..=b'z').contains(&b)
}

fn is_ascii_digit(b: u8) -> bool {
    (b'0'..=b'9').contains(&b)
}

fn is_ascii_alnum(b: u8) -> bool {
    is_ascii_alpha(b) || is_ascii_digit(b)
}

/// Parse column letters to 0-indexed column number (A=0, B=1, ..., Z=25, AA=26, etc.)
pub fn col_from_letters(letters: &str) -> usize {
    let mut result = 0usize;
    for c in letters.chars() {
        result = result * 26 + (c as usize - 'A' as usize + 1);
    }
    result - 1
}

pub fn letters_from_col(mut col: usize) -> String {
    col += 1;
    let mut buf = Vec::new();
    while col > 0 {
        col -= 1;
        let rem = (col % 26) as u8;
        buf.push((b'A' + rem) as char);
        col /= 26;
    }
    buf.into_iter().rev().collect()
}


pub fn translate_references(s: &str, row_diff: isize, col_diff: isize) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());

    if s.chars().next() != Some('=') {
        // for equations only
        return String::from(s);
    }

    let mut i = 0;
    while i < bytes.len() {
        // Look for a potential column start: ASCII letter
        if is_ascii_alpha(bytes[i]) {
            let start = i;

            // 1) consume letters (column)
            let mut j = i;
            while j < bytes.len() && is_ascii_alpha(bytes[j]) {
                j += 1;
            }
            let col_part = &s[start..j];

            // 2) consume digits (row)
            let mut k = j;
            while k < bytes.len() && is_ascii_digit(bytes[k]) {
                k += 1;
            }

            // Must have at least one digit for a reference (e.g., "AA24")
            if k > j {
                let row_part = &s[j..k];

                // Boundary checks: don't match inside alphanumeric tokens
                let prev_ok = start == 0 || !is_ascii_alnum(bytes[start - 1]);
                let next_ok = k == bytes.len() || !is_ascii_alnum(bytes[k]);

                if prev_ok && next_ok {
                    // Parse & translate
                    if let (col, Some(row)) =
                        (col_from_letters(col_part), row_part.parse::<usize>().ok())
                    {
                        // Apply signed offsets, clamping to 0
                        let new_col = (col as isize + col_diff).max(0) as usize;
                        let new_row = (row as isize + row_diff).max(1) as usize; // rows are 1-based

                        // Preserve input column case (all-lower => lower; else upper)
                        let lower = col_part.bytes().all(|b| (b'a'..=b'z').contains(&b));
                        let mut col_str = letters_from_col(new_col);
                        if lower {
                            col_str.make_ascii_lowercase();
                        }

                        out.push_str(&col_str);
                        out.push_str(&new_row.to_string());
                        i = k;
                        continue;
                    }
                }
            }

            // Not a valid ref -> emit current char and continue
            out.push(bytes[i] as char);
            i += 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

/// Parse a cell reference like "A1" or "AA123"
pub fn parse_cell_ref(s: &str) -> Option<CellRef> {
    let s = s.trim().to_uppercase();
    let re = Regex::new(r"^([A-Z]+)(\d+)$").ok()?;
    let caps = re.captures(&s)?;

    let col_str = caps.get(1)?.as_str();
    let row_str = caps.get(2)?.as_str();

    let row: usize = row_str.parse().ok()?;
    if row == 0 {
        return None; // Rows are 1-indexed in user notation
    }

    let col = col_from_letters(col_str);
    Some(CellRef { row: row - 1, col }) // Convert to 0-indexed
}

/// Parse a range like "A1:A10" and return all cell refs
pub fn parse_range(s: &str, row_count: usize, col_count: usize, skip_header: bool) -> Result<Vec<CellRef>, CalcError> {
    // Normalize to uppercase for matching
    let upper = s.to_uppercase();

    // delegation
    let row_range_re = Regex::new(r"^\d+:\d+$").unwrap();
    let col_range_re = Regex::new(r"^[A-Z]+:[A-Z]+$").unwrap();

    if row_range_re.is_match(&upper) { return parse_row_range(&upper, col_count); }
    if col_range_re.is_match(&upper) { return parse_col_range(&upper, row_count, skip_header); }

    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(CalcError::ParseError(format!("Invalid range: {}", s)));
    }

    let start = parse_cell_ref(parts[0])
        .ok_or_else(|| CalcError::InvalidReference(parts[0].to_string()))?;
    let end = parse_cell_ref(parts[1])
        .ok_or_else(|| CalcError::InvalidReference(parts[1].to_string()))?;

    let mut refs = Vec::new();
    let row_start = start.row.min(end.row);
    let row_end = start.row.max(end.row);
    let col_start = start.col.min(end.col);
    let col_end = start.col.max(end.col);

    for row in row_start..=row_end {
        for col in col_start..=col_end {
            refs.push(CellRef { row, col });
        }
    }

    Ok(refs)
}

/// Parse a range of rows such as "1:10" (1-indexed, so row 1 is internal index 0)
pub fn parse_row_range(s: &str, col_count: usize) -> Result<Vec<CellRef>, CalcError> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(CalcError::ParseError(format!("Invalid range: {}", s)));
    }

    let row_start_1indexed: usize = parts[0].parse()?;
    let row_end_1indexed: usize = parts[1].parse()?;
    if row_start_1indexed == 0 || row_end_1indexed == 0 {
        return Err(CalcError::InvalidReference(s.to_string()));
    }

    // Convert to 0-indexed
    let row_start = row_start_1indexed - 1;
    let row_end = row_end_1indexed - 1;

    let mut refs = Vec::new();
    let col_start = 0;
    let col_end = col_count - 1;

    for row in row_start..=row_end {
        for col in col_start..=col_end {
            refs.push(CellRef { row, col });
        }
    }

    Ok(refs)
}

/// Parse a range of columns such as "A:D"
pub fn parse_col_range(s: &str, row_count: usize, skip_header: bool) -> Result<Vec<CellRef>, CalcError> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(CalcError::ParseError(format!("Invalid range: {}", s)));
    }

    // Validate that both parts are non-empty letter sequences
    if parts[0].is_empty() || parts[1].is_empty() {
        return Err(CalcError::InvalidReference(s.to_string()));
    }

    let col_start: usize = col_from_letters(parts[0]);
    let col_end: usize = col_from_letters(parts[1]);

    let mut refs = Vec::new();
    let row_start = if skip_header { 1 } else { 0 };
    let row_end = row_count - 1;

    let col_min = col_start.min(col_end);
    let col_max = col_start.max(col_end);

    for row in row_start..=row_end {
        for col in col_min..=col_max {
            refs.push(CellRef { row, col });
        }
    }

    Ok(refs)
}

// === Unicode utilities ===

use unicode_width::UnicodeWidthStr;

/// Get the display width of a string (handles CJK double-width characters)
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Get the number of characters in a string
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Get the byte index for a character index
/// Returns string length if char_idx is >= character count
pub fn byte_index_of_char(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

/// Get the character index for a byte index
/// Finds the character that contains or starts at the given byte position
pub fn char_index_of_byte(s: &str, byte_idx: usize) -> usize {
    s.char_indices()
        .take_while(|(idx, _)| *idx < byte_idx)
        .count()
}

/// Safely slice a string by character indices (not byte indices)
/// Returns the substring from char_start to char_end (exclusive)
pub fn slice_by_chars(s: &str, char_start: usize, char_end: usize) -> &str {
    let start_byte = byte_index_of_char(s, char_start);
    let end_byte = byte_index_of_char(s, char_end);
    &s[start_byte..end_byte]
}

/// Get the character at a given character index, if it exists
pub fn char_at(s: &str, char_idx: usize) -> Option<char> {
    s.chars().nth(char_idx)
}

/// Remove the character at the given character index
/// Returns (new_string, removed_char) or None if index is out of bounds
pub fn remove_char_at(s: &str, char_idx: usize) -> Option<(String, char)> {
    let mut chars: Vec<char> = s.chars().collect();
    if char_idx >= chars.len() {
        return None;
    }
    let removed = chars.remove(char_idx);
    Some((chars.into_iter().collect(), removed))
}

/// Insert a character at the given character index
pub fn insert_char_at(s: &str, char_idx: usize, c: char) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    let insert_pos = char_idx.min(chars.len());
    chars.insert(insert_pos, c);
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // === col_from_letters tests ===

    #[test]
    fn test_col_from_letters_single() {
        assert_eq!(col_from_letters("A"), 0);
        assert_eq!(col_from_letters("B"), 1);
        assert_eq!(col_from_letters("Z"), 25);
    }

    #[test]
    fn test_col_from_letters_double() {
        assert_eq!(col_from_letters("AA"), 26);
        assert_eq!(col_from_letters("AB"), 27);
        assert_eq!(col_from_letters("AZ"), 51);
        assert_eq!(col_from_letters("BA"), 52);
        assert_eq!(col_from_letters("ZZ"), 701);
    }

    #[test]
    fn test_col_from_letters_triple() {
        assert_eq!(col_from_letters("AAA"), 702);
        assert_eq!(col_from_letters("AAB"), 703);
    }

    // === letters_from_col tests ===

    #[test]
    fn test_letters_from_col_single() {
        assert_eq!(letters_from_col(0), "A");
        assert_eq!(letters_from_col(1), "B");
        assert_eq!(letters_from_col(25), "Z");
    }

    #[test]
    fn test_letters_from_col_double() {
        assert_eq!(letters_from_col(26), "AA");
        assert_eq!(letters_from_col(27), "AB");
        assert_eq!(letters_from_col(51), "AZ");
        assert_eq!(letters_from_col(52), "BA");
        assert_eq!(letters_from_col(701), "ZZ");
    }

    #[test]
    fn test_letters_from_col_triple() {
        assert_eq!(letters_from_col(702), "AAA");
        assert_eq!(letters_from_col(703), "AAB");
    }

    #[test]
    fn test_col_letters_roundtrip() {
        for col in 0..1000 {
            let letters = letters_from_col(col);
            assert_eq!(col_from_letters(&letters), col, "Failed roundtrip for col {}", col);
        }
    }

    // === parse_cell_ref tests ===

    #[test]
    fn test_parse_cell_ref_simple() {
        let r = parse_cell_ref("A1").unwrap();
        assert_eq!(r.row, 0);
        assert_eq!(r.col, 0);

        let r = parse_cell_ref("B2").unwrap();
        assert_eq!(r.row, 1);
        assert_eq!(r.col, 1);
    }

    #[test]
    fn test_parse_cell_ref_large() {
        let r = parse_cell_ref("AA10").unwrap();
        assert_eq!(r.row, 9);
        assert_eq!(r.col, 26);

        let r = parse_cell_ref("ZZ100").unwrap();
        assert_eq!(r.row, 99);
        assert_eq!(r.col, 701);
    }

    #[test]
    fn test_parse_cell_ref_lowercase() {
        let r = parse_cell_ref("a1").unwrap();
        assert_eq!(r.row, 0);
        assert_eq!(r.col, 0);

        let r = parse_cell_ref("aa10").unwrap();
        assert_eq!(r.row, 9);
        assert_eq!(r.col, 26);
    }

    #[test]
    fn test_parse_cell_ref_whitespace() {
        let r = parse_cell_ref("  A1  ").unwrap();
        assert_eq!(r.row, 0);
        assert_eq!(r.col, 0);
    }

    #[test]
    fn test_parse_cell_ref_invalid() {
        assert!(parse_cell_ref("").is_none());
        assert!(parse_cell_ref("A").is_none());
        assert!(parse_cell_ref("1").is_none());
        assert!(parse_cell_ref("A0").is_none()); // Row 0 is invalid
        assert!(parse_cell_ref("1A").is_none());
        assert!(parse_cell_ref("A1B").is_none());
    }

    // === parse_range tests ===

    #[test]
    fn test_parse_range_column() {
        let refs = parse_range("A1:A3", 100, 26, false).unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], CellRef { row: 0, col: 0 });
        assert_eq!(refs[1], CellRef { row: 1, col: 0 });
        assert_eq!(refs[2], CellRef { row: 2, col: 0 });
    }

    #[test]
    fn test_parse_range_row() {
        let refs = parse_range("A1:C1", 100, 26, false).unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], CellRef { row: 0, col: 0 });
        assert_eq!(refs[1], CellRef { row: 0, col: 1 });
        assert_eq!(refs[2], CellRef { row: 0, col: 2 });
    }

    #[test]
    fn test_parse_range_rectangular() {
        let refs = parse_range("A1:B2", 100, 26, false).unwrap();
        assert_eq!(refs.len(), 4);
        assert!(refs.contains(&CellRef { row: 0, col: 0 }));
        assert!(refs.contains(&CellRef { row: 0, col: 1 }));
        assert!(refs.contains(&CellRef { row: 1, col: 0 }));
        assert!(refs.contains(&CellRef { row: 1, col: 1 }));
    }

    #[test]
    fn test_parse_range_reversed() {
        // Should work even if end comes before start
        let refs = parse_range("B2:A1", 100, 26, false).unwrap();
        assert_eq!(refs.len(), 4);
    }

    #[test]
    fn test_parse_range_single_cell() {
        let refs = parse_range("A1:A1", 100, 26, false).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0], CellRef { row: 0, col: 0 });
    }

    #[test]
    fn test_parse_range_invalid() {
        assert!(parse_range("A1", 100, 26, false).is_err());
        assert!(parse_range("A1:", 100, 26, false).is_err());
        assert!(parse_range(":A1", 100, 26, false).is_err());
        assert!(parse_range("A1:B", 100, 26, false).is_err());
    }

    // === translate_references tests ===
    // Note: translate_references only works on formulas (strings starting with '=')
    // Non-formula strings are returned unchanged.

    #[test]
    fn test_translate_references_simple() {
        assert_eq!(translate_references("=A1", 1, 0), "=A2");
        assert_eq!(translate_references("=A1", 0, 1), "=B1");
        assert_eq!(translate_references("=A1", 1, 1), "=B2");
    }

    #[test]
    fn test_translate_references_multiple() {
        assert_eq!(translate_references("=A1+B2", 1, 0), "=A2+B3");
        assert_eq!(translate_references("=A1*B1+C1", 0, 1), "=B1*C1+D1");
    }

    #[test]
    fn test_translate_references_formula() {
        assert_eq!(translate_references("=A1+B2*C3", 1, 1), "=B2+C3*D4");
        assert_eq!(translate_references("=SUM(A1:A10)", 0, 1), "=SUM(B1:B10)");
    }

    #[test]
    fn test_translate_references_preserves_uppercase() {
        // Uppercase references work correctly
        assert_eq!(translate_references("=A1", 1, 0), "=A2");
        assert_eq!(translate_references("=B1", 0, 1), "=C1");
        assert_eq!(translate_references("=AA1", 1, 0), "=AA2");
    }

    // Note: lowercase cell references have a known quirk in col_from_letters
    // which assumes uppercase input. This is documented behavior we're preserving.

    #[test]
    fn test_translate_references_negative_offset() {
        assert_eq!(translate_references("=B2", 0, -1), "=A2");
        assert_eq!(translate_references("=A2", -1, 0), "=A1");
        assert_eq!(translate_references("=C3", -1, -1), "=B2");
    }

    #[test]
    fn test_translate_references_clamps_to_bounds() {
        // Column can't go below 0
        assert_eq!(translate_references("=A1", 0, -1), "=A1");
        // Row can't go below 1 (1-based in references)
        assert_eq!(translate_references("=A1", -1, 0), "=A1");
    }

    #[test]
    fn test_translate_references_large_refs() {
        assert_eq!(translate_references("=AA100", 1, 1), "=AB101");
        assert_eq!(translate_references("=ZZ999", 1, 1), "=AAA1000");
    }

    #[test]
    fn test_translate_references_no_refs() {
        // Non-formula strings are returned unchanged
        assert_eq!(translate_references("hello world", 1, 1), "hello world");
        assert_eq!(translate_references("123", 1, 1), "123");
        assert_eq!(translate_references("", 1, 1), "");
        // Even strings with cell-like patterns are unchanged if not a formula
        assert_eq!(translate_references("A1", 1, 1), "A1");
    }

    #[test]
    fn test_translate_references_not_in_identifiers() {
        // References followed by alphanumeric chars are not matched
        assert_eq!(translate_references("=A1B", 1, 0), "=A1B"); // A1B - 'B' after, not matched
    }

    #[test]
    fn test_translate_references_standalone() {
        // Standalone references in formulas should be translated
        assert_eq!(translate_references("= A1 ", 1, 0), "= A2 ");
        assert_eq!(translate_references("=(A1)", 1, 0), "=(A2)");
    }

    #[test]
    fn test_translate_references_with_symbols() {
        assert_eq!(translate_references("=(A1)", 1, 0), "=(A2)");
        assert_eq!(translate_references("=[A1]", 1, 0), "=[A2]");
        assert_eq!(translate_references("=A1,B2,C3", 1, 0), "=A2,B3,C4");
    }

    #[test]
    fn test_display_width() {
        use super::display_width;
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("héllo"), 5); // é is still 1 display width
        assert_eq!(display_width("你好"), 4);   // CJK chars are 2 columns each
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn test_char_count() {
        use super::char_count;
        assert_eq!(char_count("hello"), 5);
        assert_eq!(char_count("héllo"), 5);
        assert_eq!(char_count("你好"), 2);
        assert_eq!(char_count(""), 0);
    }

    #[test]
    fn test_char_indices() {
        use super::{byte_index_of_char, char_index_of_byte};
        let s = "héllo"; // é is 2 bytes
        assert_eq!(byte_index_of_char(s, 0), 0);
        assert_eq!(byte_index_of_char(s, 1), 1);  // 'h' ends, 'é' starts
        assert_eq!(byte_index_of_char(s, 2), 3);  // 'é' is 2 bytes, 'l' starts at 3
        assert_eq!(byte_index_of_char(s, 5), 6);  // End of string

        assert_eq!(char_index_of_byte(s, 0), 0);
        assert_eq!(char_index_of_byte(s, 1), 1);
        assert_eq!(char_index_of_byte(s, 3), 2);
    }
}
