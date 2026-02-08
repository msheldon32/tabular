use std::fmt;
use super::calctype::CalcType;
use super::parser::ParseError;



/// Token types produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A literal value (int, float, string, or bool)
    Literal(CalcType),
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
            Token::Literal(CalcType::Int(n)) => write!(f, "{}", n),
            Token::Literal(CalcType::Float(n)) => write!(f, "{}", n),
            Token::Literal(CalcType::Str(s)) => write!(f, "\"{}\"", s),
            Token::Literal(CalcType::Bool(true)) => write!(f, "TRUE"),
            Token::Literal(CalcType::Bool(false)) => write!(f, "FALSE"),
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

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
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

            '"' => {
                let mut buf = Vec::new();
                while let Some((_,ch)) = self.next_char() {
                    if ch == '"' {
                        return Ok(Token::Literal(CalcType::Str(buf.into_iter().collect())));
                    } else if ch == '\\' {
                        if self.peek_char() == Some('\\') {
                            buf.push('\\');
                            self.next_char();
                        } else if self.peek_char() == Some('"') {
                            buf.push('"');
                            self.next_char();
                        }
                    } else {
                        buf.push(ch);
                    }
                }
                Err(ParseError::UnclosedQuote)
            }

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
        let mut has_exp = false;
        if let Some(&(_, c)) = self.chars.peek() {
            if c == 'e' || c == 'E' {
                has_exp = true;
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

        // Parse as integer if possible (no dot, no exponent, fits in i64)
        if !has_dot && !has_exp {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Token::Literal(CalcType::Int(n)));
            }
        }

        // Otherwise parse as float
        let n: f64 = s.parse().map_err(|_| ParseError::InvalidNumber(s))?;
        Ok(Token::Literal(CalcType::Float(n)))
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
            "TRUE" => return Ok(Token::Literal(CalcType::Bool(true))),
            "FALSE" => return Ok(Token::Literal(CalcType::Bool(false))),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_numbers() {
        let lexer = Lexer::new("123 45.67 1e5 2.5e-3");
        let tokens = lexer.tokenize().unwrap();
        assert!(matches!(tokens[0], Token::Literal(CalcType::Int(123))));
        assert!(matches!(tokens[1], Token::Literal(CalcType::Float(n)) if (n - 45.67).abs() < 0.001));
        assert!(matches!(tokens[2], Token::Literal(CalcType::Float(n)) if n == 1e5));
        assert!(matches!(tokens[3], Token::Literal(CalcType::Float(n)) if (n - 2.5e-3).abs() < 1e-10));
    }

    #[test]
    fn test_lex_string() {
        let lexer = Lexer::new("\"Hello world\" \"Welcome to my lexer\"");
        let tokens = lexer.tokenize().unwrap();

        assert!(matches!(&tokens[0], Token::Literal(CalcType::Str(s)) if s == "Hello world"));
        assert!(matches!(&tokens[1], Token::Literal(CalcType::Str(s)) if s == "Welcome to my lexer"));
    }

    #[test]
    fn test_lex_string_escape() {
        let lexer = Lexer::new("\"\\\"Hello world\\\"\" \"Welcome to my\\\" lexer\"");
        let tokens = lexer.tokenize().unwrap();

        assert!(matches!(&tokens[0], Token::Literal(CalcType::Str(s)) if s == "\"Hello world\""));
        assert!(matches!(&tokens[1], Token::Literal(CalcType::Str(s)) if s == "Welcome to my\" lexer"));
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

}
