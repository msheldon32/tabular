use std::collections::{HashMap, HashSet};

use crate::table::table::Table;
use crate::util::{CellRef, CalcError, col_from_letters, letters_from_col};
use crate::numeric::parser::{self, Expr, ParseError};
use crate::numeric::formula::{self as formula, ExprEvaluator};
use crate::numeric::calctype::CalcType;
use crate::plugin::PluginManager;


/// Format a numeric value for display, removing unnecessary trailing zeros
fn format_number(vt: CalcType) -> String {
    match vt {
        CalcType::Float(value) => {
            if value.fract() == 0.0 && value.abs() < 1e15 {
                format!("{}", value as i64)
            } else if value.is_nan() {
                "NaN".to_string()
            } else if value.is_infinite() {
                if value.is_sign_positive() { "Inf" } else { "-Inf" }.to_string()
            } else {
                format!("{:.10}", value).trim_end_matches('0').trim_end_matches('.').to_string()
            }
        },
        CalcType::Int(value) => {
            format!("{}", value)
        },
        CalcType::Str(value) => {
            format!("{}", value)
        },
        CalcType::Bool(value) => {
            format!("{}", value)
        }
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
    plugin_manager: Option<&'a PluginManager>,
}

impl<'a> Calculator<'a> {
    #[allow(dead_code)]
    pub fn new(table: &'a Table, skip_header: bool) -> Self {
        Self { table, skip_header, plugin_manager: None }
    }

    pub fn with_plugins(table: &'a Table, skip_header: bool, plugin_manager: &'a PluginManager) -> Self {
        Self { table, skip_header, plugin_manager: Some(plugin_manager) }
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
            results.insert(cell_ref.clone(), value.clone());
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
            Expr::Literal(_) => {}
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

        if let Ok(i) = trimmed.parse::<i64>() {
            return CalcType::Int(i);
        }

        if let Ok(f) = trimmed.parse::<f64>() {
            return CalcType::Float(f);
        }

        if let Ok(b) = trimmed.to_lowercase().parse::<bool>() {
            return CalcType::Bool(b);
        }

        CalcType::Str(trimmed.to_string())
    }

    /// Evaluate an expression to CalcType
    fn evaluate_expr(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<CalcType, CalcError> {
        match expr {
            Expr::Literal(val) => Ok(val.clone()),

            Expr::CellRef { col, row } => {
                let col_idx = col_from_letters(col);
                let cell = CellRef { row: *row - 1, col: col_idx };
                Ok(self.get_cell_value(&cell, results))
            }

            Expr::Neg(inner) => {
                let val = self.evaluate_expr(inner, results)?;
                CalcType::negate(val)
            }

            Expr::Not(inner) => {
                let val = self.evaluate_expr(inner, results)?;
                CalcType::not(val)
            }

            Expr::BinOp { op, left, right } => {
                let a = self.evaluate_expr(left, results)?;
                let b = self.evaluate_expr(right, results)?;
                CalcType::bin_op(*op, a, b)
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
    pub fn expand_range(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<CalcType>, CalcError> {
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
                            let val = self.get_cell_value(&cell, results);
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
                        let val = self.get_cell_value(&cell, results);
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
                        let val = self.get_cell_value(&cell, results);
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
    fn eval(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<CalcType, CalcError> {
        self.evaluate_expr(expr, results)
    }

    fn expand(&self, expr: &Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<CalcType>, CalcError> {
        self.expand_range(expr, results)
    }

    fn call_plugin_function(&self, name: &str, args: &[CalcType]) -> Option<Result<CalcType, CalcError>> {
        let pm = self.plugin_manager?;
        if pm.has_function(name) {
            Some(pm.execute_function(name, args))
        } else {
            None
        }
    }
}
