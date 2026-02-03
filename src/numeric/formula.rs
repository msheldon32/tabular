//! Formula function implementations
//!
//! This module contains all spreadsheet function implementations (SUM, AVG, IF, etc.)

use std::collections::HashMap;
use rand::Rng;

use crate::util::{CellRef, CalcError};
use crate::numeric::calculator::CalcType;
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
            let mut vals = evaluator.expand(&args[0], results)?;

            Ok(vals.iter().try_fold(CalcType::Int(0), |acc, v| {
                CalcType::bin_op(BinOp::Add, acc, v.clone())
            })?)
        },
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
