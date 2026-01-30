use std::collections::{HashMap, HashSet};
use regex::Regex;

use crate::table::Table;
use crate::util::{CellRef, parse_cell_ref, parse_range, CalcError};

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

    /// Extract all cell references from a formula
    fn extract_cell_refs(&self, formula: &str) -> Result<HashSet<CellRef>, CalcError> {
        let mut refs = HashSet::new();
        let upper = formula.to_uppercase();

        // Find ranges first (e.g., A1:B10)
        let range_re = Regex::new(r"[A-Z]+\d+:[A-Z]+\d+").unwrap();
        for cap in range_re.find_iter(&upper) {
            for cell_ref in parse_range(cap.as_str())? {
                refs.insert(cell_ref);
            }
        }

        // Find single cell refs
        let cell_re = Regex::new(r"[A-Z]+\d+").unwrap();
        for cap in cell_re.find_iter(&upper) {
            if let Some(cell_ref) = parse_cell_ref(cap.as_str()) {
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
        let refs = parse_range(range)?;
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
        let sum_re = Regex::new(r"(?i)SUM\(([^)]+)\)").unwrap();
        while let Some(caps) = sum_re.captures(&result) {
            let full_match = caps.get(0).unwrap();
            let range = caps.get(1).unwrap().as_str();
            let values = self.get_range_values(range, results)?;
            let sum: f64 = values.iter().sum();
            result = format!(
                "{}{}{}",
                &result[..full_match.start()],
                sum,
                &result[full_match.end()..]
            );
        }

        // Handle AVG
        let avg_re = Regex::new(r"(?i)AVG\(([^)]+)\)").unwrap();
        while let Some(caps) = avg_re.captures(&result) {
            let full_match = caps.get(0).unwrap();
            let range = caps.get(1).unwrap().as_str();
            let values = self.get_range_values(range, results)?;
            let avg = if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 };
            result = format!(
                "{}{}{}",
                &result[..full_match.start()],
                avg,
                &result[full_match.end()..]
            );
        }

        // Handle MIN
        let min_re = Regex::new(r"(?i)MIN\(([^)]+)\)").unwrap();
        while let Some(caps) = min_re.captures(&result) {
            let full_match = caps.get(0).unwrap();
            let range = caps.get(1).unwrap().as_str();
            let values = self.get_range_values(range, results)?;
            let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            result = format!(
                "{}{}{}",
                &result[..full_match.start()],
                min,
                &result[full_match.end()..]
            );
        }

        // Handle MAX
        let max_re = Regex::new(r"(?i)MAX\(([^)]+)\)").unwrap();
        while let Some(caps) = max_re.captures(&result) {
            let full_match = caps.get(0).unwrap();
            let range = caps.get(1).unwrap().as_str();
            let values = self.get_range_values(range, results)?;
            let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            result = format!(
                "{}{}{}",
                &result[..full_match.start()],
                max,
                &result[full_match.end()..]
            );
        }

        // Handle COUNT
        let count_re = Regex::new(r"(?i)COUNT\(([^)]+)\)").unwrap();
        while let Some(caps) = count_re.captures(&result) {
            let full_match = caps.get(0).unwrap();
            let range = caps.get(1).unwrap().as_str();
            let values = self.get_range_values(range, results)?;
            result = format!(
                "{}{}{}",
                &result[..full_match.start()],
                values.len(),
                &result[full_match.end()..]
            );
        }

        Ok(result)
    }

    /// Substitute cell references with their values
    fn substitute_cell_refs(&self, formula: &str, results: &HashMap<CellRef, f64>) -> Result<String, CalcError> {
        let mut result = formula.to_string();

        // Find all cell references and replace from end to start
        let cell_re = Regex::new(r"[A-Za-z]+\d+").unwrap();
        let matches: Vec<_> = cell_re.find_iter(&result.to_uppercase())
            .map(|m| (m.start(), m.end(), m.as_str().to_string()))
            .collect();

        // Replace from end to start to preserve positions
        for (start, end, cell_str) in matches.into_iter().rev() {
            if let Some(cell_ref) = parse_cell_ref(&cell_str) {
                let value = self.get_cell_value(&cell_ref, results)?;
                result = format!("{}{}{}", &result[..start], value, &result[end..]);
            }
        }

        Ok(result)
    }
}

