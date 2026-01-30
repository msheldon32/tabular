use regex::Regex;

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
pub fn parse_range(s: &str) -> Result<Vec<CellRef>, CalcError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_from_letters() {
        assert_eq!(col_from_letters("A"), 0);
        assert_eq!(col_from_letters("B"), 1);
        assert_eq!(col_from_letters("Z"), 25);
        assert_eq!(col_from_letters("AA"), 26);
        assert_eq!(col_from_letters("AB"), 27);
        assert_eq!(col_from_letters("AZ"), 51);
        assert_eq!(col_from_letters("BA"), 52);
    }

    #[test]
    fn test_parse_cell_ref() {
        let r = parse_cell_ref("A1").unwrap();
        assert_eq!(r.row, 0);
        assert_eq!(r.col, 0);

        let r = parse_cell_ref("B2").unwrap();
        assert_eq!(r.row, 1);
        assert_eq!(r.col, 1);

        let r = parse_cell_ref("AA10").unwrap();
        assert_eq!(r.row, 9);
        assert_eq!(r.col, 26);
    }

    #[test]
    fn test_parse_range() {
        let refs = parse_range("A1:A3").unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], CellRef { row: 0, col: 0 });
        assert_eq!(refs[1], CellRef { row: 1, col: 0 });
        assert_eq!(refs[2], CellRef { row: 2, col: 0 });
    }
}
