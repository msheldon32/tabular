use regex::Regex;
use std::fmt;

use crate::util::ColumnType;


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Op::Eq => "=",
            Op::Ne => "!",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
        };
        write!(f, "{s}")
    }
}


#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(dead_code)]
pub enum Predicate {
    Comparator {
        op: Op,
        val: String,
    },
    Not(Box<Predicate>),
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>)
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Predicate::Comparator { op, val } => {
                write!(f, "{} {}", op, val)
            },
            Predicate::Not(pred) => {
                write!(f, "NOT({})", pred)
            },
            Predicate::And(lhs, rhs) => {
                write!(f, "AND({},{})", lhs, rhs)
            },
            Predicate::Or(lhs, rhs) => {
                write!(f, "OR({},{})", lhs, rhs)
            },
        }
    }
}


impl Predicate {
    pub fn evaluate(&self, other: &str, col_type: ColumnType) -> bool {
        match self {
            Predicate::Comparator { op, val } => match col_type {
                ColumnType::Numeric => {
                    // Use parse_numeric so formatted values ("$5", "1,000", "15%")
                    // filter consistently with how column types are probed
                    let lhs: f64 = match crate::numeric::format::parse_numeric(other) {
                        Some(v) => v,
                        None => return false,
                    };
                    let rhs: f64 = match crate::numeric::format::parse_numeric(val) {
                        Some(v) => v,
                        None => return false,
                    };

                    match op {
                        Op::Eq => lhs == rhs,
                        Op::Ne => lhs != rhs,
                        Op::Lt => lhs < rhs,
                        Op::Le => lhs <= rhs,
                        Op::Gt => lhs > rhs,
                        Op::Ge => lhs >= rhs,
                    }
                }

                ColumnType::Text => {
                    let lhs = other.trim().to_lowercase();
                    let rhs = val.trim().to_lowercase();

                    match op {
                        Op::Eq => lhs == rhs,
                        Op::Ne => lhs != rhs,
                        Op::Lt => lhs < rhs,
                        Op::Le => lhs <= rhs,
                        Op::Gt => lhs > rhs,
                        Op::Ge => lhs >= rhs,
                    }
                },
            },
            Predicate::Not(pred) => {
                !pred.evaluate(other, col_type)
            },
            Predicate::And(lhs, rhs) => {
                lhs.evaluate(other, col_type) && rhs.evaluate(other, col_type)
            },
            Predicate::Or(lhs, rhs) => {
                lhs.evaluate(other, col_type) || rhs.evaluate(other, col_type)
            },
        }
    }
}


pub fn parse_predicate(pred_string: String) -> Option<Predicate> {
    // Multi-char operators must come first: alternation is leftmost-first,
    // so "<" before "<=" would match "<" and corrupt the value with "=".
    // The value may contain spaces (e.g. text filters like `= New York`).
    let pred_re = Regex::new(r"^\s*(<=|>=|!=|==|!|=|<|>)\s*(\S.*?)\s*$")
        .expect("invalid regex");

    let caps = pred_re
        .captures(&pred_string)?;

    let op_str = &caps[1];
    let val = caps[2].to_string();

    let op = match op_str {
        "=" | "==" => Op::Eq,
        "!" | "!=" => Op::Ne,
        "<"  => Op::Lt,
        "<=" => Op::Le,
        ">"  => Op::Gt,
        ">=" => Op::Ge,
        _ => unreachable!(),
    };

    Some(Predicate::Comparator { op, val })
}
