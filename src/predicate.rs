use regex::Regex;


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Predicate {
    Comparator {
        op: Op,
        val: String,
    },
}


#[derive(Debug, Clone, Copy)]
pub enum ColumnType {
    Numeric,
    Text,
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
        }
    }
}


pub fn parse_predicate(pred_string: String) -> Predicate {
    let pred_re = Regex::new(r"^\s*(!|=|<|<=|>|>=)\s*([0-9]+)\s*$")
        .expect("invalid regex");

    let caps = pred_re
        .captures(&pred_string)
        .expect("invalid predicate string");

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

    Predicate::Comparator { op, val }
}
