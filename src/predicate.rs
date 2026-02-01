
#[derive(Debug, Clone, Copy)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}


#[derive(Debug, Clone)]
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

                ColumnType::Text => match op {
                    Op::Eq => other == val,
                    Op::Ne => other != val,
                    Op::Lt => other < val,
                    Op::Le => other <= val,
                    Op::Gt => other > val,
                    Op::Ge => other >= val,
                },
            },
        }
    }
}
