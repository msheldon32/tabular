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
                    let lhs: f64 = match other.parse() {
                        Ok(v) => v,
                        Err(_) => return false,
                    };
                    let rhs: f64 = match val.parse() {
                        Ok(v) => v,
                        Err(_) => return false,
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
    let pred_re = Regex::new(r"^\s*(!|=|<|<=|>|>=)\s*(\S+)\s*$")
        .expect("invalid regex");

    let caps = pred_re
        .captures(&pred_string)?;

    let op_str = &caps[1];
    let val = caps[2].to_string();

    let op = match op_str {
        "="  => Op::Eq,
        "!"  => Op::Ne,
        "<"  => Op::Lt,
        "<=" => Op::Le,
        ">"  => Op::Gt,
        ">=" => Op::Ge,
        _ => unreachable!(),
    };

    Some(Predicate::Comparator { op, val })
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::ColumnType;

    // === parse_predicate tests ===

    #[test]
    fn parse_predicate_eq() {
        let pred = parse_predicate("= 42".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Eq, val: "42".to_string() }));
    }

    #[test]
    fn parse_predicate_ne() {
        let pred = parse_predicate("! 10".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Ne, val: "10".to_string() }));
    }

    #[test]
    fn parse_predicate_lt() {
        let pred = parse_predicate("< 5".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Lt, val: "5".to_string() }));
    }

    #[test]
    fn parse_predicate_le() {
        let pred = parse_predicate("<= 100".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Le, val: "100".to_string() }));
    }

    #[test]
    fn parse_predicate_gt() {
        let pred = parse_predicate("> 0".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Gt, val: "0".to_string() }));
    }

    #[test]
    fn parse_predicate_ge() {
        let pred = parse_predicate(">= 50".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Ge, val: "50".to_string() }));
    }

    #[test]
    fn parse_predicate_with_whitespace() {
        let pred = parse_predicate("  >=   123  ".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Ge, val: "123".to_string() }));
    }

    // === parse_predicate tests for text values ===

    #[test]
    fn parse_predicate_text_eq() {
        let pred = parse_predicate("= hello".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Eq, val: "hello".to_string() }));
    }

    #[test]
    fn parse_predicate_text_ne() {
        let pred = parse_predicate("! active".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Ne, val: "active".to_string() }));
    }

    #[test]
    fn parse_predicate_text_lt() {
        let pred = parse_predicate("< zebra".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Lt, val: "zebra".to_string() }));
    }

    #[test]
    fn parse_predicate_text_mixed_case() {
        let pred = parse_predicate("= HelloWorld".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Eq, val: "HelloWorld".to_string() }));
    }

    #[test]
    fn parse_predicate_text_with_numbers() {
        let pred = parse_predicate("= user123".to_string());
        assert_eq!(pred, Some(Predicate::Comparator { op: Op::Eq, val: "user123".to_string() }));
    }

    #[test]
    fn parse_predicate_invalid_returns_none() {
        assert_eq!(parse_predicate("".to_string()), None);
        assert_eq!(parse_predicate("invalid".to_string()), None);
        assert_eq!(parse_predicate("== 5".to_string()), None);
        assert_eq!(parse_predicate("5".to_string()), None);
        assert_eq!(parse_predicate("=".to_string()), None);
        assert_eq!(parse_predicate("= ".to_string()), None);
    }

    // === Predicate::evaluate tests for Numeric ===

    #[test]
    fn evaluate_numeric_eq() {
        let pred = Predicate::Comparator { op: Op::Eq, val: "42".to_string() };
        assert!(pred.evaluate("42", ColumnType::Numeric));
        assert!(pred.evaluate("42.0", ColumnType::Numeric));
        assert!(!pred.evaluate("43", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_ne() {
        let pred = Predicate::Comparator { op: Op::Ne, val: "10".to_string() };
        assert!(pred.evaluate("5", ColumnType::Numeric));
        assert!(!pred.evaluate("10", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_lt() {
        let pred = Predicate::Comparator { op: Op::Lt, val: "10".to_string() };
        assert!(pred.evaluate("5", ColumnType::Numeric));
        assert!(pred.evaluate("9.99", ColumnType::Numeric));
        assert!(!pred.evaluate("10", ColumnType::Numeric));
        assert!(!pred.evaluate("15", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_le() {
        let pred = Predicate::Comparator { op: Op::Le, val: "10".to_string() };
        assert!(pred.evaluate("5", ColumnType::Numeric));
        assert!(pred.evaluate("10", ColumnType::Numeric));
        assert!(!pred.evaluate("11", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_gt() {
        let pred = Predicate::Comparator { op: Op::Gt, val: "10".to_string() };
        assert!(pred.evaluate("15", ColumnType::Numeric));
        assert!(!pred.evaluate("10", ColumnType::Numeric));
        assert!(!pred.evaluate("5", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_ge() {
        let pred = Predicate::Comparator { op: Op::Ge, val: "10".to_string() };
        assert!(pred.evaluate("15", ColumnType::Numeric));
        assert!(pred.evaluate("10", ColumnType::Numeric));
        assert!(!pred.evaluate("9", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_numeric_non_parseable_returns_false() {
        let pred = Predicate::Comparator { op: Op::Eq, val: "10".to_string() };
        assert!(!pred.evaluate("abc", ColumnType::Numeric));
        assert!(!pred.evaluate("", ColumnType::Numeric));
    }

    // === Predicate::evaluate tests for Text ===

    #[test]
    fn evaluate_text_eq_case_insensitive() {
        let pred = Predicate::Comparator { op: Op::Eq, val: "Hello".to_string() };
        assert!(pred.evaluate("hello", ColumnType::Text));
        assert!(pred.evaluate("HELLO", ColumnType::Text));
        assert!(pred.evaluate("  Hello  ", ColumnType::Text));
        assert!(!pred.evaluate("world", ColumnType::Text));
    }

    #[test]
    fn evaluate_text_ne() {
        let pred = Predicate::Comparator { op: Op::Ne, val: "foo".to_string() };
        assert!(pred.evaluate("bar", ColumnType::Text));
        assert!(!pred.evaluate("foo", ColumnType::Text));
        assert!(!pred.evaluate("FOO", ColumnType::Text));
    }

    #[test]
    fn evaluate_text_lt() {
        let pred = Predicate::Comparator { op: Op::Lt, val: "m".to_string() };
        assert!(pred.evaluate("apple", ColumnType::Text));
        assert!(!pred.evaluate("zebra", ColumnType::Text));
    }

    #[test]
    fn evaluate_text_gt() {
        let pred = Predicate::Comparator { op: Op::Gt, val: "m".to_string() };
        assert!(pred.evaluate("zebra", ColumnType::Text));
        assert!(!pred.evaluate("apple", ColumnType::Text));
    }

    // === Compound predicate tests ===

    #[test]
    fn evaluate_not() {
        let inner = Predicate::Comparator { op: Op::Eq, val: "5".to_string() };
        let pred = Predicate::Not(Box::new(inner));
        assert!(pred.evaluate("10", ColumnType::Numeric));
        assert!(!pred.evaluate("5", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_and() {
        let left = Predicate::Comparator { op: Op::Gt, val: "5".to_string() };
        let right = Predicate::Comparator { op: Op::Lt, val: "10".to_string() };
        let pred = Predicate::And(Box::new(left), Box::new(right));
        assert!(pred.evaluate("7", ColumnType::Numeric));
        assert!(!pred.evaluate("3", ColumnType::Numeric));
        assert!(!pred.evaluate("12", ColumnType::Numeric));
    }

    #[test]
    fn evaluate_or() {
        let left = Predicate::Comparator { op: Op::Lt, val: "5".to_string() };
        let right = Predicate::Comparator { op: Op::Gt, val: "10".to_string() };
        let pred = Predicate::Or(Box::new(left), Box::new(right));
        assert!(pred.evaluate("3", ColumnType::Numeric));
        assert!(pred.evaluate("15", ColumnType::Numeric));
        assert!(!pred.evaluate("7", ColumnType::Numeric));
    }

    // === Display tests ===

    #[test]
    fn display_op() {
        assert_eq!(format!("{}", Op::Eq), "=");
        assert_eq!(format!("{}", Op::Ne), "!");
        assert_eq!(format!("{}", Op::Lt), "<");
        assert_eq!(format!("{}", Op::Le), "<=");
        assert_eq!(format!("{}", Op::Gt), ">");
        assert_eq!(format!("{}", Op::Ge), ">=");
    }

    #[test]
    fn display_predicate() {
        let pred = Predicate::Comparator { op: Op::Ge, val: "100".to_string() };
        assert_eq!(format!("{}", pred), ">= 100");

        let not_pred = Predicate::Not(Box::new(pred.clone()));
        assert_eq!(format!("{}", not_pred), "NOT(>= 100)");
    }
}
