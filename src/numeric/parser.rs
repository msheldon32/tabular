//! Formula parser for spreadsheet-like calculations.
//!
//! This module provides a proper lexer and parser for formula expressions like:
//! - Cell references: A1, AA123
//! - Ranges: A1:B10, A:A, 1:5
//! - Function calls: SUM(A1:A10), AVG(B1:B5)
//! - Arithmetic: A1 + B1 * 2
//! - Nested expressions: SUM(A1:A5) + SQRT(B1)
//! - Boolean expressions: TRUE, FALSE, AND, OR, NOT
//! - Ternary: IF(condition, true_value, false_value)

use std::fmt;
use super::calctype::CalcType;
use super::lexer::{Token, Lexer};

/// Abstract Syntax Tree nodes for formulas
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A literal value (int, float, string, or bool)
    Literal(CalcType),
    /// A cell reference (col letters, row number 1-indexed)
    CellRef { col: String, row: usize },
    /// A range between two cell references
    Range { start: Box<Expr>, end: Box<Expr> },
    /// A row range like 1:5
    RowRange { start: usize, end: usize },
    /// A column range like A:C
    ColRange { start: String, end: String },
    /// A function call with arguments
    FnCall { name: String, args: Vec<Expr> },
    /// Binary operation
    BinOp { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    /// Unary negation
    Neg(Box<Expr>),
    /// Logical NOT
    Not(Box<Expr>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Pow => write!(f, "^"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Eq => write!(f, "="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "AND"),
            BinOp::Or => write!(f, "OR"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedChar(char, usize),
    UnexpectedToken { expected: String, found: Token },
    UnexpectedEof,
    InvalidNumber(String),
    InvalidCellRef(String),
    EmptyExpression,
    UnclosedQuote,
}

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedChar(c, pos) => {
                write!(f, "unexpected character '{}' at position {}", c, pos)
            }
            ParseError::UnexpectedToken { expected, found } => {
                write!(f, "expected {}, found {}", expected, found)
            }
            ParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            ParseError::InvalidNumber(s) => write!(f, "invalid number: {}", s),
            ParseError::InvalidCellRef(s) => write!(f, "invalid cell reference: {}", s),
            ParseError::EmptyExpression => write!(f, "empty expression"),
            ParseError::UnclosedQuote => write!(f, "unclosed quotation"),
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse a formula string into an AST
    pub fn parse_formula(input: &str) -> Result<Expr, ParseError> {
        let input = input.trim();

        // Skip leading '=' if present
        let input = input.strip_prefix('=').unwrap_or(input);

        if input.is_empty() {
            return Err(ParseError::EmptyExpression);
        }

        let lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse_expr()
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let tok = self.advance();
        if tok == expected {
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: expected.to_string(),
                found: tok,
            })
        }
    }

    /// Parse an expression (entry point)
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    /// Parse OR operator (lowest precedence)
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;

        loop {
            if !matches!(self.peek(), Token::Or) {
                break;
            }
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse AND operator
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;

        loop {
            if !matches!(self.peek(), Token::And) {
                break;
            }
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::BinOp {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse comparison operators
    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive()?;

        loop {
            let op = match self.peek() {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse additive operators (+ -)
    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse multiplicative operators (* / %)
    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_power()?;

        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_power()?;
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    /// Parse power operator (^) - right associative
    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_unary()?;

        if matches!(self.peek(), Token::Caret) {
            self.advance();
            let right = self.parse_power()?; // Right associative
            Ok(Expr::BinOp {
                op: BinOp::Pow,
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    /// Parse unary operators (-, NOT, !)
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek(), Token::Minus) {
            self.advance();
            let expr = self.parse_unary()?;
            Ok(Expr::Neg(Box::new(expr)))
        } else if matches!(self.peek(), Token::Not) {
            self.advance();
            let expr = self.parse_unary()?;
            Ok(Expr::Not(Box::new(expr)))
        } else {
            self.parse_primary()
        }
    }

    /// Parse primary expressions (numbers, cell refs, function calls, parentheses)
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Literal(CalcType::Int(n)) => {
                self.advance();
                // Check if this is a row range like 1:5
                if matches!(self.peek(), Token::Colon) {
                    if n >= 1 {
                        self.advance();
                        if let Token::Literal(CalcType::Int(end)) = self.peek().clone() {
                            if end >= 1 {
                                self.advance();
                                return Ok(Expr::RowRange {
                                    start: n as usize,
                                    end: end as usize,
                                });
                            }
                        }
                        return Err(ParseError::UnexpectedToken {
                            expected: "row number".to_string(),
                            found: self.peek().clone(),
                        });
                    }
                }
                Ok(Expr::Literal(CalcType::Int(n)))
            }

            Token::Literal(CalcType::Float(n)) => {
                self.advance();
                Ok(Expr::Literal(CalcType::Float(n)))
            }

            Token::Literal(CalcType::Str(s)) => {
                self.advance();
                Ok(Expr::Literal(CalcType::Str(s)))
            }

            Token::Literal(CalcType::Bool(b)) => {
                self.advance();
                Ok(Expr::Literal(CalcType::Bool(b)))
            }

            Token::CellRef { col, row } => {
                self.advance();
                // Check if this is the start of a range
                if matches!(self.peek(), Token::Colon) {
                    self.advance();
                    self.parse_range_end(col, row)
                } else {
                    Ok(Expr::CellRef { col, row })
                }
            }

            Token::Ident(name) => {
                self.advance();
                // Check if this is a function call
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call(name)
                } else if matches!(self.peek(), Token::Colon) {
                    // Could be a column range like A:C
                    self.advance();
                    self.parse_col_range(name)
                } else {
                    // Just an identifier (shouldn't happen in valid formulas)
                    Err(ParseError::InvalidCellRef(name))
                }
            }

            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }

            // AND/OR/NOT can also be function names when followed by (
            Token::And => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call("AND".to_string())
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "( after AND".to_string(),
                        found: self.peek().clone(),
                    })
                }
            }

            Token::Or => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call("OR".to_string())
                } else {
                    Err(ParseError::UnexpectedToken {
                        expected: "( after OR".to_string(),
                        found: self.peek().clone(),
                    })
                }
            }

            Token::Not => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.parse_function_call("NOT".to_string())
                } else {
                    // NOT as unary operator - but we already handle this in parse_unary
                    // This shouldn't be reached normally
                    let expr = self.parse_unary()?;
                    Ok(Expr::Not(Box::new(expr)))
                }
            }

            Token::Eof => Err(ParseError::UnexpectedEof),

            other => Err(ParseError::UnexpectedToken {
                expected: "number, boolean, cell reference, or function".to_string(),
                found: other,
            }),
        }
    }

    /// Parse the end of a range (after seeing "A1:")
    fn parse_range_end(&mut self, start_col: String, start_row: usize) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::CellRef { col, row } => {
                self.advance();
                Ok(Expr::Range {
                    start: Box::new(Expr::CellRef {
                        col: start_col,
                        row: start_row,
                    }),
                    end: Box::new(Expr::CellRef { col, row }),
                })
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "cell reference".to_string(),
                found: self.peek().clone(),
            }),
        }
    }

    /// Parse a column range like A:C (after seeing "A:")
    fn parse_col_range(&mut self, start: String) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Ident(end) => {
                // Verify it's all letters
                if end.chars().all(|c| c.is_ascii_alphabetic()) {
                    self.advance();
                    Ok(Expr::ColRange {
                        start: start.to_uppercase(),
                        end: end.to_uppercase(),
                    })
                } else {
                    Err(ParseError::InvalidCellRef(end))
                }
            }
            _ => Err(ParseError::UnexpectedToken {
                expected: "column letter".to_string(),
                found: self.peek().clone(),
            }),
        }
    }

    /// Parse a function call like SUM(A1:A10)
    fn parse_function_call(&mut self, name: String) -> Result<Expr, ParseError> {
        self.expect(Token::LParen)?;

        let mut args = Vec::new();

        // Handle empty argument list
        if !matches!(self.peek(), Token::RParen) {
            args.push(self.parse_expr()?);

            while matches!(self.peek(), Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }

        self.expect(Token::RParen)?;

        Ok(Expr::FnCall {
            name: name.to_uppercase(),
            args,
        })
    }
}

/// Parse a formula string into an AST
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    Parser::parse_formula(input)
}
