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

// ============================================================================
// Tokens
// ============================================================================

/// Token types produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A number literal (integer or float)
    Number(f64),
    /// Boolean literal
    True,
    False,
    /// An identifier (function name or could be part of cell ref)
    Ident(String),
    /// A cell reference like A1, AA123
    CellRef { col: String, row: usize },
    /// A colon (used in ranges)
    Colon,
    /// Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Caret,   // ^
    Percent, // % (modulo)
    /// Comparison operators
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Logical operators
    And,  // && or AND
    Or,   // || or OR
    Not,  // ! or NOT
    /// Parentheses
    LParen,
    RParen,
    /// Comma (argument separator)
    Comma,
    /// End of input
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Number(n) => write!(f, "{}", n),
            Token::True => write!(f, "TRUE"),
            Token::False => write!(f, "FALSE"),
            Token::Ident(s) => write!(f, "{}", s),
            Token::CellRef { col, row } => write!(f, "{}{}", col, row),
            Token::Colon => write!(f, ":"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Caret => write!(f, "^"),
            Token::Percent => write!(f, "%"),
            Token::Eq => write!(f, "="),
            Token::Ne => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Le => write!(f, "<="),
            Token::Gt => write!(f, ">"),
            Token::Ge => write!(f, ">="),
            Token::And => write!(f, "AND"),
            Token::Or => write!(f, "OR"),
            Token::Not => write!(f, "NOT"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

// ============================================================================
// AST
// ============================================================================

/// Abstract Syntax Tree nodes for formulas
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A numeric literal
    Number(f64),
    /// A boolean literal
    Boolean(bool),
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

// ============================================================================
// Parse Error
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedChar(char, usize),
    UnexpectedToken { expected: String, found: Token },
    UnexpectedEof,
    InvalidNumber(String),
    InvalidCellRef(String),
    EmptyExpression,
}

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
        }
    }
}

impl std::error::Error for ParseError {}

// ============================================================================
// Lexer
// ============================================================================

pub struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
            pos: 0,
        }
    }

    /// Tokenize the entire input into a Vec of tokens
    pub fn tokenize(mut self) -> Result<Vec<Token>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            if tok == Token::Eof {
                tokens.push(tok);
                break;
            }
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, c)| c)
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
        let result = self.chars.next();
        if let Some((pos, _)) = result {
            self.pos = pos;
        }
        result
    }

    fn skip_whitespace(&mut self) {
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        let (pos, ch) = match self.next_char() {
            Some(x) => x,
            None => return Ok(Token::Eof),
        };

        match ch {
            // Single-char tokens
            '+' => Ok(Token::Plus),
            '-' => Ok(Token::Minus),
            '*' => Ok(Token::Star),
            '/' => Ok(Token::Slash),
            '^' => Ok(Token::Caret),
            '%' => Ok(Token::Percent),
            '(' => Ok(Token::LParen),
            ')' => Ok(Token::RParen),
            ',' => Ok(Token::Comma),
            ':' => Ok(Token::Colon),

            // Comparison operators
            '=' => {
                if self.peek_char() == Some('=') {
                    self.next_char();
                }
                Ok(Token::Eq)
            }
            '!' => {
                if self.peek_char() == Some('=') {
                    self.next_char();
                    Ok(Token::Ne)
                } else {
                    // Standalone ! is NOT
                    Ok(Token::Not)
                }
            }
            '<' => {
                if self.peek_char() == Some('=') {
                    self.next_char();
                    Ok(Token::Le)
                } else if self.peek_char() == Some('>') {
                    self.next_char();
                    Ok(Token::Ne)
                } else {
                    Ok(Token::Lt)
                }
            }
            '>' => {
                if self.peek_char() == Some('=') {
                    self.next_char();
                    Ok(Token::Ge)
                } else {
                    Ok(Token::Gt)
                }
            }

            // Logical operators
            '&' => {
                if self.peek_char() == Some('&') {
                    self.next_char();
                    Ok(Token::And)
                } else {
                    // Single & could be used as AND in some spreadsheets
                    Ok(Token::And)
                }
            }
            '|' => {
                if self.peek_char() == Some('|') {
                    self.next_char();
                    Ok(Token::Or)
                } else {
                    // Single | could be used as OR
                    Ok(Token::Or)
                }
            }

            // Numbers
            '0'..='9' | '.' => self.read_number(ch),

            // Identifiers or cell references
            'A'..='Z' | 'a'..='z' | '_' => self.read_ident_or_cell(ch),

            _ => Err(ParseError::UnexpectedChar(ch, pos)),
        }
    }

    fn read_number(&mut self, first: char) -> Result<Token, ParseError> {
        let mut s = String::new();
        s.push(first);

        // Collect digits and at most one decimal point
        let mut has_dot = first == '.';
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.next_char();
            } else if c == '.' && !has_dot {
                has_dot = true;
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }

        // Check for scientific notation
        if let Some(&(_, c)) = self.chars.peek() {
            if c == 'e' || c == 'E' {
                s.push(c);
                self.next_char();

                // Optional sign
                if let Some(&(_, sign)) = self.chars.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.next_char();
                    }
                }

                // Exponent digits
                while let Some(&(_, c)) = self.chars.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.next_char();
                    } else {
                        break;
                    }
                }
            }
        }

        let n: f64 = s.parse().map_err(|_| ParseError::InvalidNumber(s))?;
        Ok(Token::Number(n))
    }

    fn read_ident_or_cell(&mut self, first: char) -> Result<Token, ParseError> {
        let mut s = String::new();
        s.push(first);

        // Collect alphanumeric characters
        while let Some(&(_, c)) = self.chars.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }

        // Check for keywords (case-insensitive)
        let upper = s.to_uppercase();
        match upper.as_str() {
            "TRUE" => return Ok(Token::True),
            "FALSE" => return Ok(Token::False),
            "AND" => return Ok(Token::And),
            "OR" => return Ok(Token::Or),
            "NOT" => return Ok(Token::Not),
            _ => {}
        }

        // Check if this looks like a cell reference (letters followed by digits)
        if let Some((col, row)) = parse_cell_ref_parts(&s) {
            Ok(Token::CellRef { col, row })
        } else {
            Ok(Token::Ident(s))
        }
    }
}

/// Try to parse a string as a cell reference (e.g., "A1", "AA123")
/// Returns (column_letters, row_number) if valid
fn parse_cell_ref_parts(s: &str) -> Option<(String, usize)> {
    let s_upper = s.to_uppercase();
    let mut chars = s_upper.chars().peekable();

    // Collect letters (column)
    let mut col = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            col.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if col.is_empty() {
        return None;
    }

    // Collect digits (row)
    let mut row_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            row_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    // Must have digits and no trailing characters
    if row_str.is_empty() || chars.next().is_some() {
        return None;
    }

    let row: usize = row_str.parse().ok()?;
    if row == 0 {
        return None; // Row 0 is invalid (1-indexed)
    }

    Some((col, row))
}

// ============================================================================
// Parser
// ============================================================================

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
            Token::Number(n) => {
                self.advance();
                // Check if this is a row range like 1:5
                if matches!(self.peek(), Token::Colon) {
                    if n.fract() == 0.0 && n >= 1.0 {
                        self.advance();
                        if let Token::Number(end) = self.peek().clone() {
                            if end.fract() == 0.0 && end >= 1.0 {
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
                Ok(Expr::Number(n))
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

            Token::True => {
                self.advance();
                Ok(Expr::Boolean(true))
            }

            Token::False => {
                self.advance();
                Ok(Expr::Boolean(false))
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

// ============================================================================
// Convenience functions
// ============================================================================

/// Parse a formula string into an AST
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    Parser::parse_formula(input)
}

/// Extract all cell references from an expression
pub fn extract_cell_refs(expr: &Expr) -> Vec<(String, usize)> {
    let mut refs = Vec::new();
    collect_cell_refs(expr, &mut refs);
    refs
}

fn collect_cell_refs(expr: &Expr, refs: &mut Vec<(String, usize)>) {
    match expr {
        Expr::CellRef { col, row } => {
            refs.push((col.clone(), *row));
        }
        Expr::Range { start, end } => {
            collect_cell_refs(start, refs);
            collect_cell_refs(end, refs);
        }
        Expr::FnCall { args, .. } => {
            for arg in args {
                collect_cell_refs(arg, refs);
            }
        }
        Expr::BinOp { left, right, .. } => {
            collect_cell_refs(left, refs);
            collect_cell_refs(right, refs);
        }
        Expr::Neg(inner) | Expr::Not(inner) => {
            collect_cell_refs(inner, refs);
        }
        Expr::Number(_) | Expr::Boolean(_) | Expr::RowRange { .. } | Expr::ColRange { .. } => {}
    }
}

/// Check if an expression contains any ranges
pub fn has_ranges(expr: &Expr) -> bool {
    match expr {
        Expr::Range { .. } | Expr::RowRange { .. } | Expr::ColRange { .. } => true,
        Expr::FnCall { args, .. } => args.iter().any(has_ranges),
        Expr::BinOp { left, right, .. } => has_ranges(left) || has_ranges(right),
        Expr::Neg(inner) | Expr::Not(inner) => has_ranges(inner),
        Expr::Number(_) | Expr::Boolean(_) | Expr::CellRef { .. } => false,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_numbers() {
        let lexer = Lexer::new("123 45.67 1e5 2.5e-3");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0], Token::Number(n) if n == 123.0));
        assert!(matches!(tokens[1], Token::Number(n) if (n - 45.67).abs() < 0.001));
        assert!(matches!(tokens[2], Token::Number(n) if n == 1e5));
        assert!(matches!(tokens[3], Token::Number(n) if (n - 2.5e-3).abs() < 1e-10));
    }

    #[test]
    fn test_lex_cell_refs() {
        let lexer = Lexer::new("A1 AA123 Z99");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(&tokens[0], Token::CellRef { col, row } if col == "A" && *row == 1));
        assert!(matches!(&tokens[1], Token::CellRef { col, row } if col == "AA" && *row == 123));
        assert!(matches!(&tokens[2], Token::CellRef { col, row } if col == "Z" && *row == 99));
    }

    #[test]
    fn test_lex_operators() {
        let lexer = Lexer::new("+ - * / ^ % ( ) , :");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Plus);
        assert_eq!(tokens[1], Token::Minus);
        assert_eq!(tokens[2], Token::Star);
        assert_eq!(tokens[3], Token::Slash);
        assert_eq!(tokens[4], Token::Caret);
        assert_eq!(tokens[5], Token::Percent);
        assert_eq!(tokens[6], Token::LParen);
        assert_eq!(tokens[7], Token::RParen);
        assert_eq!(tokens[8], Token::Comma);
        assert_eq!(tokens[9], Token::Colon);
    }

    #[test]
    fn test_lex_comparisons() {
        let lexer = Lexer::new("= == != <> < <= > >=");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Eq);
        assert_eq!(tokens[1], Token::Eq);
        assert_eq!(tokens[2], Token::Ne);
        assert_eq!(tokens[3], Token::Ne);
        assert_eq!(tokens[4], Token::Lt);
        assert_eq!(tokens[5], Token::Le);
        assert_eq!(tokens[6], Token::Gt);
        assert_eq!(tokens[7], Token::Ge);
    }

    #[test]
    fn test_parse_number() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, Expr::Number(42.0));

        let expr = parse("3.14159").unwrap();
        assert!(matches!(expr, Expr::Number(n) if (n - 3.14159).abs() < 0.00001));
    }

    #[test]
    fn test_parse_cell_ref() {
        let expr = parse("A1").unwrap();
        assert!(matches!(expr, Expr::CellRef { col, row } if col == "A" && row == 1));

        let expr = parse("AA123").unwrap();
        assert!(matches!(expr, Expr::CellRef { col, row } if col == "AA" && row == 123));
    }

    #[test]
    fn test_parse_range() {
        let expr = parse("A1:B10").unwrap();
        assert!(matches!(expr, Expr::Range { .. }));
    }

    #[test]
    fn test_parse_col_range() {
        let expr = parse("A:C").unwrap();
        assert!(matches!(expr, Expr::ColRange { start, end } if start == "A" && end == "C"));
    }

    #[test]
    fn test_parse_arithmetic() {
        let expr = parse("A1 + B1").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Add, .. }));

        let expr = parse("A1 * 2 + B1").unwrap();
        // Should be (A1 * 2) + B1 due to precedence
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Add, .. }));

        let expr = parse("2 ^ 3 ^ 2").unwrap();
        // Should be 2 ^ (3 ^ 2) due to right associativity
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Pow, .. }));
    }

    #[test]
    fn test_parse_negation() {
        let expr = parse("-5").unwrap();
        assert!(matches!(expr, Expr::Neg(_)));

        let expr = parse("-A1").unwrap();
        assert!(matches!(expr, Expr::Neg(_)));
    }

    #[test]
    fn test_parse_function_call() {
        let expr = parse("SUM(A1:A10)").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, args } if name == "SUM" && args.len() == 1));

        let expr = parse("POW(2, 3)").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, args } if name == "POW" && args.len() == 2));
    }

    #[test]
    fn test_parse_nested_functions() {
        let expr = parse("SQRT(SUM(A1:A10))").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, .. } if name == "SQRT"));
    }

    #[test]
    fn test_parse_complex_expression() {
        let expr = parse("SUM(A1:A10) + AVG(B1:B10) * 2").unwrap();
        // Should be SUM(...) + (AVG(...) * 2)
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Add, .. }));
    }

    #[test]
    fn test_parse_with_equals() {
        // Leading = should be stripped
        let expr = parse("=A1+B1").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Add, .. }));
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = parse("(A1 + B1) * 2").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Mul, .. }));
    }

    #[test]
    fn test_extract_cell_refs() {
        let expr = parse("A1 + B2 + SUM(C1:C10)").unwrap();
        let refs = extract_cell_refs(&expr);
        assert!(refs.contains(&("A".to_string(), 1)));
        assert!(refs.contains(&("B".to_string(), 2)));
        assert!(refs.contains(&("C".to_string(), 1)));
        assert!(refs.contains(&("C".to_string(), 10)));
    }

    #[test]
    fn test_case_insensitivity() {
        // Function names should be normalized to uppercase
        let expr = parse("sum(a1:a10)").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, .. } if name == "SUM"));

        // Cell refs should preserve original case in col
        let expr = parse("a1").unwrap();
        assert!(matches!(expr, Expr::CellRef { col, .. } if col == "A"));
    }

    #[test]
    fn test_constants() {
        let expr = parse("PI()").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, args } if name == "PI" && args.is_empty()));

        let expr = parse("E()").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, args } if name == "E" && args.is_empty()));
    }

    #[test]
    fn test_empty_expression() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(parse("=").is_err());
    }

    #[test]
    fn test_comparisons() {
        let expr = parse("A1 > 10").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Gt, .. }));

        let expr = parse("A1 <= B1").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Le, .. }));
    }

    // === Boolean expression tests ===

    #[test]
    fn test_lex_boolean_literals() {
        let lexer = Lexer::new("TRUE FALSE true false True False");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::True);
        assert_eq!(tokens[1], Token::False);
        assert_eq!(tokens[2], Token::True);
        assert_eq!(tokens[3], Token::False);
        assert_eq!(tokens[4], Token::True);
        assert_eq!(tokens[5], Token::False);
    }

    #[test]
    fn test_lex_logical_operators() {
        let lexer = Lexer::new("AND OR NOT && || !");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::And);
        assert_eq!(tokens[1], Token::Or);
        assert_eq!(tokens[2], Token::Not);
        assert_eq!(tokens[3], Token::And);
        assert_eq!(tokens[4], Token::Or);
        assert_eq!(tokens[5], Token::Not);
    }

    #[test]
    fn test_parse_boolean_literals() {
        let expr = parse("TRUE").unwrap();
        assert_eq!(expr, Expr::Boolean(true));

        let expr = parse("FALSE").unwrap();
        assert_eq!(expr, Expr::Boolean(false));

        let expr = parse("true").unwrap();
        assert_eq!(expr, Expr::Boolean(true));
    }

    #[test]
    fn test_parse_not_operator() {
        let expr = parse("NOT TRUE").unwrap();
        assert!(matches!(expr, Expr::Not(_)));

        let expr = parse("!FALSE").unwrap();
        assert!(matches!(expr, Expr::Not(_)));

        let expr = parse("NOT A1").unwrap();
        assert!(matches!(expr, Expr::Not(_)));
    }

    #[test]
    fn test_parse_and_operator() {
        let expr = parse("TRUE AND FALSE").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::And, .. }));

        let expr = parse("A1 && B1").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::And, .. }));
    }

    #[test]
    fn test_parse_or_operator() {
        let expr = parse("TRUE OR FALSE").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Or, .. }));

        let expr = parse("A1 || B1").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Or, .. }));
    }

    #[test]
    fn test_boolean_precedence() {
        // OR has lower precedence than AND
        // A OR B AND C should be A OR (B AND C)
        let expr = parse("TRUE OR FALSE AND TRUE").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Or, .. }));

        // AND has lower precedence than comparison
        // A > B AND C < D should be (A > B) AND (C < D)
        let expr = parse("A1 > 5 AND B1 < 10").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::And, .. }));
    }

    #[test]
    fn test_parse_if_function() {
        let expr = parse("IF(A1>5, TRUE, FALSE)").unwrap();
        assert!(matches!(expr, Expr::FnCall { name, args } if name == "IF" && args.len() == 3));
    }

    #[test]
    fn test_complex_boolean_expression() {
        // (A1 > 5 AND B1 < 10) OR C1 = 0
        let expr = parse("(A1 > 5 AND B1 < 10) OR C1 = 0").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Or, .. }));
    }

    #[test]
    fn test_nested_not() {
        let expr = parse("NOT NOT TRUE").unwrap();
        assert!(matches!(expr, Expr::Not(_)));
    }

    #[test]
    fn test_not_with_comparison() {
        let expr = parse("NOT (A1 > 5)").unwrap();
        assert!(matches!(expr, Expr::Not(_)));
    }
}
