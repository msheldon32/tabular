use std::collections::{HashMap, HashSet};

use crate::table::table::Table;
use crate::util::{CellRef, CalcError, col_from_letters, letters_from_col};
use crate::numeric::parser::{self, Expr, BinOp, ParseError};
use crate::numeric::formula::{self as formula, ExprEvaluator};

#[derive(Debug, Clone)]
pub enum CalcType {
    Int(i64),
    Str(String),
    Float(f64),
    Bool(bool)
}

impl CalcType {
    fn use_int(&self) -> Option<i64> {
        if let CalcType::Int(i) = self { Some(*i) } else { None }
    }
    fn use_string(&self) -> Option<String> {
        if let CalcType::Str(s) = self { Some(s.to_string()) } else { None }
    }
    fn use_float(&self) -> Option<f64> {
        if let CalcType::Float(x) = self { Some(*x) } else { None }
    }
    fn use_bool(&self) -> Option<bool> {
        if let CalcType::Bool(x) = self { Some(*x) } else { None }
    }

    fn numeric_precedence(l: CalcType, r: CalcType) -> Result<(CalcType, CalcType), CalcError> {
        match (l,r) {
            (CalcType::Int(a), CalcType::Int(b)) => Ok((CalcType::Int(a), CalcType::Int(b))),
            (CalcType::Int(a), CalcType::Float(b)) => Ok((CalcType::Float(a as f64), CalcType::Float(b))),
            (CalcType::Float(a), CalcType::Int(b)) => Ok((CalcType::Float(a), CalcType::Float(b as f64))),
            (CalcType::Float(a), CalcType::Float(b)) => Ok((CalcType::Float(a), CalcType::Float(b))),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    fn bin_op(op: BinOp, l: CalcType, r: CalcType) -> Result<CalcType, CalcError> {
        match op {
            BinOp::And => {
                match (l,r) {
                    (CalcType::Bool(a), CalcType::Bool(b)) => {
                        Ok(CalcType::Bool(a && b))
                    }
                    _default => {
                        Err(CalcError::EvalError("Boolean operation on non-boolean expressions".to_string()))
                    }
                }
            }
            BinOp::Or => {
                match (l,r) {
                    (CalcType::Bool(a), CalcType::Bool(b)) => {
                        Ok(CalcType::Bool(a || b))
                    }
                    _default => {
                        Err(CalcError::EvalError("Boolean operation on non-boolean expressions".to_string()))
                    }
                }
            }
            BinOp::Add => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(a+b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a+b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Sub => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(a-b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a-b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Mul => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(a*b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a*b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Div => {
                // making an exception here to the rule since integer division would be surprising
                // for most
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Float((a as f64)/(b as f64))),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a/b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Pow => {
                // continuing the exception here, since rust is picky about overflows and negative
                // exponents
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Float((a as f64).powf(b as f64))),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a.powf(b))),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Mod => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(a%b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a%b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Lt => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Bool(a < b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Bool(a < b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Le => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Bool(a <= b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Bool(a <= b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Gt => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Bool(a > b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Bool(a > b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }
            BinOp::Ge => {
                match CalcType::numeric_precedence(l,r) {
                    Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Bool(a >= b)),
                    Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Bool(a >= b)),
                    _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
                }
            }

            BinOp::Ne => {
                match (l,r) {
                    (CalcType::Int(a), CalcType::Int(b)) => Ok(CalcType::Bool(a != b)),
                    (CalcType::Float(a), CalcType::Int(b)) => Ok(CalcType::Bool(a != (b as f64))),
                    (CalcType::Int(a), CalcType::Float(b)) => Ok(CalcType::Bool((a as f64) != b)),
                    (CalcType::Float(a), CalcType::Float(b)) => Ok(CalcType::Bool(a != b)),
                    (CalcType::Str(a), CalcType::Str(b)) => Ok(CalcType::Bool(a != b)),
                    (CalcType::Bool(a), CalcType::Bool(b)) => Ok(CalcType::Bool(a != b)),
                    _default => Err(CalcError::EvalError("Incompatible types".to_string()))
                }
            }
            BinOp::Eq => {
                match (l,r) {
                    (CalcType::Int(a), CalcType::Int(b)) => Ok(CalcType::Bool(a == b)),
                    (CalcType::Float(a), CalcType::Int(b)) => Ok(CalcType::Bool(a == (b as f64))),
                    (CalcType::Int(a), CalcType::Float(b)) => Ok(CalcType::Bool((a as f64) == b)),
                    (CalcType::Float(a), CalcType::Float(b)) => Ok(CalcType::Bool(a == b)),
                    (CalcType::Str(a), CalcType::Str(b)) => Ok(CalcType::Bool(a == b)),
                    (CalcType::Bool(a), CalcType::Bool(b)) => Ok(CalcType::Bool(a == b)),
                    _default => Err(CalcError::EvalError("Incompatible types".to_string()))
                }
            }
        }
    }
}

/// Format a numeric value for display, removing unnecessary trailing zeros
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() { "Inf" } else { "-Inf" }.to_string()
    } else {
        format!("{:.10}", value).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Convert a cell reference to a human-readable name like "A1", "B2", etc.
fn cell_ref_to_name(cell: &CellRef) -> String {
    format!("{}{}", letters_from_col(cell.col), cell.row + 1)
}

impl From<ParseError> for CalcError {
    fn from(e: ParseError) -> Self {
        CalcError::ParseError(e.to_string())
    }
}

pub struct Calculator<'a> {
    table: &'a Table,
    skip_header: bool,
}

impl<'a> Calculator<'a> {
    pub fn new(table: &'a Table, skip_header: bool) -> Self {
        Self { table, skip_header }
    }

    /// Evaluate all formula cells and return updates as (row, col, value)
    pub fn evaluate_all(&self) -> Result<Vec<(usize, usize, String)>, CalcError> {
        // Find all formula cells and parse them
        let mut formulas: HashMap<CellRef, Expr> = HashMap::new();
        for (row_idx, row) in self.table.rows_iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                if cell.starts_with('=') {
                    let expr = parser::parse(cell)?;
                    formulas.insert(
                        CellRef { row: row_idx, col: col_idx },
                        expr,
                    );
                }
            }
        }

        if formulas.is_empty() {
            return Ok(vec![]);
        }

        // Build dependency graph
        let mut dependencies: HashMap<CellRef, HashSet<CellRef>> = HashMap::new();
        for (cell_ref, expr) in &formulas {
            let refs = self.extract_cell_refs_from_expr(expr)?;
            dependencies.insert(cell_ref.clone(), refs);
        }

        // Check for circular references and get evaluation order
        let order = self.topological_sort(&formulas, &dependencies)?;

        // Evaluate in order
        let mut results: HashMap<CellRef, CalcType> = HashMap::new();
        let mut updates: Vec<(usize, usize, String)> = Vec::new();

        for cell_ref in order {
            let expr = &formulas[&cell_ref];
            let value = self.evaluate_expr(expr, &results)?;
            results.insert(cell_ref.clone(), CalcType::Float(value));
            updates.push((cell_ref.row, cell_ref.col, format_number(value)));
        }

        Ok(updates)
    }

    /// Extract all cell references from a parsed expression
    fn extract_cell_refs_from_expr(&self, expr: &Expr) -> Result<HashSet<CellRef>, CalcError> {
        let mut refs = HashSet::new();
        self.collect_refs(expr, &mut refs)?;
        Ok(refs)
    }

    fn collect_refs(&self, expr: &Expr, refs: &mut HashSet<CellRef>) -> Result<(), CalcError> {
        match expr {
            Expr::Number(_) => {}
            Expr::CellRef { col, row } => {
                let col_idx = col_from_letters(col);
                refs.insert(CellRef { row: *row - 1, col: col_idx });
            }
            Expr::Range { start, end } => {
                // Expand range to all cells
                if let (Expr::CellRef { col: start_col, row: start_row },
                        Expr::CellRef { col: end_col, row: end_row }) = (start.as_ref(), end.as_ref()) {
                    let start_col_idx = col_from_letters(start_col);
                    let end_col_idx = col_from_letters(end_col);
                    let row_min = (*start_row).min(*end_row);
                    let row_max = (*start_row).max(*end_row);
                    let col_min = start_col_idx.min(end_col_idx);
                    let col_max = start_col_idx.max(end_col_idx);

                    for r in row_min..=row_max {
                        for c in col_min..=col_max {
                            refs.insert(CellRef { row: r - 1, col: c });
                        }
                    }
                }
            }
            Expr::RowRange { start, end } => {
                let row_min = (*start).min(*end);
                let row_max = (*start).max(*end);
                for r in row_min..=row_max {
                    for c in 0..self.table.col_count() {
                        refs.insert(CellRef { row: r - 1, col: c });
                    }
                }
            }
            Expr::ColRange { start, end } => {
                let start_col = col_from_letters(start);
                let end_col = col_from_letters(end);
                let col_min = start_col.min(end_col);
                let col_max = start_col.max(end_col);
                let row_start = if self.skip_header { 1 } else { 0 };
                for r in row_start..self.table.row_count() {
                    for c in col_min..=col_max {
                        refs.insert(CellRef { row: r, col: c });
                    }
                }
            }
            Expr::FnCall { args, .. } => {
                for arg in args {
                    self.collect_refs(arg, refs)?;
                }
            }
            Expr::BinOp { left, right, .. } => {
                self.collect_refs(left, refs)?;
                self.collect_refs(right, refs)?;
            }
            Expr::Neg(inner) | Expr::Not(inner) => {
                self.collect_refs(inner, refs)?;
            }
            Expr::Boolean(_) => {}
        }
        Ok(())
    }

    /// Topological sort with cycle detection
    fn topological_sort(
        &self,
        formulas: &HashMap<CellRef, Expr>,
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
        formulas: &HashMap<CellRef, Expr>,
        dependencies: &HashMap<CellRef, HashSet<CellRef>>,
        visited: &mut HashSet<CellRef>,
        in_stack: &mut HashSet<CellRef>,
        order: &mut Vec<CellRef>,
    ) -> Result<(), CalcError> {
        if in_stack.contains(cell) {
            return Err(CalcError::CircularReference(cell_ref_to_name(cell)));
        }

        if visited.contains(cell) {
            return Ok(());
        }

        in_stack.insert(cell.clone());
        visited.insert(cell.clone());

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

    /// Get cell value as CalcType
    fn get_cell_value(
        &self,
        cell: &CellRef,
        results: &HashMap<CellRef, CalcType>,
    ) -> CalcType {
        if let Some(val) = results.get(cell) {
            return val.clone();
        }

        let empty_str = String::new();

        let cell_content = self
            .table
            .get_cell(cell.row, cell.col).unwrap_or(&empty_str);

        let trimmed = cell_content.trim();

        if trimmed.is_empty() {
            return CalcType::Float(0.0);
        }

        // this is put before i64 for debugging, move it afterwards!
        if let Ok(f) = trimmed.parse::<f64>() {
            return CalcType::Float(f);
        }

        if let Ok(i) = trimmed.parse::<i64>() {
            return CalcType::Int(i);
        }

        if let Ok(b) = trimmed.to_lowercase().parse::<bool>() {
            return CalcType::Bool(b);
        }

        CalcType::Str(trimmed.to_string())
    }

    /// Evaluate an expression to f64
    /// Booleans are represented as 1.0 (true) and 0.0 (false)
    fn evaluate_expr(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<f64, CalcError> {
        match expr {
            Expr::Number(n) => Ok(*n),

            Expr::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),

            Expr::CellRef { col, row } => {
                let col_idx = col_from_letters(col);
                let cell = CellRef { row: *row - 1, col: col_idx };
                self.get_cell_value(&cell, results).use_float().ok_or_else(|| CalcError::ParseError("Not a number".to_string()))
            }

            Expr::Neg(inner) => {
                let val = self.evaluate_expr(inner, results)?;
                Ok(-val)
            }

            Expr::Not(inner) => {
                let val = self.evaluate_expr(inner, results)?;
                // 0 is false, anything else is true
                Ok(if val == 0.0 { 1.0 } else { 0.0 })
            }

            Expr::BinOp { op, left, right } => {
                // Short-circuit evaluation for AND/OR
                match op {
                    BinOp::And => {
                        let l = self.evaluate_expr(left, results)?;
                        if l == 0.0 {
                            return Ok(0.0); // Short-circuit
                        }
                        let r = self.evaluate_expr(right, results)?;
                        Ok(if r != 0.0 { 1.0 } else { 0.0 })
                    }
                    BinOp::Or => {
                        let l = self.evaluate_expr(left, results)?;
                        if l != 0.0 {
                            return Ok(1.0); // Short-circuit
                        }
                        let r = self.evaluate_expr(right, results)?;
                        Ok(if r != 0.0 { 1.0 } else { 0.0 })
                    }
                    _ => {
                        let l = self.evaluate_expr(left, results)?;
                        let r = self.evaluate_expr(right, results)?;
                        Ok(match op {
                            BinOp::Add => l + r,
                            BinOp::Sub => l - r,
                            BinOp::Mul => l * r,
                            BinOp::Div => l / r,
                            BinOp::Pow => l.powf(r),
                            BinOp::Mod => l % r,
                            BinOp::Eq => if (l - r).abs() < f64::EPSILON { 1.0 } else { 0.0 },
                            BinOp::Ne => if (l - r).abs() >= f64::EPSILON { 1.0 } else { 0.0 },
                            BinOp::Lt => if l < r { 1.0 } else { 0.0 },
                            BinOp::Le => if l <= r { 1.0 } else { 0.0 },
                            BinOp::Gt => if l > r { 1.0 } else { 0.0 },
                            BinOp::Ge => if l >= r { 1.0 } else { 0.0 },
                            BinOp::And | BinOp::Or => unreachable!(),
                        })
                    }
                }
            }

            Expr::FnCall { name, args } => {
                formula::evaluate_function(self, name, args, results)
            }

            // Ranges should only appear as function arguments
            Expr::Range { .. } | Expr::RowRange { .. } | Expr::ColRange { .. } => {
                Err(CalcError::EvalError("Range used outside of function".to_string()))
            }
        }
    }

    /// Expand a range expression to a Vec of f64 values
    pub fn expand_range(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<f64>, CalcError> {
        match expr {
            Expr::Range { start, end } => {
                if let (Expr::CellRef { col: start_col, row: start_row },
                        Expr::CellRef { col: end_col, row: end_row }) = (start.as_ref(), end.as_ref()) {
                    let start_col_idx = col_from_letters(start_col);
                    let end_col_idx = col_from_letters(end_col);
                    let row_min = (*start_row).min(*end_row);
                    let row_max = (*start_row).max(*end_row);
                    let col_min = start_col_idx.min(end_col_idx);
                    let col_max = start_col_idx.max(end_col_idx);

                    let mut values = Vec::new();
                    for r in row_min..=row_max {
                        for c in col_min..=col_max {
                            let cell = CellRef { row: r - 1, col: c };
                            let val = self.get_cell_value(&cell, results).use_float().ok_or_else(|| CalcError::ParseError("Not a number".to_string()))?;
                            values.push(val);
                        }
                    }
                    Ok(values)
                } else {
                    Err(CalcError::EvalError("Invalid range".to_string()))
                }
            }
            Expr::RowRange { start, end } => {
                let row_min = (*start).min(*end);
                let row_max = (*start).max(*end);
                let mut values = Vec::new();
                for r in row_min..=row_max {
                    for c in 0..self.table.col_count() {
                        let cell = CellRef { row: r - 1, col: c };
                        let val = self.get_cell_value(&cell, results).use_float().ok_or_else(|| CalcError::ParseError("Not a number".to_string()))?;
                        values.push(val)
                    }
                }
                Ok(values)
            }
            Expr::ColRange { start, end } => {
                let start_col = col_from_letters(start);
                let end_col = col_from_letters(end);
                let col_min = start_col.min(end_col);
                let col_max = start_col.max(end_col);
                let row_start = if self.skip_header { 1 } else { 0 };
                let mut values = Vec::new();
                for r in row_start..self.table.row_count() {
                    for c in col_min..=col_max {
                        let cell = CellRef { row: r, col: c };
                        let val = self.get_cell_value(&cell, results).use_float().ok_or_else(|| CalcError::ParseError("Not a number".to_string()))?;
                        values.push(val);
                    }
                }
                Ok(values)
            }
            // Single value - wrap in vec
            _ => Ok(vec![self.evaluate_expr(expr, results)?])
        }
    }

}

impl ExprEvaluator for Calculator<'_> {
    fn eval(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<f64, CalcError> {
        self.evaluate_expr(expr, results)
    }

    fn expand(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<f64>, CalcError> {
        self.expand_range(expr, results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table::new(
            data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect()
        )
    }

    #[test]
    fn test_basic_formula() {
        let table = make_table(vec![
            vec!["10", "20", "=A1+B1"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "30");
    }

    #[test]
    fn test_sum() {
        let table = make_table(vec![
            vec!["1", "2", "3"],
            vec!["4", "5", "6"],
            vec!["=sum(A1:C2)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "21");
    }

    #[test]
    fn test_avg() {
        let table = make_table(vec![
            vec!["10", "20", "30", "40"],
            vec!["=avg(A1:D1)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "25");
    }

    #[test]
    fn test_stdev() {
        let table = make_table(vec![
            vec!["2", "4", "4", "4", "5", "5", "7", "9"],
            vec!["=stdev(A1:H1)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        let val: f64 = results[0].2.parse().unwrap();
        assert!((val - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_median() {
        let table = make_table(vec![
            vec!["1", "3", "5", "7", "9"],
            vec!["=median(A1:E1)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "5");
    }

    #[test]
    fn test_correl() {
        let table = make_table(vec![
            vec!["1", "2"],
            vec!["2", "4"],
            vec!["3", "6"],
            vec!["=correl(A1:A3,B1:B3)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        let val: f64 = results[0].2.parse().unwrap();
        assert!((val - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_math_functions() {
        let table = make_table(vec![
            vec!["=sqrt(16)"],
            vec!["=abs(-5)"],
            vec!["=pow(2,3)"],
            vec!["=floor(3.7)"],
            vec!["=ceil(3.2)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "4");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "5");
        assert_eq!(results.iter().find(|r| r.0 == 2).unwrap().2, "8");
        assert_eq!(results.iter().find(|r| r.0 == 3).unwrap().2, "3");
        assert_eq!(results.iter().find(|r| r.0 == 4).unwrap().2, "4");
    }

    #[test]
    fn test_trig_functions() {
        let table = make_table(vec![
            vec!["=sin(0)"],
            vec!["=cos(0)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        let sin_val: f64 = results.iter().find(|r| r.0 == 0).unwrap().2.parse().unwrap();
        let cos_val: f64 = results.iter().find(|r| r.0 == 1).unwrap().2.parse().unwrap();
        assert!(sin_val.abs() < 0.0001);
        assert!((cos_val - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_constants() {
        let table = make_table(vec![
            vec!["=PI()"],
            vec!["=E()"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        let pi_val: f64 = results.iter().find(|r| r.0 == 0).unwrap().2.parse().unwrap();
        let e_val: f64 = results.iter().find(|r| r.0 == 1).unwrap().2.parse().unwrap();
        assert!((pi_val - std::f64::consts::PI).abs() < 0.0001);
        assert!((e_val - std::f64::consts::E).abs() < 0.0001);
    }

    #[test]
    fn test_combinatorics() {
        let table = make_table(vec![
            vec!["=combin(5,2)"],
            vec!["=permut(5,2)"],
            vec!["=fact(5)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "10");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "20");
        assert_eq!(results.iter().find(|r| r.0 == 2).unwrap().2, "120");
    }

    #[test]
    fn test_gcd_lcm() {
        let table = make_table(vec![
            vec!["=gcd(12,18)"],
            vec!["=lcm(12,18)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "6");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "36");
    }

    #[test]
    fn test_percentile() {
        let table = make_table(vec![
            vec!["1", "2", "3", "4", "5"],
            vec!["=percentile(A1:E1,0.5)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "3");
    }

    #[test]
    fn test_nested_functions() {
        let table = make_table(vec![
            vec!["=sqrt(abs(-16))"],
            vec!["=pow(sqrt(4),2)"],
            vec!["=abs(floor(-3.7))"],
            vec!["=round(sqrt(2),2)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "4");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "4");
        assert_eq!(results.iter().find(|r| r.0 == 2).unwrap().2, "4");
        assert_eq!(results.iter().find(|r| r.0 == 3).unwrap().2, "1.41");
    }

    #[test]
    fn test_deeply_nested_functions() {
        let table = make_table(vec![
            vec!["=sqrt(sqrt(sqrt(256)))"],
            vec!["=abs(abs(abs(-5)))"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "2");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "5");
    }

    #[test]
    fn test_column_range_sum() {
        let table = make_table(vec![
            vec!["1", "10", "100", "=sum(A:A)"],
            vec!["2", "20", "200", "=sum(B:B)"],
            vec!["3", "30", "300", "=sum(C:C)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0 && r.1 == 3).unwrap().2, "6");
        assert_eq!(results.iter().find(|r| r.0 == 1 && r.1 == 3).unwrap().2, "60");
        assert_eq!(results.iter().find(|r| r.0 == 2 && r.1 == 3).unwrap().2, "600");
    }

    #[test]
    fn test_row_range_sum_1indexed() {
        let table = make_table(vec![
            vec!["1", "2", "3"],
            vec!["10", "20", "30"],
            vec!["100", "200", "300"],
            vec!["=sum(1:1)", "=sum(2:2)", "=sum(3:3)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 0).unwrap().2, "6");
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 1).unwrap().2, "60");
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 2).unwrap().2, "600");
    }

    #[test]
    fn test_lowercase_column_range() {
        let table = make_table(vec![
            vec!["1", "10", "=sum(a:a)"],
            vec!["2", "20", "=sum(b:b)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0 && r.1 == 2).unwrap().2, "3");
        assert_eq!(results.iter().find(|r| r.0 == 1 && r.1 == 2).unwrap().2, "30");
    }

    #[test]
    fn test_arithmetic_expression() {
        let table = make_table(vec![
            vec!["5", "3", "=(A1+B1)*2"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "16");
    }

    #[test]
    fn test_power_operator() {
        let table = make_table(vec![
            vec!["2", "=A1^3"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "8");
    }

    // === Boolean expression tests ===

    #[test]
    fn test_boolean_literals() {
        let table = make_table(vec![
            vec!["=TRUE", "=FALSE"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "0");
    }

    #[test]
    fn test_not_operator() {
        let table = make_table(vec![
            vec!["=NOT TRUE", "=NOT FALSE", "=!TRUE"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "0");
    }

    #[test]
    fn test_and_operator() {
        let table = make_table(vec![
            vec!["=TRUE AND TRUE", "=TRUE AND FALSE", "=FALSE AND TRUE", "=FALSE AND FALSE"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "0");
    }

    #[test]
    fn test_or_operator() {
        let table = make_table(vec![
            vec!["=TRUE OR TRUE", "=TRUE OR FALSE", "=FALSE OR TRUE", "=FALSE OR FALSE"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "0");
    }

    #[test]
    fn test_symbolic_boolean_operators() {
        let table = make_table(vec![
            vec!["=TRUE && FALSE", "=TRUE || FALSE"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "1");
    }

    #[test]
    fn test_if_function() {
        let table = make_table(vec![
            vec!["=IF(TRUE, 10, 20)", "=IF(FALSE, 10, 20)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "10");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "20");
    }

    #[test]
    fn test_if_with_comparison() {
        let table = make_table(vec![
            vec!["10", "=IF(A1>5, 100, 200)", "=IF(A1<5, 100, 200)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "100");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "200");
    }

    #[test]
    fn test_nested_if() {
        let table = make_table(vec![
            vec!["5", "=IF(A1>10, 1, IF(A1>3, 2, 3))"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "2");
    }

    #[test]
    fn test_and_function() {
        let table = make_table(vec![
            vec!["=AND(TRUE, TRUE)", "=AND(TRUE, FALSE)", "=AND(TRUE, TRUE, TRUE)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "1");
    }

    #[test]
    fn test_or_function() {
        let table = make_table(vec![
            vec!["=OR(FALSE, FALSE)", "=OR(FALSE, TRUE)", "=OR(FALSE, FALSE, TRUE)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "1");
    }

    #[test]
    fn test_not_function() {
        let table = make_table(vec![
            vec!["=NOT(TRUE)", "=NOT(FALSE)", "=NOT(1)", "=NOT(0)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "1");
    }

    #[test]
    fn test_complex_boolean_formula() {
        let table = make_table(vec![
            vec!["10", "5", "=IF(A1>5 AND B1<10, 1, 0)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "1");
    }

    #[test]
    fn test_boolean_with_cell_refs() {
        let table = make_table(vec![
            vec!["1", "0", "=A1 AND B1", "=A1 OR B1"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "0");
        assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "1");
    }

    #[test]
    fn test_iferror() {
        let table = make_table(vec![
            vec!["=IFERROR(1/0, 999)", "=IFERROR(10, 999)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "999");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "10");
    }

    #[test]
    fn test_true_false_functions() {
        let table = make_table(vec![
            vec!["=TRUE()", "=FALSE()"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "1");
        assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "0");
    }
}
