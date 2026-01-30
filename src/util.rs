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


/// Parse column letters to 0-indexed column number (A=0, B=1, ..., Z=25, AA=26, etc.)
pub fn col_from_letters(letters: &str) -> usize {
    let mut result = 0usize;
    for c in letters.chars() {
        result = result * 26 + (c as usize - 'A' as usize + 1);
    }
    result - 1
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
