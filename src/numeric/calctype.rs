use std::cmp;

use crate::numeric::parser::BinOp;
use crate::util::CalcError;

#[derive(Debug, Clone, PartialEq)]
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

    pub fn numeric_precedence(l: CalcType, r: CalcType) -> Result<(CalcType, CalcType), CalcError> {
        match (l,r) {
            (CalcType::Int(a), CalcType::Int(b)) => Ok((CalcType::Int(a), CalcType::Int(b))),
            (CalcType::Int(a), CalcType::Float(b)) => Ok((CalcType::Float(a as f64), CalcType::Float(b))),
            (CalcType::Float(a), CalcType::Int(b)) => Ok((CalcType::Float(a), CalcType::Float(b as f64))),
            (CalcType::Float(a), CalcType::Float(b)) => Ok((CalcType::Float(a), CalcType::Float(b))),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn compare(l: CalcType, r: CalcType) -> std::cmp::Ordering {
        match CalcType::numeric_precedence(l, r) {
            Ok((CalcType::Int(a), CalcType::Int(b))) => a.cmp(&b),
            Ok((CalcType::Float(a), CalcType::Float(b))) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
            _default => std::cmp::Ordering::Equal
        }
    }

    pub fn not(x: CalcType) -> Result<CalcType, CalcError> {
        match x {
            CalcType::Bool(b) => Ok(CalcType::Bool(!b)),
            _default => Err(CalcError::EvalError("Boolean operation on non-boolean expressions".to_string()))
        }
    }

    pub fn negate(x: CalcType) -> Result<CalcType, CalcError> {
        match x {
            CalcType::Int(x) => Ok(CalcType::Int(-x)),
            CalcType::Float(x) => Ok(CalcType::Float(-x)),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn abs(x: CalcType) -> Result<CalcType, CalcError> {
        match x {
            CalcType::Int(x) => Ok(CalcType::Int(x.abs())),
            CalcType::Float(x) => Ok(CalcType::Float(x.abs())),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn floor(x: CalcType) -> Result<CalcType, CalcError> {
        match x {
            CalcType::Int(x) => Ok(CalcType::Int(x)),
            CalcType::Float(x) => Ok(CalcType::Float(x.floor())),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn ceil(x: CalcType) -> Result<CalcType, CalcError> {
        match x {
            CalcType::Int(x) => Ok(CalcType::Int(x)),
            CalcType::Float(x) => Ok(CalcType::Float(x.ceil())),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn min(l: CalcType, r: CalcType) -> Result<CalcType, CalcError> {
        match CalcType::numeric_precedence(l,r) {
            Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(cmp::min(a,b))),
            Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a.min(b))),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn max(l: CalcType, r: CalcType) -> Result<CalcType, CalcError> {
        match CalcType::numeric_precedence(l,r) {
            Ok((CalcType::Int(a), CalcType::Int(b))) => Ok(CalcType::Int(cmp::max(a,b))),
            Ok((CalcType::Float(a), CalcType::Float(b))) => Ok(CalcType::Float(a.max(b))),
            _default => Err(CalcError::EvalError("Numeric operation on non-numeric data".to_string()))
        }
    }

    pub fn bin_op(op: BinOp, l: CalcType, r: CalcType) -> Result<CalcType, CalcError> {
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
