use std::collections::{HashMap, HashSet};
use regex::Regex;
use rand::Rng;

use crate::table::Table;
use crate::util::{CellRef, parse_cell_ref, parse_range, parse_row_range, parse_col_range, CalcError};

/// Find a function call in the expression, returning (start, end, arguments)
/// This properly handles nested parentheses.
fn find_function_call(expr: &str, func_name: &str) -> Option<(usize, usize, Vec<String>)> {
    let upper = expr.to_uppercase();
    let pattern = format!("{}(", func_name.to_uppercase());

    let start = upper.find(&pattern)?;
    let paren_start = start + func_name.len();

    // Find matching closing parenthesis
    // Start AFTER the opening paren, so we don't count it in depth
    let mut depth = 0;
    let mut end = None;
    let chars: Vec<char> = expr.chars().collect();

    for (i, &ch) in chars.iter().enumerate().skip(paren_start + 1) {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    end = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let end = end?;
    let args_str = &expr[paren_start + 1..end];

    // Split arguments by comma, but respect nested parentheses
    let args = split_args(args_str);

    Some((start, end + 1, args))
}

/// Split function arguments by comma, respecting nested parentheses
fn split_args(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in args_str.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }

    args
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
    skip_header: bool, // whether the header should be skipped in column aggregations
}

impl<'a> Calculator<'a> {
    pub fn new(table: &'a Table, skip_header: bool) -> Self {
        Self { table: table, skip_header: skip_header }
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
            let refs = self.extract_cell_refs(formula, self.table.row_count(), self.table.col_count())?;
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
            } else if value.is_nan() {
                "NaN".to_string()
            } else if value.is_infinite() {
                if value.is_sign_positive() { "Inf" } else { "-Inf" }.to_string()
            } else {
                format!("{:.10}", value).trim_end_matches('0').trim_end_matches('.').to_string()
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
    fn extract_cell_refs(&self, formula: &str, row_count: usize, col_count: usize) -> Result<HashSet<CellRef>, CalcError> {
        let mut refs = HashSet::new();
        let upper = formula.to_uppercase();

        // Find ranges first (e.g., A1:B10)
        let range_re = Regex::new(r"[A-Z]+\d+:[A-Z]+\d+").unwrap();
        for cap in range_re.find_iter(&upper) {
            for cell_ref in parse_range(cap.as_str(), row_count, col_count, self.skip_header)? {
                refs.insert(cell_ref);
            }
        }

        // Row ranges
        let row_range_re = Regex::new(r"\d+:\d+").unwrap();
        for cap in row_range_re.find_iter(&upper) {
            for cell_ref in parse_row_range(cap.as_str(), col_count)? {
                refs.insert(cell_ref);
            }
        }

        // Col ranges
        let col_range_re = Regex::new(r"[A-Z]+:[A-Z]+").unwrap();
        for cap in col_range_re.find_iter(&upper) {
            for cell_ref in parse_col_range(cap.as_str(), row_count, self.skip_header)? {
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
        let refs = parse_range(range, self.table.row_count(), self.table.col_count(), self.skip_header)?;
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

    /// Helper to apply a single-argument function
    fn apply_func_single(&self, result: &mut String, func_name: &str, func: fn(f64) -> f64, results: &HashMap<CellRef, f64>) -> Result<(), CalcError> {
        while let Some((start, end, args)) = find_function_call(result, func_name) {
            if args.len() != 1 {
                return Err(CalcError::EvalError(format!("{} requires 1 argument", func_name)));
            }

            // Recursively expand any nested functions in the argument
            let arg_expanded = self.expand_functions(&args[0], results)?;
            let arg_expr = self.substitute_cell_refs(&arg_expanded, results)?;
            let arg_val = evalexpr::eval(&arg_expr)
                .map_err(|e| CalcError::EvalError(e.to_string()))?
                .as_float()
                .or_else(|_| evalexpr::eval(&arg_expr).unwrap().as_int().map(|i| i as f64))
                .map_err(|_| CalcError::EvalError("Invalid argument".to_string()))?;

            let output = func(arg_val);
            *result = format!(
                "{}{}{}",
                &result[..start],
                output,
                &result[end..]
            );
        }
        Ok(())
    }

    /// Helper to apply a two-argument function
    fn apply_func_double(&self, result: &mut String, func_name: &str, func: fn(f64, f64) -> f64, results: &HashMap<CellRef, f64>) -> Result<(), CalcError> {
        while let Some((start, end, args)) = find_function_call(result, func_name) {
            if args.len() != 2 {
                return Err(CalcError::EvalError(format!("{} requires 2 arguments", func_name)));
            }

            // Recursively expand any nested functions in arguments
            let arg1_expanded = self.expand_functions(&args[0], results)?;
            let arg2_expanded = self.expand_functions(&args[1], results)?;

            let arg1_expr = self.substitute_cell_refs(&arg1_expanded, results)?;
            let arg2_expr = self.substitute_cell_refs(&arg2_expanded, results)?;

            let arg1 = evalexpr::eval(&arg1_expr)
                .map_err(|e| CalcError::EvalError(e.to_string()))?
                .as_float()
                .or_else(|_| evalexpr::eval(&arg1_expr).unwrap().as_int().map(|i| i as f64))
                .map_err(|_| CalcError::EvalError("Invalid argument".to_string()))?;
            let arg2 = evalexpr::eval(&arg2_expr)
                .map_err(|e| CalcError::EvalError(e.to_string()))?
                .as_float()
                .or_else(|_| evalexpr::eval(&arg2_expr).unwrap().as_int().map(|i| i as f64))
                .map_err(|_| CalcError::EvalError("Invalid argument".to_string()))?;

            let output = func(arg1, arg2);
            *result = format!(
                "{}{}{}",
                &result[..start],
                output,
                &result[end..]
            );
        }
        Ok(())
    }

    /// Helper to apply an aggregate function on a range
    fn apply_aggregate(&self, result: &mut String, func_name: &str, func: fn(&[f64]) -> f64, results: &HashMap<CellRef, f64>) -> Result<(), CalcError> {
        while let Some((start, end, args)) = find_function_call(result, func_name) {
            if args.len() != 1 {
                return Err(CalcError::EvalError(format!("{} requires 1 argument (a range)", func_name)));
            }

            let range = &args[0];
            let values = self.get_range_values(range, results)?;
            let output = func(&values);
            *result = format!(
                "{}{}{}",
                &result[..start],
                output,
                &result[end..]
            );
        }
        Ok(())
    }

    /// Expand function calls like sum(A1:A10) to their values
    fn expand_functions(&self, formula: &str, results: &HashMap<CellRef, f64>) -> Result<String, CalcError> {
        let mut result = formula.to_string();

        // === Aggregate functions (take ranges) ===

        // SUM
        self.apply_aggregate(&mut result, "SUM", |vals| vals.iter().sum(), results)?;

        // AVG / AVERAGE
        self.apply_aggregate(&mut result, "AVG", |vals| {
            if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
        }, results)?;
        self.apply_aggregate(&mut result, "AVERAGE", |vals| {
            if vals.is_empty() { 0.0 } else { vals.iter().sum::<f64>() / vals.len() as f64 }
        }, results)?;

        // MIN
        self.apply_aggregate(&mut result, "MIN", |vals| {
            vals.iter().cloned().fold(f64::INFINITY, f64::min)
        }, results)?;

        // MAX
        self.apply_aggregate(&mut result, "MAX", |vals| {
            vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        }, results)?;

        // COUNT
        self.apply_aggregate(&mut result, "COUNT", |vals| vals.len() as f64, results)?;

        // PRODUCT
        self.apply_aggregate(&mut result, "PRODUCT", |vals| {
            vals.iter().product()
        }, results)?;

        // MEDIAN
        self.apply_aggregate(&mut result, "MEDIAN", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let mut sorted = vals.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 0 {
                (sorted[mid - 1] + sorted[mid]) / 2.0
            } else {
                sorted[mid]
            }
        }, results)?;

        // MODE (returns first mode if multiple)
        self.apply_aggregate(&mut result, "MODE", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let mut counts: HashMap<i64, usize> = HashMap::new();
            for &v in vals {
                let key = (v * 1e10) as i64; // Handle floats approximately
                *counts.entry(key).or_insert(0) += 1;
            }
            let (mode_key, _) = counts.into_iter().max_by_key(|&(_, c)| c).unwrap();
            mode_key as f64 / 1e10
        }, results)?;

        // STDEV (sample standard deviation)
        self.apply_aggregate(&mut result, "STDEV", |vals| {
            if vals.len() < 2 { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            variance.sqrt()
        }, results)?;

        // STDEVP (population standard deviation)
        self.apply_aggregate(&mut result, "STDEVP", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64;
            variance.sqrt()
        }, results)?;

        // VAR (sample variance)
        self.apply_aggregate(&mut result, "VAR", |vals| {
            if vals.len() < 2 { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64
        }, results)?;

        // VARP (population variance)
        self.apply_aggregate(&mut result, "VARP", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64
        }, results)?;

        // GEOMEAN (geometric mean)
        self.apply_aggregate(&mut result, "GEOMEAN", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let product: f64 = vals.iter().product();
            if product < 0.0 { return f64::NAN; }
            product.powf(1.0 / vals.len() as f64)
        }, results)?;

        // HARMEAN (harmonic mean)
        self.apply_aggregate(&mut result, "HARMEAN", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let sum_recip: f64 = vals.iter().map(|x| 1.0 / x).sum();
            vals.len() as f64 / sum_recip
        }, results)?;

        // SUMSQ (sum of squares)
        self.apply_aggregate(&mut result, "SUMSQ", |vals| {
            vals.iter().map(|x| x * x).sum()
        }, results)?;

        // AVEDEV (average absolute deviation)
        self.apply_aggregate(&mut result, "AVEDEV", |vals| {
            if vals.is_empty() { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|x| (x - mean).abs()).sum::<f64>() / vals.len() as f64
        }, results)?;

        // DEVSQ (sum of squared deviations from mean)
        self.apply_aggregate(&mut result, "DEVSQ", |vals| {
            if vals.is_empty() { return 0.0; }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            vals.iter().map(|x| (x - mean).powi(2)).sum()
        }, results)?;

        // KURT (kurtosis)
        self.apply_aggregate(&mut result, "KURT", |vals| {
            let n = vals.len() as f64;
            if n < 4.0 { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / n;
            let m2 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            let m4 = vals.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n;
            let g2 = m4 / (m2 * m2) - 3.0;
            // Excess kurtosis with sample correction
            ((n + 1.0) * g2 + 6.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0))
        }, results)?;

        // SKEW (skewness)
        self.apply_aggregate(&mut result, "SKEW", |vals| {
            let n = vals.len() as f64;
            if n < 3.0 { return f64::NAN; }
            let mean = vals.iter().sum::<f64>() / n;
            let m2 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            let m3 = vals.iter().map(|x| (x - mean).powi(3)).sum::<f64>() / n;
            let g1 = m3 / m2.powf(1.5);
            // Sample skewness correction
            (n * (n - 1.0)).sqrt() / (n - 2.0) * g1
        }, results)?;

        // === Two-range functions ===

        // CORREL (Pearson correlation)
        while let Some((start, end, args)) = find_function_call(&result, "CORREL") {
            if args.len() != 2 {
                return Err(CalcError::EvalError("CORREL requires 2 arguments".to_string()));
            }
            let vals1 = self.get_range_values(&args[0], results)?;
            let vals2 = self.get_range_values(&args[1], results)?;

            let corr = if vals1.len() != vals2.len() || vals1.is_empty() {
                f64::NAN
            } else {
                let n = vals1.len() as f64;
                let mean1 = vals1.iter().sum::<f64>() / n;
                let mean2 = vals2.iter().sum::<f64>() / n;
                let cov: f64 = vals1.iter().zip(vals2.iter())
                    .map(|(a, b)| (a - mean1) * (b - mean2))
                    .sum();
                let std1 = vals1.iter().map(|x| (x - mean1).powi(2)).sum::<f64>().sqrt();
                let std2 = vals2.iter().map(|x| (x - mean2).powi(2)).sum::<f64>().sqrt();
                if std1 == 0.0 || std2 == 0.0 { f64::NAN } else { cov / (std1 * std2) }
            };

            result = format!("{}{}{}", &result[..start], corr, &result[end..]);
        }

        // COVAR (covariance - population)
        while let Some((start, end, args)) = find_function_call(&result, "COVAR") {
            if args.len() != 2 {
                return Err(CalcError::EvalError("COVAR requires 2 arguments".to_string()));
            }
            let vals1 = self.get_range_values(&args[0], results)?;
            let vals2 = self.get_range_values(&args[1], results)?;

            let cov = if vals1.len() != vals2.len() || vals1.is_empty() {
                f64::NAN
            } else {
                let n = vals1.len() as f64;
                let mean1 = vals1.iter().sum::<f64>() / n;
                let mean2 = vals2.iter().sum::<f64>() / n;
                vals1.iter().zip(vals2.iter())
                    .map(|(a, b)| (a - mean1) * (b - mean2))
                    .sum::<f64>() / n
            };

            result = format!("{}{}{}", &result[..start], cov, &result[end..]);
        }

        // PERCENTILE
        while let Some((start, end, args)) = find_function_call(&result, "PERCENTILE") {
            if args.len() != 2 {
                return Err(CalcError::EvalError("PERCENTILE requires 2 arguments".to_string()));
            }
            let range = &args[0];
            let k_str = &args[1];

            let vals = self.get_range_values(range, results)?;
            let k_expanded = self.expand_functions(k_str, results)?;
            let k_expr = self.substitute_cell_refs(&k_expanded, results)?;
            let k = evalexpr::eval(&k_expr)
                .map_err(|e| CalcError::EvalError(e.to_string()))?
                .as_float()
                .or_else(|_| evalexpr::eval(&k_expr).unwrap().as_int().map(|i| i as f64))
                .map_err(|_| CalcError::EvalError("Invalid percentile".to_string()))?;

            let pct = if vals.is_empty() || k < 0.0 || k > 1.0 {
                f64::NAN
            } else {
                let mut sorted = vals.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = k * (sorted.len() - 1) as f64;
                let lower = idx.floor() as usize;
                let upper = idx.ceil() as usize;
                if lower == upper {
                    sorted[lower]
                } else {
                    sorted[lower] + (sorted[upper] - sorted[lower]) * (idx - lower as f64)
                }
            };

            result = format!("{}{}{}", &result[..start], pct, &result[end..]);
        }

        // QUARTILE
        while let Some((start, end, args)) = find_function_call(&result, "QUARTILE") {
            if args.len() != 2 {
                return Err(CalcError::EvalError("QUARTILE requires 2 arguments".to_string()));
            }
            let range = &args[0];
            let q_str = &args[1];

            let vals = self.get_range_values(range, results)?;
            let q_expanded = self.expand_functions(q_str, results)?;
            let q_expr = self.substitute_cell_refs(&q_expanded, results)?;
            let q = evalexpr::eval(&q_expr)
                .map_err(|e| CalcError::EvalError(e.to_string()))?
                .as_int()
                .map_err(|_| CalcError::EvalError("Invalid quartile".to_string()))?;

            let pct = if vals.is_empty() || q < 0 || q > 4 {
                f64::NAN
            } else {
                let k = q as f64 * 0.25;
                let mut sorted = vals.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let idx = k * (sorted.len() - 1) as f64;
                let lower = idx.floor() as usize;
                let upper = idx.ceil() as usize;
                if lower == upper {
                    sorted[lower]
                } else {
                    sorted[lower] + (sorted[upper] - sorted[lower]) * (idx - lower as f64)
                }
            };

            result = format!("{}{}{}", &result[..start], pct, &result[end..]);
        }

        // === Math functions (single argument) ===

        self.apply_func_single(&mut result, "ABS", |x| x.abs(), results)?;
        self.apply_func_single(&mut result, "SQRT", |x| x.sqrt(), results)?;
        self.apply_func_single(&mut result, "EXP", |x| x.exp(), results)?;
        self.apply_func_single(&mut result, "LN", |x| x.ln(), results)?;
        self.apply_func_single(&mut result, "LOG10", |x| x.log10(), results)?;
        self.apply_func_single(&mut result, "LOG2", |x| x.log2(), results)?;
        self.apply_func_single(&mut result, "SIN", |x| x.sin(), results)?;
        self.apply_func_single(&mut result, "COS", |x| x.cos(), results)?;
        self.apply_func_single(&mut result, "TAN", |x| x.tan(), results)?;
        self.apply_func_single(&mut result, "ASIN", |x| x.asin(), results)?;
        self.apply_func_single(&mut result, "ACOS", |x| x.acos(), results)?;
        self.apply_func_single(&mut result, "ATAN", |x| x.atan(), results)?;
        self.apply_func_single(&mut result, "SINH", |x| x.sinh(), results)?;
        self.apply_func_single(&mut result, "COSH", |x| x.cosh(), results)?;
        self.apply_func_single(&mut result, "TANH", |x| x.tanh(), results)?;
        self.apply_func_single(&mut result, "FLOOR", |x| x.floor(), results)?;
        self.apply_func_single(&mut result, "CEIL", |x| x.ceil(), results)?;
        self.apply_func_single(&mut result, "TRUNC", |x| x.trunc(), results)?;
        self.apply_func_single(&mut result, "SIGN", |x| {
            if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
        }, results)?;
        self.apply_func_single(&mut result, "FACT", |x| {
            let n = x as u64;
            (1..=n).product::<u64>() as f64
        }, results)?;
        self.apply_func_single(&mut result, "DEGREES", |x| x.to_degrees(), results)?;
        self.apply_func_single(&mut result, "RADIANS", |x| x.to_radians(), results)?;

        // === Math functions (two arguments) ===

        self.apply_func_double(&mut result, "POW", |x, y| x.powf(y), results)?;
        self.apply_func_double(&mut result, "POWER", |x, y| x.powf(y), results)?;
        self.apply_func_double(&mut result, "MOD", |x, y| x % y, results)?;
        self.apply_func_double(&mut result, "LOG", |x, base| x.log(base), results)?;
        self.apply_func_double(&mut result, "ATAN2", |y, x| y.atan2(x), results)?;
        self.apply_func_double(&mut result, "ROUND", |x, digits| {
            let factor = 10f64.powi(digits as i32);
            (x * factor).round() / factor
        }, results)?;
        self.apply_func_double(&mut result, "COMBIN", |n, k| {
            // n choose k
            let n = n as u64;
            let k = k as u64;
            if k > n { return 0.0; }
            let k = k.min(n - k);
            (0..k).fold(1u64, |acc, i| acc * (n - i) / (i + 1)) as f64
        }, results)?;
        self.apply_func_double(&mut result, "PERMUT", |n, k| {
            // n permute k = n! / (n-k)!
            let n = n as u64;
            let k = k as u64;
            if k > n { return 0.0; }
            ((n - k + 1)..=n).product::<u64>() as f64
        }, results)?;
        self.apply_func_double(&mut result, "GCD", |a, b| {
            let mut a = a.abs() as u64;
            let mut b = b.abs() as u64;
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            a as f64
        }, results)?;
        self.apply_func_double(&mut result, "LCM", |a, b| {
            let a = a.abs() as u64;
            let b = b.abs() as u64;
            if a == 0 || b == 0 { return 0.0; }
            let mut x = a;
            let mut y = b;
            while y != 0 {
                let t = y;
                y = x % y;
                x = t;
            }
            (a / x * b) as f64
        }, results)?;

        // === Constants ===
        let pi_re = Regex::new(r"(?i)\bPI\(\)").unwrap();
        result = pi_re.replace_all(&result, std::f64::consts::PI.to_string().as_str()).to_string();

        let e_re = Regex::new(r"(?i)\bE\(\)").unwrap();
        result = e_re.replace_all(&result, std::f64::consts::E.to_string().as_str()).to_string();

        // RAND() - returns uniform random number in [0, 1)
        let rand_re = Regex::new(r"(?i)\bRAND\(\)").unwrap();
        let mut rng = rand::thread_rng();
        while rand_re.is_match(&result) {
            let rand_val: f64 = rng.gen();
            result = rand_re.replace(&result, rand_val.to_string().as_str()).to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(data: Vec<Vec<&str>>) -> Table {
        Table {
            cells: data.into_iter()
                .map(|row| row.into_iter().map(|s| s.to_string()).collect())
                .collect(),
        }
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
        // Sample std dev of [2,4,4,4,5,5,7,9] = 2.138...
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
        // Perfect positive correlation
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
            vec!["=combin(5,2)"],  // 5 choose 2 = 10
            vec!["=permut(5,2)"],  // 5 permute 2 = 20
            vec!["=fact(5)"],      // 5! = 120
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
            vec!["=gcd(12,18)"],  // 6
            vec!["=lcm(12,18)"],  // 36
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
            vec!["=percentile(A1:E1,0.5)"],  // median = 3
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results[0].2, "3");
    }

    #[test]
    fn test_nested_functions() {
        let table = make_table(vec![
            vec!["=sqrt(abs(-16))"],          // sqrt(16) = 4
            vec!["=pow(sqrt(4),2)"],          // pow(2,2) = 4
            vec!["=abs(floor(-3.7))"],        // abs(-4) = 4
            vec!["=round(sqrt(2),2)"],        // round(1.414..., 2) = 1.41
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
            vec!["=sqrt(sqrt(sqrt(256)))"],   // sqrt(sqrt(sqrt(256))) = sqrt(sqrt(16)) = sqrt(4) = 2
            vec!["=abs(abs(abs(-5)))"],       // 5
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        assert_eq!(results.iter().find(|r| r.0 == 0).unwrap().2, "2");
        assert_eq!(results.iter().find(|r| r.0 == 1).unwrap().2, "5");
    }

    #[test]
    fn test_column_range_sum() {
        // Test SUM(A:A), SUM(B:B), etc.
        // Put formulas in column D to avoid circular refs
        let table = make_table(vec![
            vec!["1", "10", "100", "=sum(A:A)"],
            vec!["2", "20", "200", "=sum(B:B)"],
            vec!["3", "30", "300", "=sum(C:C)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        // A:A = 1+2+3 = 6, B:B = 10+20+30 = 60, C:C = 100+200+300 = 600
        assert_eq!(results.iter().find(|r| r.0 == 0 && r.1 == 3).unwrap().2, "6");
        assert_eq!(results.iter().find(|r| r.0 == 1 && r.1 == 3).unwrap().2, "60");
        assert_eq!(results.iter().find(|r| r.0 == 2 && r.1 == 3).unwrap().2, "600");
    }

    #[test]
    fn test_row_range_sum_1indexed() {
        // Test SUM(1:1), SUM(2:2), etc. - should be 1-indexed
        // Put formulas in row 4 to avoid circular refs
        let table = make_table(vec![
            vec!["1", "2", "3"],       // Row 1 (index 0)
            vec!["10", "20", "30"],    // Row 2 (index 1)
            vec!["100", "200", "300"], // Row 3 (index 2)
            vec!["=sum(1:1)", "=sum(2:2)", "=sum(3:3)"],  // Row 4 (index 3)
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        // Row 1 = 1+2+3 = 6, Row 2 = 10+20+30 = 60, Row 3 = 100+200+300 = 600
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 0).unwrap().2, "6");
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 1).unwrap().2, "60");
        assert_eq!(results.iter().find(|r| r.0 == 3 && r.1 == 2).unwrap().2, "600");
    }

    #[test]
    fn test_lowercase_column_range() {
        // Test that lowercase column ranges work
        // Put formulas in column C to avoid circular refs
        let table = make_table(vec![
            vec!["1", "10", "=sum(a:a)"],
            vec!["2", "20", "=sum(b:b)"],
        ]);
        let calc = Calculator::new(&table, false);
        let results = calc.evaluate_all().unwrap();
        // A:A = 1+2 = 3, B:B = 10+20 = 30
        assert_eq!(results.iter().find(|r| r.0 == 0 && r.1 == 2).unwrap().2, "3");
        assert_eq!(results.iter().find(|r| r.0 == 1 && r.1 == 2).unwrap().2, "30");
    }
}
