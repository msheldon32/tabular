use std::collections::{HashMap, HashSet};
use rand::Rng;

use crate::table::table::Table;
use crate::util::{CellRef, CalcError, col_from_letters, letters_from_col};
use crate::numeric::parser::{self, Expr, BinOp, ParseError};

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
        let mut results: HashMap<CellRef, f64> = HashMap::new();
        let mut updates: Vec<(usize, usize, String)> = Vec::new();

        for cell_ref in order {
            let expr = &formulas[&cell_ref];
            let value = self.evaluate_expr(expr, &results)?;
            results.insert(cell_ref.clone(), value);
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
            Expr::Neg(inner) => {
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

    /// Get cell value as f64
    fn get_cell_value(&self, cell: &CellRef, results: &HashMap<CellRef, f64>) -> Result<f64, CalcError> {
        if let Some(&val) = results.get(cell) {
            return Ok(val);
        }

        let cell_content = self.table.get_cell(cell.row, cell.col)
            .ok_or_else(|| CalcError::InvalidReference(cell_ref_to_name(cell)))?;

        if cell_content.trim().is_empty() {
            return Ok(0.0);
        }

        crate::numeric::format::parse_numeric(cell_content)
            .ok_or_else(|| CalcError::EvalError(format!("{} is not a number", cell_ref_to_name(cell))))
    }

    /// Evaluate an expression to f64
    fn evaluate_expr(&self, expr: &Expr, results: &HashMap<CellRef, f64>) -> Result<f64, CalcError> {
        match expr {
            Expr::Number(n) => Ok(*n),

            Expr::CellRef { col, row } => {
                let col_idx = col_from_letters(col);
                let cell = CellRef { row: *row - 1, col: col_idx };
                self.get_cell_value(&cell, results)
            }

            Expr::Neg(inner) => {
                let val = self.evaluate_expr(inner, results)?;
                Ok(-val)
            }

            Expr::BinOp { op, left, right } => {
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
                })
            }

            Expr::FnCall { name, args } => {
                self.evaluate_function(name, args, results)
            }

            // Ranges should only appear as function arguments
            Expr::Range { .. } | Expr::RowRange { .. } | Expr::ColRange { .. } => {
                Err(CalcError::EvalError("Range used outside of function".to_string()))
            }
        }
    }

    /// Expand a range expression to a Vec of f64 values
    fn expand_range(&self, expr: &Expr, results: &HashMap<CellRef, f64>) -> Result<Vec<f64>, CalcError> {
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
                            values.push(self.get_cell_value(&cell, results)?);
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
                        values.push(self.get_cell_value(&cell, results)?);
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
                        values.push(self.get_cell_value(&cell, results)?);
                    }
                }
                Ok(values)
            }
            // Single value - wrap in vec
            _ => Ok(vec![self.evaluate_expr(expr, results)?])
        }
    }

    /// Evaluate a function call
    fn evaluate_function(&self, name: &str, args: &[Expr], results: &HashMap<CellRef, f64>) -> Result<f64, CalcError> {
        match name {
            // === Aggregate functions ===
            "SUM" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.iter().sum())
            }
            "AVG" | "AVERAGE" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { Ok(0.0) } else { Ok(vals.iter().sum::<f64>() / vals.len() as f64) }
            }
            "MIN" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.iter().cloned().fold(f64::INFINITY, f64::min))
            }
            "MAX" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
            }
            "COUNT" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.len() as f64)
            }
            "PRODUCT" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.iter().product())
            }
            "MEDIAN" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let mut sorted = vals;
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len() % 2 == 0 {
                    Ok((sorted[mid - 1] + sorted[mid]) / 2.0)
                } else {
                    Ok(sorted[mid])
                }
            }
            "MODE" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for &v in &vals {
                    let key = (v * 1e10) as i64;
                    *counts.entry(key).or_insert(0) += 1;
                }
                let (mode_key, _) = counts.into_iter().max_by_key(|&(_, c)| c).unwrap();
                Ok(mode_key as f64 / 1e10)
            }
            "STDEV" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.len() < 2 { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
                Ok(variance.sqrt())
            }
            "STDEVP" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64;
                Ok(variance.sqrt())
            }
            "VAR" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.len() < 2 { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                Ok(vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64)
            }
            "VARP" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                Ok(vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64)
            }
            "GEOMEAN" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let product: f64 = vals.iter().product();
                if product < 0.0 { return Ok(f64::NAN); }
                Ok(product.powf(1.0 / vals.len() as f64))
            }
            "HARMEAN" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let sum_recip: f64 = vals.iter().map(|x| 1.0 / x).sum();
                Ok(vals.len() as f64 / sum_recip)
            }
            "SUMSQ" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                Ok(vals.iter().map(|x| x * x).sum())
            }
            "AVEDEV" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                Ok(vals.iter().map(|x| (x - mean).abs()).sum::<f64>() / vals.len() as f64)
            }
            "DEVSQ" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                if vals.is_empty() { return Ok(0.0); }
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                Ok(vals.iter().map(|x| (x - mean).powi(2)).sum())
            }
            "KURT" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                let n = vals.len() as f64;
                if n < 4.0 { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / n;
                let m2 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
                let m4 = vals.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n;
                let g2 = m4 / (m2 * m2) - 3.0;
                Ok(((n + 1.0) * g2 + 6.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0)))
            }
            "SKEW" => {
                self.require_args(name, args, 1)?;
                let vals = self.expand_range(&args[0], results)?;
                let n = vals.len() as f64;
                if n < 3.0 { return Ok(f64::NAN); }
                let mean = vals.iter().sum::<f64>() / n;
                let m2 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
                let m3 = vals.iter().map(|x| (x - mean).powi(3)).sum::<f64>() / n;
                let g1 = m3 / m2.powf(1.5);
                Ok((n * (n - 1.0)).sqrt() / (n - 2.0) * g1)
            }

            // === Two-range functions ===
            "CORREL" => {
                self.require_args(name, args, 2)?;
                let vals1 = self.expand_range(&args[0], results)?;
                let vals2 = self.expand_range(&args[1], results)?;
                if vals1.len() != vals2.len() || vals1.is_empty() { return Ok(f64::NAN); }
                let n = vals1.len() as f64;
                let mean1 = vals1.iter().sum::<f64>() / n;
                let mean2 = vals2.iter().sum::<f64>() / n;
                let cov: f64 = vals1.iter().zip(vals2.iter())
                    .map(|(a, b)| (a - mean1) * (b - mean2))
                    .sum();
                let std1 = vals1.iter().map(|x| (x - mean1).powi(2)).sum::<f64>().sqrt();
                let std2 = vals2.iter().map(|x| (x - mean2).powi(2)).sum::<f64>().sqrt();
                if std1 == 0.0 || std2 == 0.0 { Ok(f64::NAN) } else { Ok(cov / (std1 * std2)) }
            }
            "COVAR" => {
                self.require_args(name, args, 2)?;
                let vals1 = self.expand_range(&args[0], results)?;
                let vals2 = self.expand_range(&args[1], results)?;
                if vals1.len() != vals2.len() || vals1.is_empty() { return Ok(f64::NAN); }
                let n = vals1.len() as f64;
                let mean1 = vals1.iter().sum::<f64>() / n;
                let mean2 = vals2.iter().sum::<f64>() / n;
                Ok(vals1.iter().zip(vals2.iter())
                    .map(|(a, b)| (a - mean1) * (b - mean2))
                    .sum::<f64>() / n)
            }
            "PERCENTILE" => {
                self.require_args(name, args, 2)?;
                let vals = self.expand_range(&args[0], results)?;
                let k = self.evaluate_expr(&args[1], results)?;
                if vals.is_empty() || k < 0.0 || k > 1.0 { return Ok(f64::NAN); }
                let mut sorted = vals;
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = k * (sorted.len() - 1) as f64;
                let lower = idx.floor() as usize;
                let upper = idx.ceil() as usize;
                if lower == upper {
                    Ok(sorted[lower])
                } else {
                    Ok(sorted[lower] + (sorted[upper] - sorted[lower]) * (idx - lower as f64))
                }
            }
            "QUARTILE" => {
                self.require_args(name, args, 2)?;
                let vals = self.expand_range(&args[0], results)?;
                let q = self.evaluate_expr(&args[1], results)? as i64;
                if vals.is_empty() || q < 0 || q > 4 { return Ok(f64::NAN); }
                let k = q as f64 * 0.25;
                let mut sorted = vals;
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = k * (sorted.len() - 1) as f64;
                let lower = idx.floor() as usize;
                let upper = idx.ceil() as usize;
                if lower == upper {
                    Ok(sorted[lower])
                } else {
                    Ok(sorted[lower] + (sorted[upper] - sorted[lower]) * (idx - lower as f64))
                }
            }

            // === Single-arg math functions ===
            "ABS" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.abs()) }
            "SQRT" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.sqrt()) }
            "EXP" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.exp()) }
            "LN" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.ln()) }
            "LOG10" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.log10()) }
            "LOG2" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.log2()) }
            "SIN" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.sin()) }
            "COS" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.cos()) }
            "TAN" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.tan()) }
            "ASIN" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.asin()) }
            "ACOS" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.acos()) }
            "ATAN" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.atan()) }
            "SINH" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.sinh()) }
            "COSH" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.cosh()) }
            "TANH" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.tanh()) }
            "FLOOR" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.floor()) }
            "CEIL" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.ceil()) }
            "TRUNC" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.trunc()) }
            "SIGN" => {
                self.require_args(name, args, 1)?;
                let x = self.evaluate_expr(&args[0], results)?;
                Ok(if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 })
            }
            "FACT" => {
                self.require_args(name, args, 1)?;
                let n = self.evaluate_expr(&args[0], results)? as u64;
                Ok((1..=n).product::<u64>() as f64)
            }
            "DEGREES" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.to_degrees()) }
            "RADIANS" => { self.require_args(name, args, 1)?; Ok(self.evaluate_expr(&args[0], results)?.to_radians()) }

            // === Two-arg math functions ===
            "POW" | "POWER" => {
                self.require_args(name, args, 2)?;
                let x = self.evaluate_expr(&args[0], results)?;
                let y = self.evaluate_expr(&args[1], results)?;
                Ok(x.powf(y))
            }
            "MOD" => {
                self.require_args(name, args, 2)?;
                let x = self.evaluate_expr(&args[0], results)?;
                let y = self.evaluate_expr(&args[1], results)?;
                Ok(x % y)
            }
            "LOG" => {
                self.require_args(name, args, 2)?;
                let x = self.evaluate_expr(&args[0], results)?;
                let base = self.evaluate_expr(&args[1], results)?;
                Ok(x.log(base))
            }
            "ATAN2" => {
                self.require_args(name, args, 2)?;
                let y = self.evaluate_expr(&args[0], results)?;
                let x = self.evaluate_expr(&args[1], results)?;
                Ok(y.atan2(x))
            }
            "ROUND" => {
                self.require_args(name, args, 2)?;
                let x = self.evaluate_expr(&args[0], results)?;
                let digits = self.evaluate_expr(&args[1], results)? as i32;
                let factor = 10f64.powi(digits);
                Ok((x * factor).round() / factor)
            }
            "COMBIN" => {
                self.require_args(name, args, 2)?;
                let n = self.evaluate_expr(&args[0], results)? as u64;
                let k = self.evaluate_expr(&args[1], results)? as u64;
                if k > n { return Ok(0.0); }
                let k = k.min(n - k);
                Ok((0..k).fold(1u64, |acc, i| acc * (n - i) / (i + 1)) as f64)
            }
            "PERMUT" => {
                self.require_args(name, args, 2)?;
                let n = self.evaluate_expr(&args[0], results)? as u64;
                let k = self.evaluate_expr(&args[1], results)? as u64;
                if k > n { return Ok(0.0); }
                Ok(((n - k + 1)..=n).product::<u64>() as f64)
            }
            "GCD" => {
                self.require_args(name, args, 2)?;
                let mut a = self.evaluate_expr(&args[0], results)?.abs() as u64;
                let mut b = self.evaluate_expr(&args[1], results)?.abs() as u64;
                while b != 0 {
                    let t = b;
                    b = a % b;
                    a = t;
                }
                Ok(a as f64)
            }
            "LCM" => {
                self.require_args(name, args, 2)?;
                let a = self.evaluate_expr(&args[0], results)?.abs() as u64;
                let b = self.evaluate_expr(&args[1], results)?.abs() as u64;
                if a == 0 || b == 0 { return Ok(0.0); }
                let mut x = a;
                let mut y = b;
                while y != 0 {
                    let t = y;
                    y = x % y;
                    x = t;
                }
                Ok((a / x * b) as f64)
            }

            // === Constants ===
            "PI" => {
                self.require_args(name, args, 0)?;
                Ok(std::f64::consts::PI)
            }
            "E" => {
                self.require_args(name, args, 0)?;
                Ok(std::f64::consts::E)
            }
            "RAND" => {
                self.require_args(name, args, 0)?;
                Ok(rand::thread_rng().gen())
            }

            _ => Err(CalcError::EvalError(format!("Unknown function: {}", name)))
        }
    }

    fn require_args(&self, name: &str, args: &[Expr], expected: usize) -> Result<(), CalcError> {
        if args.len() != expected {
            Err(CalcError::EvalError(format!("{} requires {} argument(s), got {}", name, expected, args.len())))
        } else {
            Ok(())
        }
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
}
