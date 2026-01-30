use std::collections::{HashMap, HashSet};

use crate::table::Table;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CellRef {
    row: usize,
    col: usize,
}

#[derive(Debug)]
pub enum CalcError {
    CircularReference(String),
    InvalidReference(String),
    ParseError(String),
    EvalError(String),
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalcError::CircularReference(s) => write!(f, "Circular reference: {}", s),
            CalcError::InvalidReference(s) => write!(f, "Invalid reference: {}", s),
            CalcError::ParseError(s) => write!(f, "Parse error: {}", s),
            CalcError::EvalError(s) => write!(f, "Eval error: {}", s),
        }
    }
}

pub struct Calculator<'a> {
    table: &'a Table,
}

impl<'a> Calculator<'a> {
    pub fn new(table: &'a Table) -> Self {
        Self { table }
    }

    /// Evaluate all formula cells and return updates as (row, col, value)
    pub fn evaluate_all(&self) -> Result<Vec<(usize, usize, String)>, CalcError> {
        // Find all formula cells
        let mut formulas: HashMap<CellRef, String> = HashMap::new();
        for (row_idx, row) in self.table.cells.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                if cell.starts_with('=') {
                    formulas.insert(
                        CellRef { row: row_idx, col: col_idx },
                        cell[1..].to_string(),
                    );
                }
            }
        }

        if formulas.is_empty() {
            return Ok(vec![]);
        }

        // Build dependency graph
        let mut dependencies: HashMap<CellRef, HashSet<CellRef>> = HashMap::new();
        for (cell_ref, formula) in &formulas {
            let refs = self.extract_cell_refs(formula)?;
            dependencies.insert(cell_ref.clone(), refs);
        }

        // Check for circular references and get evaluation order
        let order = self.topological_sort(&formulas, &dependencies)?;

        // Evaluate in order
        let mut results: HashMap<CellRef, f64> = HashMap::new();
        let mut updates: Vec<(usize, usize, String)> = Vec::new();

        for cell_ref in order {
            let formula = &formulas[&cell_ref];
            let value = self.evaluate_formula(formula, &results)?;
            results.insert(cell_ref.clone(), value);

            // Format nicely: remove trailing zeros for integers
            let formatted = if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{}", value as i64)
            } else {
                format!("{}", value)
            };
            updates.push((cell_ref.row, cell_ref.col, formatted));
        }

        Ok(updates)
    }

    /// Parse column letters to 0-indexed column number (A=0, B=1, ..., Z=25, AA=26, etc.)
    fn col_from_letters(&self, letters: &str) -> usize {
        let mut result = 0usize;
        for c in letters.chars() {
            result = result * 26 + (c as usize - 'A' as usize + 1);
        }
        result - 1
    }

    /// Convert column index to letters for error messages
    fn col_to_letters(&self, mut col: usize) -> String {
        let mut result = String::new();
        loop {
            result.insert(0, (b'A' + (col % 26) as u8) as char);
            if col < 26 {
                break;
            }
            col = col / 26 - 1;
        }
        result
    }

    /// Parse a cell reference like "A1" or "AA123"
    fn parse_cell_ref(&self, s: &str) -> Option<CellRef> {
        let s = s.trim().to_uppercase();
        let mut col_end = 0;
        for (i, c) in s.chars().enumerate() {
            if c.is_ascii_alphabetic() {
                col_end = i + 1;
            } else {
                break;
            }
        }

        if col_end == 0 || col_end >= s.len() {
            return None;
        }

        let col_str = &s[..col_end];
        let row_str = &s[col_end..];

        let row: usize = row_str.parse().ok()?;
        if row == 0 {
            return None; // Rows are 1-indexed in user notation
        }

        let col = self.col_from_letters(col_str);
        Some(CellRef { row: row - 1, col }) // Convert to 0-indexed
    }

    /// Parse a range like "A1:A10" and return all cell refs
    fn parse_range(&self, s: &str) -> Result<Vec<CellRef>, CalcError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(CalcError::ParseError(format!("Invalid range: {}", s)));
        }

        let start = self.parse_cell_ref(parts[0])
            .ok_or_else(|| CalcError::InvalidReference(parts[0].to_string()))?;
        let end = self.parse_cell_ref(parts[1])
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

    /// Extract all cell references from a formula
    fn extract_cell_refs(&self, formula: &str) -> Result<HashSet<CellRef>, CalcError> {
        let mut refs = HashSet::new();
        let upper = formula.to_uppercase();

        // Find ranges first (e.g., A1:B10)
        let range_re = regex_lite(r"[A-Z]+[0-9]+:[A-Z]+[0-9]+");
        for cap in find_all_matches(&upper, &range_re) {
            for cell_ref in self.parse_range(&cap)? {
                refs.insert(cell_ref);
            }
        }

        // Find single cell refs (not part of ranges)
        let cell_re = regex_lite(r"[A-Z]+[0-9]+");
        for cap in find_all_matches(&upper, &cell_re) {
            // Skip if this is part of a range (contains :)
            if let Some(cell_ref) = self.parse_cell_ref(&cap) {
                refs.insert(cell_ref);
            }
        }

        Ok(refs)
    }

    /// Topological sort with cycle detection
    fn topological_sort(
        &self,
        formulas: &HashMap<CellRef, String>,
        dependencies: &HashMap<CellRef, HashSet<CellRef>>,
    ) -> Result<Vec<CellRef>, CalcError> {
        let mut visited: HashSet<CellRef> = HashSet::new();
        let mut in_stack: HashSet<CellRef> = HashSet::new();
        let mut order: Vec<CellRef> = Vec::new();

        for cell_ref in formulas.keys() {
            if !visited.contains(cell_ref) {
                self.dfs_topo(
                    cell_ref,
                    formulas,
                    dependencies,
                    &mut visited,
                    &mut in_stack,
                    &mut order,
                )?;
            }
        }

        Ok(order)
    }

    fn dfs_topo(
        &self,
        cell: &CellRef,
        formulas: &HashMap<CellRef, String>,
        dependencies: &HashMap<CellRef, HashSet<CellRef>>,
        visited: &mut HashSet<CellRef>,
        in_stack: &mut HashSet<CellRef>,
        order: &mut Vec<CellRef>,
    ) -> Result<(), CalcError> {
        if in_stack.contains(cell) {
            let cell_name = format!("{}{}", self.col_to_letters(cell.col), cell.row + 1);
            return Err(CalcError::CircularReference(cell_name));
        }

        if visited.contains(cell) {
            return Ok(());
        }

        in_stack.insert(cell.clone());
        visited.insert(cell.clone());

        // Only follow dependencies that are also formulas
        if let Some(deps) = dependencies.get(cell) {
            for dep in deps {
                if formulas.contains_key(dep) {
                    self.dfs_topo(dep, formulas, dependencies, visited, in_stack, order)?;
                }
            }
        }

        in_stack.remove(cell);
        order.push(cell.clone());

        Ok(())
    }

    /// Get cell value as f64
    fn get_cell_value(&self, cell: &CellRef, results: &HashMap<CellRef, f64>) -> Result<f64, CalcError> {
        // Check if we already computed this cell
        if let Some(&val) = results.get(cell) {
            return Ok(val);
        }

        // Get from table
        let cell_content = self.table.get_cell(cell.row, cell.col)
            .ok_or_else(|| {
                let name = format!("{}{}", self.col_to_letters(cell.col), cell.row + 1);
                CalcError::InvalidReference(name)
            })?;

        // Empty cell = 0
        if cell_content.trim().is_empty() {
            return Ok(0.0);
        }

        // Try to parse as number
        cell_content.trim().parse::<f64>()
            .map_err(|_| {
                let name = format!("{}{}", self.col_to_letters(cell.col), cell.row + 1);
                CalcError::EvalError(format!("{} is not a number", name))
            })
    }

    /// Get values for a range
    fn get_range_values(&self, range: &str, results: &HashMap<CellRef, f64>) -> Result<Vec<f64>, CalcError> {
        let refs = self.parse_range(range)?;
        let mut values = Vec::new();
        for cell_ref in refs {
            values.push(self.get_cell_value(&cell_ref, results)?);
        }
        Ok(values)
    }

    /// Evaluate a formula
    fn evaluate_formula(&self, formula: &str, results: &HashMap<CellRef, f64>) -> Result<f64, CalcError> {
        let mut expr = formula.to_string();

        // Handle functions first
        expr = self.expand_functions(&expr, results)?;

        // Replace cell references with their values
        expr = self.substitute_cell_refs(&expr, results)?;

        // Evaluate the expression
        let result = evalexpr::eval(&expr)
            .map_err(|e| CalcError::EvalError(e.to_string()))?;

        // Try to get as float first, then as int
        if let Ok(f) = result.as_float() {
            Ok(f)
        } else if let Ok(i) = result.as_int() {
            Ok(i as f64)
        } else {
            Err(CalcError::EvalError("Result is not a number".to_string()))
        }
    }

    /// Expand function calls like sum(A1:A10) to their values
    fn expand_functions(&self, formula: &str, results: &HashMap<CellRef, f64>) -> Result<String, CalcError> {
        let mut result = formula.to_string();

        // Handle SUM
        while let Some(start) = upper_find(&result, "SUM(") {
            let end = find_matching_paren(&result, start + 4)?;
            let range = &result[start + 4..end];
            let values = self.get_range_values(range, results)?;
            let sum: f64 = values.iter().sum();
            result = format!("{}{}{}", &result[..start], sum, &result[end + 1..]);
        }

        // Handle AVG
        while let Some(start) = upper_find(&result, "AVG(") {
            let end = find_matching_paren(&result, start + 4)?;
            let range = &result[start + 4..end];
            let values = self.get_range_values(range, results)?;
            let avg = if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 };
            result = format!("{}{}{}", &result[..start], avg, &result[end + 1..]);
        }

        // Handle MIN
        while let Some(start) = upper_find(&result, "MIN(") {
            let end = find_matching_paren(&result, start + 4)?;
            let range = &result[start + 4..end];
            let values = self.get_range_values(range, results)?;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            result = format!("{}{}{}", &result[..start], min, &result[end + 1..]);
        }

        // Handle MAX
        while let Some(start) = upper_find(&result, "MAX(") {
            let end = find_matching_paren(&result, start + 4)?;
            let range = &result[start + 4..end];
            let values = self.get_range_values(range, results)?;
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            result = format!("{}{}{}", &result[..start], max, &result[end + 1..]);
        }

        // Handle COUNT
        while let Some(start) = upper_find(&result, "COUNT(") {
            let end = find_matching_paren(&result, start + 6)?;
            let range = &result[start + 6..end];
            let values = self.get_range_values(range, results)?;
            result = format!("{}{}{}", &result[..start], values.len(), &result[end + 1..]);
        }

        Ok(result)
    }

    /// Substitute cell references with their values
    fn substitute_cell_refs(&self, formula: &str, results: &HashMap<CellRef, f64>) -> Result<String, CalcError> {
        let mut result = formula.to_string();

        // Find and replace cell references (longest first to handle AA1 before A1)
        let cell_re = regex_lite(r"[A-Za-z]+[0-9]+");
        let mut matches: Vec<(usize, usize, String)> = Vec::new();

        for m in find_all_matches_with_pos(&result, &cell_re) {
            matches.push(m);
        }

        // Sort by position descending to replace from end to start
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        for (start, end, cell_str) in matches {
            if let Some(cell_ref) = self.parse_cell_ref(&cell_str) {
                let value = self.get_cell_value(&cell_ref, results)?;
                result = format!("{}{}{}", &result[..start], value, &result[end..]);
            }
        }

        Ok(result)
    }
}

// Simple regex-like matching without regex crate
fn regex_lite(pattern: &str) -> String {
    pattern.to_string()
}

fn find_all_matches(text: &str, _pattern: &str) -> Vec<String> {
    let mut matches = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Try to match [A-Z]+[0-9]+ pattern possibly with :
        if chars[i].is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            if i < chars.len() && chars[i].is_ascii_digit() {
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                // Check for range
                if i < chars.len() && chars[i] == ':' {
                    let colon = i;
                    i += 1;
                    if i < chars.len() && chars[i].is_ascii_alphabetic() {
                        while i < chars.len() && chars[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                        if i < chars.len() && chars[i].is_ascii_digit() {
                            while i < chars.len() && chars[i].is_ascii_digit() {
                                i += 1;
                            }
                            matches.push(chars[start..i].iter().collect());
                            continue;
                        }
                    }
                    i = colon; // Reset to just after first cell ref
                }
                matches.push(chars[start..i].iter().collect());
                continue;
            }
            // Not a valid cell ref, continue from where we stopped
        }
        i += 1;
    }

    matches
}

fn find_all_matches_with_pos(text: &str, _pattern: &str) -> Vec<(usize, usize, String)> {
    let mut matches = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut byte_pos = 0;

    while i < chars.len() {
        if chars[i].is_ascii_alphabetic() && chars[i].is_ascii_uppercase() {
            let start_i = i;
            let start_byte = byte_pos;

            while i < chars.len() && chars[i].is_ascii_alphabetic() && chars[i].is_ascii_uppercase() {
                byte_pos += chars[i].len_utf8();
                i += 1;
            }

            if i < chars.len() && chars[i].is_ascii_digit() {
                while i < chars.len() && chars[i].is_ascii_digit() {
                    byte_pos += chars[i].len_utf8();
                    i += 1;
                }
                // Skip ranges - they should be expanded already
                if i < chars.len() && chars[i] == ':' {
                    // It's a range, skip the whole thing
                    continue;
                }
                matches.push((start_byte, byte_pos, chars[start_i..i].iter().collect()));
                continue;
            }
        }

        byte_pos += chars[i].len_utf8();
        i += 1;
    }

    matches
}

fn upper_find(text: &str, pattern: &str) -> Option<usize> {
    text.to_uppercase().find(pattern)
}

fn find_matching_paren(text: &str, start: usize) -> Result<usize, CalcError> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth = 1;
    let mut i = start;

    while i < chars.len() && depth > 0 {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            i += 1;
        }
    }

    if depth != 0 {
        Err(CalcError::ParseError("Unmatched parenthesis".to_string()))
    } else {
        Ok(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_from_letters() {
        let table = Table::new();
        let calc = Calculator::new(&table);
        assert_eq!(calc.col_from_letters("A"), 0);
        assert_eq!(calc.col_from_letters("B"), 1);
        assert_eq!(calc.col_from_letters("Z"), 25);
        assert_eq!(calc.col_from_letters("AA"), 26);
        assert_eq!(calc.col_from_letters("AB"), 27);
        assert_eq!(calc.col_from_letters("AZ"), 51);
        assert_eq!(calc.col_from_letters("BA"), 52);
    }

    #[test]
    fn test_parse_cell_ref() {
        let table = Table::new();
        let calc = Calculator::new(&table);

        let r = calc.parse_cell_ref("A1").unwrap();
        assert_eq!(r.row, 0);
        assert_eq!(r.col, 0);

        let r = calc.parse_cell_ref("B2").unwrap();
        assert_eq!(r.row, 1);
        assert_eq!(r.col, 1);

        let r = calc.parse_cell_ref("AA10").unwrap();
        assert_eq!(r.row, 9);
        assert_eq!(r.col, 26);
    }

    #[test]
    fn test_parse_range() {
        let table = Table::new();
        let calc = Calculator::new(&table);

        let refs = calc.parse_range("A1:A3").unwrap();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], CellRef { row: 0, col: 0 });
        assert_eq!(refs[1], CellRef { row: 1, col: 0 });
        assert_eq!(refs[2], CellRef { row: 2, col: 0 });
    }
}
