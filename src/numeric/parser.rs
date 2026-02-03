//! Numeric parser with proper lexer/tokenizer and structured output.
//!
//! This module provides a full parser for numeric strings that can handle:
//! - Plain integers and floats
//! - Scientific notation (1.23e-5)
//! - Currency values ($1,234.56, €1.234,56)
//! - Percentages (15%, 15.5%)
//! - Numbers with thousand separators (1,234,567.89)
//! - Negative values including accounting format (parentheses)
//! - Hexadecimal (0x1A2B), octal (0o755), binary (0b1010)

use std::fmt;

/// Tokens produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A sequence of digits
    Digits(String),
    /// Decimal point
    Decimal,
    /// Sign (+ or -)
    Sign(char),
    /// Exponent marker (e or E)
    Exponent,
    /// Percentage sign
    Percent,
    /// Currency symbol ($, €, £, ¥, etc.)
    Currency(char),
    /// Thousand separator (comma in US format)
    Comma,
    /// Open parenthesis (for accounting negative)
    OpenParen,
    /// Close parenthesis
    CloseParen,
    /// Hex prefix 0x or 0X
    HexPrefix,
    /// Octal prefix 0o or 0O
    OctalPrefix,
    /// Binary prefix 0b or 0B
    BinaryPrefix,
    /// Hex digits (a-f, A-F)
    HexDigits(String),
    /// Whitespace
    Whitespace,
    /// Unknown character
    Unknown(char),
}

/// The detected format of a numeric value
#[derive(Debug, Clone, PartialEq)]
pub enum NumericFormat {
    /// Plain integer or float
    Plain,
    /// Currency with the symbol used
    Currency(char),
    /// Percentage (value is stored as decimal, e.g., 15% -> 0.15)
    Percentage,
    /// Scientific notation with original exponent
    Scientific { exponent: i32 },
    /// Number had thousand separators
    Formatted,
    /// Hexadecimal number
    Hexadecimal,
    /// Octal number
    Octal,
    /// Binary number
    Binary,
}

/// A parsed numeric value with format information
#[derive(Debug, Clone, PartialEq)]
pub struct NumericValue {
    /// The numeric value as f64
    pub value: f64,
    /// The detected format
    pub format: NumericFormat,
    /// Whether the number was negative
    pub is_negative: bool,
    /// Whether parentheses were used for negative (accounting style)
    pub accounting_negative: bool,
    /// Number of decimal places in original input (if applicable)
    pub decimal_places: Option<usize>,
}

impl NumericValue {
    /// Create a new NumericValue
    pub fn new(value: f64, format: NumericFormat) -> Self {
        Self {
            value,
            format,
            is_negative: value < 0.0,
            accounting_negative: false,
            decimal_places: None,
        }
    }

    /// Get the value as an integer if it has no fractional part
    pub fn as_integer(&self) -> Option<i64> {
        if self.value.fract() == 0.0 && self.value.abs() < i64::MAX as f64 {
            Some(self.value as i64)
        } else {
            None
        }
    }
}

impl fmt::Display for NumericValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.format {
            NumericFormat::Plain => {
                if let Some(i) = self.as_integer() {
                    write!(f, "{}", i)
                } else {
                    write!(f, "{}", self.value)
                }
            }
            NumericFormat::Currency(sym) => {
                if self.is_negative {
                    if self.accounting_negative {
                        write!(f, "({}{})", sym, format_with_commas(self.value.abs()))
                    } else {
                        write!(f, "-{}{}", sym, format_with_commas(self.value.abs()))
                    }
                } else {
                    write!(f, "{}{}", sym, format_with_commas(self.value))
                }
            }
            NumericFormat::Percentage => {
                write!(f, "{}%", self.value * 100.0)
            }
            NumericFormat::Scientific { exponent } => {
                write!(f, "{}e{}", self.value / 10f64.powi(*exponent), exponent)
            }
            NumericFormat::Formatted => {
                if self.is_negative {
                    write!(f, "-{}", format_with_commas(self.value.abs()))
                } else {
                    write!(f, "{}", format_with_commas(self.value))
                }
            }
            NumericFormat::Hexadecimal => {
                if let Some(i) = self.as_integer() {
                    write!(f, "0x{:X}", i.unsigned_abs())
                } else {
                    write!(f, "{}", self.value)
                }
            }
            NumericFormat::Octal => {
                if let Some(i) = self.as_integer() {
                    write!(f, "0o{:o}", i.unsigned_abs())
                } else {
                    write!(f, "{}", self.value)
                }
            }
            NumericFormat::Binary => {
                if let Some(i) = self.as_integer() {
                    write!(f, "0b{:b}", i.unsigned_abs())
                } else {
                    write!(f, "{}", self.value)
                }
            }
        }
    }
}

/// Error type for parsing failures
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Input was empty
    EmptyInput,
    /// Unexpected character at position
    UnexpectedChar { char: char, position: usize },
    /// Invalid number format
    InvalidFormat(String),
    /// Multiple decimal points
    MultipleDecimals,
    /// Multiple signs
    MultipleSigns,
    /// Invalid hex digit
    InvalidHexDigit(char),
    /// Invalid octal digit
    InvalidOctalDigit(char),
    /// Invalid binary digit
    InvalidBinaryDigit(char),
    /// Number overflow
    Overflow,
    /// Mismatched parentheses
    MismatchedParentheses,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyInput => write!(f, "empty input"),
            ParseError::UnexpectedChar { char, position } => {
                write!(f, "unexpected character '{}' at position {}", char, position)
            }
            ParseError::InvalidFormat(msg) => write!(f, "invalid format: {}", msg),
            ParseError::MultipleDecimals => write!(f, "multiple decimal points"),
            ParseError::MultipleSigns => write!(f, "multiple signs"),
            ParseError::InvalidHexDigit(c) => write!(f, "invalid hex digit: {}", c),
            ParseError::InvalidOctalDigit(c) => write!(f, "invalid octal digit: {}", c),
            ParseError::InvalidBinaryDigit(c) => write!(f, "invalid binary digit: {}", c),
            ParseError::Overflow => write!(f, "number overflow"),
            ParseError::MismatchedParentheses => write!(f, "mismatched parentheses"),
        }
    }
}

impl std::error::Error for ParseError {}

/// The lexer that tokenizes input strings
pub struct Lexer<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.char_indices().peekable(),
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens
    }

    fn next_token(&mut self) -> Option<Token> {
        let (pos, ch) = self.chars.next()?;

        match ch {
            // Whitespace
            ' ' | '\t' | '\n' | '\r' => {
                self.skip_whitespace();
                Some(Token::Whitespace)
            }

            // Signs
            '+' | '-' => Some(Token::Sign(ch)),

            // Decimal point
            '.' => Some(Token::Decimal),

            // Comma (thousand separator)
            ',' => Some(Token::Comma),

            // Parentheses
            '(' => Some(Token::OpenParen),
            ')' => Some(Token::CloseParen),

            // Percent
            '%' => Some(Token::Percent),

            // Currency symbols
            '$' | '€' | '£' | '¥' | '₹' | '₽' | '₩' | '₪' | '฿' => Some(Token::Currency(ch)),

            // Exponent
            'e' | 'E' => Some(Token::Exponent),

            // Digits - need to check for 0x, 0o, 0b prefixes
            '0' => {
                if let Some(&(_, next_ch)) = self.chars.peek() {
                    match next_ch {
                        'x' | 'X' => {
                            self.chars.next(); // consume the x
                            Some(Token::HexPrefix)
                        }
                        'o' | 'O' => {
                            self.chars.next(); // consume the o
                            Some(Token::OctalPrefix)
                        }
                        'b' | 'B' => {
                            self.chars.next(); // consume the b
                            Some(Token::BinaryPrefix)
                        }
                        _ => {
                            let digits = self.collect_digits(ch);
                            Some(Token::Digits(digits))
                        }
                    }
                } else {
                    Some(Token::Digits("0".to_string()))
                }
            }

            // Regular digits
            '1'..='9' => {
                let digits = self.collect_digits(ch);
                Some(Token::Digits(digits))
            }

            // Hex digits (when we're after a hex prefix)
            'a'..='f' | 'A'..='F' => {
                let hex = self.collect_hex_digits(ch);
                Some(Token::HexDigits(hex))
            }

            // Unknown
            _ => Some(Token::Unknown(ch)),
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn collect_digits(&mut self, first: char) -> String {
        let mut digits = String::new();
        digits.push(first);
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_ascii_digit() {
                digits.push(ch);
                self.chars.next();
            } else {
                break;
            }
        }
        digits
    }

    fn collect_hex_digits(&mut self, first: char) -> String {
        let mut digits = String::new();
        digits.push(first);
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_ascii_hexdigit() {
                digits.push(ch);
                self.chars.next();
            } else {
                break;
            }
        }
        digits
    }
}

/// The parser that converts tokens into a NumericValue
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Create a new parser from tokens
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse the tokens into a NumericValue
    pub fn parse(&mut self) -> Result<NumericValue, ParseError> {
        // Filter out whitespace
        self.tokens.retain(|t| !matches!(t, Token::Whitespace));

        if self.tokens.is_empty() {
            return Err(ParseError::EmptyInput);
        }

        // Check for special formats first
        if self.is_hex() {
            return self.parse_hex();
        }
        if self.is_octal() {
            return self.parse_octal();
        }
        if self.is_binary() {
            return self.parse_binary();
        }

        // Parse standard number formats
        self.parse_standard()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        self.pos += 1;
        token
    }

    fn is_hex(&self) -> bool {
        self.tokens.iter().any(|t| matches!(t, Token::HexPrefix))
    }

    fn is_octal(&self) -> bool {
        self.tokens.iter().any(|t| matches!(t, Token::OctalPrefix))
    }

    fn is_binary(&self) -> bool {
        self.tokens.iter().any(|t| matches!(t, Token::BinaryPrefix))
    }

    fn parse_hex(&mut self) -> Result<NumericValue, ParseError> {
        let mut is_negative = false;

        // Optional sign
        if let Some(Token::Sign(s)) = self.peek() {
            is_negative = *s == '-';
            self.advance();
        }

        // Expect hex prefix
        if !matches!(self.advance(), Some(Token::HexPrefix)) {
            return Err(ParseError::InvalidFormat("expected hex prefix".into()));
        }

        // Collect hex digits
        let mut hex_str = String::new();
        while let Some(token) = self.peek() {
            match token {
                Token::Digits(d) => {
                    hex_str.push_str(d);
                    self.advance();
                }
                Token::HexDigits(h) => {
                    hex_str.push_str(h);
                    self.advance();
                }
                _ => break,
            }
        }

        if hex_str.is_empty() {
            return Err(ParseError::InvalidFormat("no hex digits".into()));
        }

        let value = u64::from_str_radix(&hex_str, 16)
            .map_err(|_| ParseError::Overflow)? as f64;

        let value = if is_negative { -value } else { value };

        Ok(NumericValue {
            value,
            format: NumericFormat::Hexadecimal,
            is_negative,
            accounting_negative: false,
            decimal_places: None,
        })
    }

    fn parse_octal(&mut self) -> Result<NumericValue, ParseError> {
        let mut is_negative = false;

        // Optional sign
        if let Some(Token::Sign(s)) = self.peek() {
            is_negative = *s == '-';
            self.advance();
        }

        // Expect octal prefix
        if !matches!(self.advance(), Some(Token::OctalPrefix)) {
            return Err(ParseError::InvalidFormat("expected octal prefix".into()));
        }

        // Collect octal digits
        let mut oct_str = String::new();
        while let Some(Token::Digits(d)) = self.peek() {
            // Validate octal digits
            for c in d.chars() {
                if !('0'..='7').contains(&c) {
                    return Err(ParseError::InvalidOctalDigit(c));
                }
            }
            oct_str.push_str(d);
            self.advance();
        }

        if oct_str.is_empty() {
            return Err(ParseError::InvalidFormat("no octal digits".into()));
        }

        let value = u64::from_str_radix(&oct_str, 8)
            .map_err(|_| ParseError::Overflow)? as f64;

        let value = if is_negative { -value } else { value };

        Ok(NumericValue {
            value,
            format: NumericFormat::Octal,
            is_negative,
            accounting_negative: false,
            decimal_places: None,
        })
    }

    fn parse_binary(&mut self) -> Result<NumericValue, ParseError> {
        let mut is_negative = false;

        // Optional sign
        if let Some(Token::Sign(s)) = self.peek() {
            is_negative = *s == '-';
            self.advance();
        }

        // Expect binary prefix
        if !matches!(self.advance(), Some(Token::BinaryPrefix)) {
            return Err(ParseError::InvalidFormat("expected binary prefix".into()));
        }

        // Collect binary digits
        let mut bin_str = String::new();
        while let Some(Token::Digits(d)) = self.peek() {
            // Validate binary digits
            for c in d.chars() {
                if c != '0' && c != '1' {
                    return Err(ParseError::InvalidBinaryDigit(c));
                }
            }
            bin_str.push_str(d);
            self.advance();
        }

        if bin_str.is_empty() {
            return Err(ParseError::InvalidFormat("no binary digits".into()));
        }

        let value = u64::from_str_radix(&bin_str, 2)
            .map_err(|_| ParseError::Overflow)? as f64;

        let value = if is_negative { -value } else { value };

        Ok(NumericValue {
            value,
            format: NumericFormat::Binary,
            is_negative,
            accounting_negative: false,
            decimal_places: None,
        })
    }

    fn parse_standard(&mut self) -> Result<NumericValue, ParseError> {
        let mut is_negative = false;
        let mut accounting_negative = false;
        let mut currency: Option<char> = None;
        let mut has_commas = false;
        let mut is_percentage = false;
        let mut decimal_places: Option<usize> = None;

        // Check for opening parenthesis (accounting negative)
        if matches!(self.peek(), Some(Token::OpenParen)) {
            accounting_negative = true;
            is_negative = true;
            self.advance();
        }

        // Check for leading sign
        if let Some(Token::Sign(s)) = self.peek() {
            if *s == '-' {
                is_negative = true;
            }
            self.advance();
        }

        // Check for currency symbol
        if let Some(Token::Currency(c)) = self.peek() {
            currency = Some(*c);
            self.advance();
        }

        // Build the number string
        let mut num_str = String::new();
        let mut has_decimal = false;
        let mut has_exponent = false;
        let mut exponent_value: Option<i32> = None;
        let mut digits_after_decimal = 0usize;

        while let Some(token) = self.peek() {
            match token {
                Token::Digits(d) => {
                    if has_decimal && !has_exponent {
                        digits_after_decimal += d.len();
                    }
                    num_str.push_str(d);
                    self.advance();
                }
                Token::Decimal => {
                    if has_decimal {
                        return Err(ParseError::MultipleDecimals);
                    }
                    has_decimal = true;
                    num_str.push('.');
                    self.advance();
                }
                Token::Comma => {
                    // Thousand separator - skip but note it
                    has_commas = true;
                    self.advance();
                }
                Token::Exponent => {
                    if has_exponent {
                        return Err(ParseError::InvalidFormat("multiple exponents".into()));
                    }
                    has_exponent = true;
                    num_str.push('e');
                    self.advance();

                    // Optional sign after exponent
                    if let Some(Token::Sign(s)) = self.peek() {
                        num_str.push(*s);
                        self.advance();
                    }

                    // Exponent digits
                    if let Some(Token::Digits(d)) = self.peek() {
                        let exp_str = d.clone();
                        num_str.push_str(&exp_str);
                        exponent_value = exp_str.parse().ok();
                        self.advance();
                    }
                }
                Token::Percent => {
                    is_percentage = true;
                    self.advance();
                    break;
                }
                Token::CloseParen => {
                    if !accounting_negative {
                        return Err(ParseError::MismatchedParentheses);
                    }
                    self.advance();
                    break;
                }
                Token::Currency(_) => {
                    // Currency at the end (some formats put it there)
                    if currency.is_none() {
                        if let Token::Currency(c) = token {
                            currency = Some(*c);
                        }
                    }
                    self.advance();
                    break;
                }
                _ => break,
            }
        }

        if num_str.is_empty() {
            return Err(ParseError::InvalidFormat("no digits found".into()));
        }

        // Parse the number
        let mut value: f64 = num_str.parse()
            .map_err(|_| ParseError::InvalidFormat(format!("cannot parse '{}'", num_str)))?;

        // Apply percentage conversion
        if is_percentage {
            value /= 100.0;
        }

        // Apply negative
        if is_negative {
            value = -value;
        }

        // Determine format
        let format = if let Some(c) = currency {
            NumericFormat::Currency(c)
        } else if is_percentage {
            NumericFormat::Percentage
        } else if has_exponent {
            NumericFormat::Scientific {
                exponent: exponent_value.unwrap_or(0),
            }
        } else if has_commas {
            NumericFormat::Formatted
        } else {
            NumericFormat::Plain
        };

        if has_decimal {
            decimal_places = Some(digits_after_decimal);
        }

        Ok(NumericValue {
            value,
            format,
            is_negative,
            accounting_negative,
            decimal_places,
        })
    }
}

/// Parse a string into a NumericValue
pub fn parse(s: &str) -> Result<NumericValue, ParseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyInput);
    }

    let mut lexer = Lexer::new(trimmed);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// Parse a string into an f64, returning None on failure (compatible with old API)
pub fn parse_numeric(s: &str) -> Option<f64> {
    parse(s).ok().map(|v| v.value)
}

/// Format a number with comma separators
fn format_with_commas(n: f64) -> String {
    let abs_n = n.abs();
    let integer_part = abs_n.trunc() as u64;
    let fract_part = abs_n.fract();

    let int_str = integer_part.to_string();
    let with_commas: String = int_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",");

    if fract_part > 0.0 {
        // Format fractional part, removing trailing zeros
        let fract_str = format!("{:.10}", fract_part);
        let fract_str = fract_str.trim_start_matches("0.");
        let fract_str = fract_str.trim_end_matches('0');
        if fract_str.is_empty() {
            with_commas
        } else {
            format!("{}.{}", with_commas, fract_str)
        }
    } else {
        with_commas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_integers() {
        let v = parse("123").unwrap();
        assert_eq!(v.value, 123.0);
        assert_eq!(v.format, NumericFormat::Plain);

        let v = parse("-456").unwrap();
        assert_eq!(v.value, -456.0);
        assert!(v.is_negative);

        let v = parse("+789").unwrap();
        assert_eq!(v.value, 789.0);
    }

    #[test]
    fn test_parse_plain_floats() {
        let v = parse("123.45").unwrap();
        assert_eq!(v.value, 123.45);
        assert_eq!(v.decimal_places, Some(2));

        let v = parse("-0.001").unwrap();
        assert_eq!(v.value, -0.001);
        assert_eq!(v.decimal_places, Some(3));
    }

    #[test]
    fn test_parse_scientific() {
        let v = parse("1.23e5").unwrap();
        assert_eq!(v.value, 123000.0);
        assert!(matches!(v.format, NumericFormat::Scientific { .. }));

        let v = parse("1.23e-3").unwrap();
        assert!((v.value - 0.00123).abs() < 1e-10);

        let v = parse("5E10").unwrap();
        assert_eq!(v.value, 5e10);
    }

    #[test]
    fn test_parse_currency() {
        let v = parse("$1,234.56").unwrap();
        assert_eq!(v.value, 1234.56);
        assert_eq!(v.format, NumericFormat::Currency('$'));

        let v = parse("-$1,234.56").unwrap();
        assert_eq!(v.value, -1234.56);
        assert!(v.is_negative);

        let v = parse("($1,234.56)").unwrap();
        assert_eq!(v.value, -1234.56);
        assert!(v.accounting_negative);

        let v = parse("€999").unwrap();
        assert_eq!(v.value, 999.0);
        assert_eq!(v.format, NumericFormat::Currency('€'));

        let v = parse("£50.00").unwrap();
        assert_eq!(v.value, 50.0);
        assert_eq!(v.format, NumericFormat::Currency('£'));
    }

    #[test]
    fn test_parse_percentage() {
        let v = parse("15%").unwrap();
        assert_eq!(v.value, 0.15);
        assert_eq!(v.format, NumericFormat::Percentage);

        let v = parse("15.5%").unwrap();
        assert_eq!(v.value, 0.155);

        let v = parse("100%").unwrap();
        assert_eq!(v.value, 1.0);

        let v = parse("-50%").unwrap();
        assert_eq!(v.value, -0.5);
    }

    #[test]
    fn test_parse_with_commas() {
        let v = parse("1,234").unwrap();
        assert_eq!(v.value, 1234.0);
        assert_eq!(v.format, NumericFormat::Formatted);

        let v = parse("1,234,567.89").unwrap();
        assert_eq!(v.value, 1234567.89);
    }

    #[test]
    fn test_parse_hex() {
        let v = parse("0x1A").unwrap();
        assert_eq!(v.value, 26.0);
        assert_eq!(v.format, NumericFormat::Hexadecimal);

        let v = parse("0xFF").unwrap();
        assert_eq!(v.value, 255.0);

        let v = parse("-0x10").unwrap();
        assert_eq!(v.value, -16.0);

        let v = parse("0xDEADBEEF").unwrap();
        assert_eq!(v.value, 3735928559.0);
    }

    #[test]
    fn test_parse_octal() {
        let v = parse("0o755").unwrap();
        assert_eq!(v.value, 493.0);
        assert_eq!(v.format, NumericFormat::Octal);

        let v = parse("0o10").unwrap();
        assert_eq!(v.value, 8.0);

        assert!(parse("0o89").is_err()); // Invalid octal digits
    }

    #[test]
    fn test_parse_binary() {
        let v = parse("0b1010").unwrap();
        assert_eq!(v.value, 10.0);
        assert_eq!(v.format, NumericFormat::Binary);

        let v = parse("0b11111111").unwrap();
        assert_eq!(v.value, 255.0);

        assert!(parse("0b123").is_err()); // Invalid binary digits
    }

    #[test]
    fn test_parse_errors() {
        assert!(matches!(parse(""), Err(ParseError::EmptyInput)));
        assert!(matches!(parse("   "), Err(ParseError::EmptyInput)));
        assert!(parse("abc").is_err());
        assert!(parse("12.34.56").is_err());
    }

    #[test]
    fn test_lexer_tokens() {
        let mut lexer = Lexer::new("$1,234.56");
        let tokens = lexer.tokenize();
        assert!(matches!(tokens[0], Token::Currency('$')));
        assert!(matches!(tokens[1], Token::Digits(_)));
    }

    #[test]
    fn test_numeric_value_display() {
        let v = parse("$1,234.56").unwrap();
        assert_eq!(v.to_string(), "$1,234.56");

        let v = parse("15%").unwrap();
        assert_eq!(v.to_string(), "15%");

        let v = parse("0xFF").unwrap();
        assert_eq!(v.to_string(), "0xFF");
    }

    #[test]
    fn test_parse_numeric_compat() {
        // Test backward compatibility function
        assert_eq!(parse_numeric("123"), Some(123.0));
        assert_eq!(parse_numeric("$1,234.56"), Some(1234.56));
        assert_eq!(parse_numeric("15%"), Some(0.15));
        assert_eq!(parse_numeric("abc"), None);
    }

    #[test]
    fn test_whitespace_handling() {
        let v = parse("  123  ").unwrap();
        assert_eq!(v.value, 123.0);

        let v = parse("  $1,234.56  ").unwrap();
        assert_eq!(v.value, 1234.56);
    }

    #[test]
    fn test_as_integer() {
        let v = parse("123").unwrap();
        assert_eq!(v.as_integer(), Some(123));

        let v = parse("123.45").unwrap();
        assert_eq!(v.as_integer(), None);

        let v = parse("123.00").unwrap();
        assert_eq!(v.as_integer(), Some(123));
    }
}
