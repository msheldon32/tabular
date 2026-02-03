//! Formula function implementations
//!
//! This module contains all spreadsheet function implementations (SUM, AVG, IF, etc.)

use std::collections::HashMap;
use rand::Rng;

use crate::util::{CellRef, CalcError};
use crate::numeric::calculator::CalcType;

/// Trait for types that can evaluate expressions and expand ranges
/// This allows the function evaluator to delegate back to the calculator
pub trait ExprEvaluator {
    fn eval(&self, expr: &super::parser::Expr, results: &HashMap<CellRef, CalcType>) -> Result<f64, CalcError>;
    fn expand(&self, expr: &super::parser::Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<f64>, CalcError>;
}

/// Evaluate a function call
pub fn evaluate_function<E: ExprEvaluator>(
    evaluator: &E,
    name: &str,
    args: &[super::parser::Expr],
    results: &HashMap<CellRef, CalcType>,
) -> Result<f64, CalcError> {
    match name {
        // === Aggregate functions ===
        "SUM" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.iter().sum())
        }
        "AVG" | "AVERAGE" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { Ok(0.0) } else { Ok(vals.iter().sum::<f64>() / vals.len() as f64) }
        }
        "MIN" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.iter().cloned().fold(f64::INFINITY, f64::min))
        }
        "MAX" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        }
        "COUNT" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.len() as f64)
        }
        "PRODUCT" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.iter().product())
        }
        "MEDIAN" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
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
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
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
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.len() < 2 { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            Ok(variance.sqrt())
        }
        "STDEVP" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64;
            Ok(variance.sqrt())
        }
        "VAR" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.len() < 2 { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            Ok(vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64)
        }
        "VARP" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            Ok(vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / vals.len() as f64)
        }
        "GEOMEAN" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(f64::NAN); }
            let product: f64 = vals.iter().product();
            if product < 0.0 { return Ok(f64::NAN); }
            Ok(product.powf(1.0 / vals.len() as f64))
        }
        "HARMEAN" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(f64::NAN); }
            let sum_recip: f64 = vals.iter().map(|x| 1.0 / x).sum();
            Ok(vals.len() as f64 / sum_recip)
        }
        "SUMSQ" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            Ok(vals.iter().map(|x| x * x).sum())
        }
        "AVEDEV" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            Ok(vals.iter().map(|x| (x - mean).abs()).sum::<f64>() / vals.len() as f64)
        }
        "DEVSQ" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            if vals.is_empty() { return Ok(0.0); }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            Ok(vals.iter().map(|x| (x - mean).powi(2)).sum())
        }
        "KURT" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
            let n = vals.len() as f64;
            if n < 4.0 { return Ok(f64::NAN); }
            let mean = vals.iter().sum::<f64>() / n;
            let m2 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
            let m4 = vals.iter().map(|x| (x - mean).powi(4)).sum::<f64>() / n;
            let g2 = m4 / (m2 * m2) - 3.0;
            Ok(((n + 1.0) * g2 + 6.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0)))
        }
        "SKEW" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;
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
            require_args(name, args, 2)?;
            let vals1 = evaluator.expand(&args[0], results)?;
            let vals2 = evaluator.expand(&args[1], results)?;
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
            require_args(name, args, 2)?;
            let vals1 = evaluator.expand(&args[0], results)?;
            let vals2 = evaluator.expand(&args[1], results)?;
            if vals1.len() != vals2.len() || vals1.is_empty() { return Ok(f64::NAN); }
            let n = vals1.len() as f64;
            let mean1 = vals1.iter().sum::<f64>() / n;
            let mean2 = vals2.iter().sum::<f64>() / n;
            Ok(vals1.iter().zip(vals2.iter())
                .map(|(a, b)| (a - mean1) * (b - mean2))
                .sum::<f64>() / n)
        }
        "PERCENTILE" => {
            require_args(name, args, 2)?;
            let vals = evaluator.expand(&args[0], results)?;
            let k = evaluator.eval(&args[1], results)?;
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
            require_args(name, args, 2)?;
            let vals = evaluator.expand(&args[0], results)?;
            let q = evaluator.eval(&args[1], results)? as i64;
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
        "ABS" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.abs()) }
        "SQRT" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.sqrt()) }
        "EXP" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.exp()) }
        "LN" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.ln()) }
        "LOG10" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.log10()) }
        "LOG2" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.log2()) }
        "SIN" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.sin()) }
        "COS" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.cos()) }
        "TAN" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.tan()) }
        "ASIN" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.asin()) }
        "ACOS" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.acos()) }
        "ATAN" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.atan()) }
        "SINH" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.sinh()) }
        "COSH" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.cosh()) }
        "TANH" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.tanh()) }
        "FLOOR" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.floor()) }
        "CEIL" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.ceil()) }
        "TRUNC" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.trunc()) }
        "SIGN" => {
            require_args(name, args, 1)?;
            let x = evaluator.eval(&args[0], results)?;
            Ok(if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 })
        }
        "FACT" => {
            require_args(name, args, 1)?;
            let n = evaluator.eval(&args[0], results)? as u64;
            Ok((1..=n).product::<u64>() as f64)
        }
        "DEGREES" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.to_degrees()) }
        "RADIANS" => { require_args(name, args, 1)?; Ok(evaluator.eval(&args[0], results)?.to_radians()) }

        // === Two-arg math functions ===
        "POW" | "POWER" => {
            require_args(name, args, 2)?;
            let x = evaluator.eval(&args[0], results)?;
            let y = evaluator.eval(&args[1], results)?;
            Ok(x.powf(y))
        }
        "MOD" => {
            require_args(name, args, 2)?;
            let x = evaluator.eval(&args[0], results)?;
            let y = evaluator.eval(&args[1], results)?;
            Ok(x % y)
        }
        "LOG" => {
            require_args(name, args, 2)?;
            let x = evaluator.eval(&args[0], results)?;
            let base = evaluator.eval(&args[1], results)?;
            Ok(x.log(base))
        }
        "ATAN2" => {
            require_args(name, args, 2)?;
            let y = evaluator.eval(&args[0], results)?;
            let x = evaluator.eval(&args[1], results)?;
            Ok(y.atan2(x))
        }
        "ROUND" => {
            require_args(name, args, 2)?;
            let x = evaluator.eval(&args[0], results)?;
            let digits = evaluator.eval(&args[1], results)? as i32;
            let factor = 10f64.powi(digits);
            Ok((x * factor).round() / factor)
        }
        "COMBIN" => {
            require_args(name, args, 2)?;
            let n = evaluator.eval(&args[0], results)? as u64;
            let k = evaluator.eval(&args[1], results)? as u64;
            if k > n { return Ok(0.0); }
            let k = k.min(n - k);
            Ok((0..k).fold(1u64, |acc, i| acc * (n - i) / (i + 1)) as f64)
        }
        "PERMUT" => {
            require_args(name, args, 2)?;
            let n = evaluator.eval(&args[0], results)? as u64;
            let k = evaluator.eval(&args[1], results)? as u64;
            if k > n { return Ok(0.0); }
            Ok(((n - k + 1)..=n).product::<u64>() as f64)
        }
        "GCD" => {
            require_args(name, args, 2)?;
            let mut a = evaluator.eval(&args[0], results)?.abs() as u64;
            let mut b = evaluator.eval(&args[1], results)?.abs() as u64;
            while b != 0 {
                let t = b;
                b = a % b;
                a = t;
            }
            Ok(a as f64)
        }
        "LCM" => {
            require_args(name, args, 2)?;
            let a = evaluator.eval(&args[0], results)?.abs() as u64;
            let b = evaluator.eval(&args[1], results)?.abs() as u64;
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
            require_args(name, args, 0)?;
            Ok(std::f64::consts::PI)
        }
        "E" => {
            require_args(name, args, 0)?;
            Ok(std::f64::consts::E)
        }
        "RAND" => {
            require_args(name, args, 0)?;
            Ok(rand::thread_rng().gen())
        }

        // === Boolean/Logical functions ===
        "IF" => {
            require_args(name, args, 3)?;
            let condition = evaluator.eval(&args[0], results)?;
            // Non-zero is true
            if condition != 0.0 {
                evaluator.eval(&args[1], results)
            } else {
                evaluator.eval(&args[2], results)
            }
        }
        "AND" => {
            if args.is_empty() {
                return Err(CalcError::EvalError("AND requires at least 1 argument".to_string()));
            }
            for arg in args {
                let val = evaluator.eval(arg, results)?;
                if val == 0.0 {
                    return Ok(0.0); // Short-circuit on first false
                }
            }
            Ok(1.0)
        }
        "OR" => {
            if args.is_empty() {
                return Err(CalcError::EvalError("OR requires at least 1 argument".to_string()));
            }
            for arg in args {
                let val = evaluator.eval(arg, results)?;
                if val != 0.0 {
                    return Ok(1.0); // Short-circuit on first true
                }
            }
            Ok(0.0)
        }
        "NOT" => {
            require_args(name, args, 1)?;
            let val = evaluator.eval(&args[0], results)?;
            Ok(if val == 0.0 { 1.0 } else { 0.0 })
        }
        "TRUE" => {
            require_args(name, args, 0)?;
            Ok(1.0)
        }
        "FALSE" => {
            require_args(name, args, 0)?;
            Ok(0.0)
        }
        "IFERROR" => {
            require_args(name, args, 2)?;
            match evaluator.eval(&args[0], results) {
                Ok(val) if !val.is_nan() && !val.is_infinite() => Ok(val),
                _ => evaluator.eval(&args[1], results),
            }
        }

        _ => Err(CalcError::EvalError(format!("Unknown function: {}", name)))
    }
}

fn require_args(name: &str, args: &[super::parser::Expr], expected: usize) -> Result<(), CalcError> {
    if args.len() != expected {
        Err(CalcError::EvalError(format!("{} requires {} argument(s), got {}", name, expected, args.len())))
    } else {
        Ok(())
    }
}
