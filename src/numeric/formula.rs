//! Formula function implementations
//!
//! This module contains all spreadsheet function implementations (SUM, AVG, IF, etc.)

use std::collections::HashMap;
use rand::Rng;

use crate::util::{CellRef, CalcError};
use crate::numeric::calctype::CalcType;
use crate::numeric::parser::BinOp;

/// Trait for types that can evaluate expressions and expand ranges
/// This allows the function evaluator to delegate back to the calculator
pub trait ExprEvaluator {
    fn eval(&self, expr: &super::parser::Expr, results: &HashMap<CellRef, CalcType>) -> Result<CalcType, CalcError>;
    fn expand(&self, expr: &super::parser::Expr, results: &HashMap<CellRef, CalcType>) -> Result<Vec<CalcType>, CalcError>;
}

/// Evaluate a function call
pub fn evaluate_function<E: ExprEvaluator>(
    evaluator: &E,
    name: &str,
    args: &[super::parser::Expr],
    results: &HashMap<CellRef, CalcType>,
) -> Result<CalcType, CalcError> {
    // === Aggregate Functions ===
    match name {
        "SUM" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            if vals.len() == 0 {
                return Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()));
            }

            Ok(vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::bin_op(BinOp::Add, acc, v.clone())
            })?)
        },
        "AVG" | "AVERAGE" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            if vals.len() == 0 {
                return Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()));
            }

            let sum = vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::bin_op(BinOp::Add, acc, v.clone())
            })?;

            CalcType::bin_op(BinOp::Div, sum, CalcType::Int(vals.len() as i64))
        },
        "MIN" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            if vals.len() == 0 {
                return Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()));
            }

            Ok(vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::min(acc, v.clone())
            })?)
        },
        "MAX" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            if vals.len() == 0 {
                return Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()));
            }

            Ok(vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::max(acc, v.clone())
            })?)
        },
        "COUNT" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            Ok(CalcType::Int(vals.len() as i64))
        },
        "PRODUCT" => {
            require_args(name, args, 1)?;
            let vals = evaluator.expand(&args[0], results)?;

            if vals.len() == 0 {
                return Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()));
            }

            Ok(vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::bin_op(BinOp::Mul, acc, v.clone())
            })?)
        },
        "ABS" => {
            require_args(name, args, 1)?;

            CalcType::abs(evaluator.eval(&args[0], results)?)
        },
        "FLOOR" => {
            require_args(name, args, 1)?;

            CalcType::floor(evaluator.eval(&args[0], results)?)
        },
        "CEIL" => {
            require_args(name, args, 1)?;

            CalcType::ceil(evaluator.eval(&args[0], results)?)
        },
        "RAND" => {
            let mut rng = rand::thread_rng();
            Ok(CalcType::Float(rng.gen()))
        },
        "IF" => {
            require_args(name, args, 3)?;

            let cond = evaluator.eval(&args[0], results)?;

            match cond {
                CalcType::Bool(b) => {
                    if b {
                        evaluator.eval(&args[1], results)
                    } else {
                        evaluator.eval(&args[2], results)
                    }
                },
                _default => Err(CalcError::EvalError("Condition in IF() is not a boolean".to_string()))
            }
        },
        "IFERROR" => {
            require_args(name, args, 2)?;

            let val = evaluator.eval(&args[0], results);

            match val {
                Err(x) => evaluator.eval(&args[1], results),
                _default => _default
            }
        },
        "OR" => {
            require_args(name, args, 2)?;
            let cond1  = evaluator.eval(&args[0], results)?;
            let cond2  = evaluator.eval(&args[1], results)?;

            return CalcType::bin_op(BinOp::Or, cond1, cond2);
        },
        "AND" => {
            require_args(name, args, 2)?;
            let cond1  = evaluator.eval(&args[0], results)?;
            let cond2  = evaluator.eval(&args[1], results)?;

            return CalcType::bin_op(BinOp::And, cond1, cond2);
        },
        "MEDIAN" => {
            require_args(name, args, 1)?;
            let mut vals = evaluator.expand(&args[0], results)?;
            
            vals.sort_by(|a, b| CalcType::compare(a.clone(),b.clone()));
            if vals.len() == 0 {
                Err(CalcError::EvalError("Trying to use an array function on an empty array".to_string()))
            } else if vals.len() % 2 == 0 {
                let midpoint = (vals.len()/2).saturating_sub(1);
                let a = vals[midpoint].clone();
                let b = vals[midpoint+1].clone();
                let s = CalcType::bin_op(BinOp::Add, a, b)?;
                CalcType::bin_op(BinOp::Div, s, CalcType::Int(2))
            } else {
                let midpoint = vals.len()/2;
                Ok(vals[midpoint].clone())
            }
            
        },
        "PI" => Ok(CalcType::Float(std::f64::consts::PI)),
        "E" => Ok(CalcType::Float(std::f64::consts::E)),
        "TRUE" => Ok(CalcType::Bool(true)),
        "FALSE" => Ok(CalcType::Bool(false)),
        // I am just killing this function entirely for now, this will require substantial revision
        _default => Err(CalcError::EvalError("(Most) functions have been removed for now".to_string()))
    }
}

fn require_args(name: &str, args: &[super::parser::Expr], expected: usize) -> Result<(), CalcError> {
    if args.len() != expected {
        Err(CalcError::EvalError(format!("{} requires {} argument(s), got {}", name, expected, args.len())))
    } else {
        Ok(())
    }
}
