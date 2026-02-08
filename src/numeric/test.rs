use super::*;
use super::lexer::*;
use super::calctype::*;
use super::calculator::*;
use super::format::*;
use super::parser::*;
use super::predicate::*;

use crate::table::table::Table;
use crate::util::ColumnType;


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

fn make_table(data: Vec<Vec<&str>>) -> Table {
    Table::new(
        data.into_iter()
            .map(|row| row.into_iter().map(|s| s.to_string()).collect())
            .collect()
    )
}

#[test]
fn test_basic_formula() {
    let table = make_table(vec![
        vec!["10", "20", "=A1+B1"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].2, "30");
}

#[test]
fn test_arithmetic_expression() {
    let table = make_table(vec![
        vec!["5", "3", "=(A1+B1)*2"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results[0].2, "16");
}

#[test]
fn test_power_operator() {
    let table = make_table(vec![
        vec!["2", "=A1^3"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results[0].2, "8");
}

// == String equality tests ==
#[test]
fn test_string_comparison() {
    let table = make_table(vec![
        vec!["hi", "hi", "hello", "=A1==B1", "=A1!=B1", "=A1==C1", "=A1!=C1"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 4).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 5).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 6).unwrap().2, "true");
}

// === Boolean expression tests ===

#[test]
fn test_boolean_literals() {
    let table = make_table(vec![
        vec!["=TRUE", "=FALSE"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "false");
}

#[test]
fn test_not_operator() {
    let table = make_table(vec![
        vec!["=NOT TRUE", "=NOT FALSE", "=!TRUE"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "false");
}

#[test]
fn test_and_operator() {
    let table = make_table(vec![
        vec!["=TRUE AND TRUE", "=TRUE AND FALSE", "=FALSE AND TRUE", "=FALSE AND FALSE"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "false");
}

#[test]
fn test_or_operator() {
    let table = make_table(vec![
        vec!["=TRUE OR TRUE", "=TRUE OR FALSE", "=FALSE OR TRUE", "=FALSE OR FALSE"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "true");
    assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "false");
}

#[test]
fn test_symbolic_boolean_operators() {
    let table = make_table(vec![
        vec!["=TRUE && FALSE", "=TRUE || FALSE"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 0).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 1).unwrap().2, "true");
}

#[test]
fn test_boolean_with_cell_refs() {
    let table = make_table(vec![
        vec!["TRUE", "FALSE", "=A1 AND B1", "=A1 OR B1"],
    ]);
    let calc = Calculator::new(&table, false);
    let results = calc.evaluate_all().unwrap();
    assert_eq!(results.iter().find(|r| r.1 == 2).unwrap().2, "false");
    assert_eq!(results.iter().find(|r| r.1 == 3).unwrap().2, "true");
}
#[test]
fn test_parse_numeric_basic() {
    assert_eq!(parse_numeric("123"), Some(123.0));
    assert_eq!(parse_numeric("123.45"), Some(123.45));
    assert_eq!(parse_numeric("-123.45"), Some(-123.45));
    assert_eq!(parse_numeric("  123  "), Some(123.0));
    assert_eq!(parse_numeric(""), None);
    assert_eq!(parse_numeric("abc"), None);
}

#[test]
fn test_parse_numeric_scientific() {
    assert_eq!(parse_numeric("1.23e5"), Some(123000.0));
    assert_eq!(parse_numeric("1.23e-3"), Some(0.00123));
}

#[test]
fn test_parse_numeric_currency() {
    assert_eq!(parse_numeric("$1,234.56"), Some(1234.56));
    assert_eq!(parse_numeric("$1234.56"), Some(1234.56));
    assert_eq!(parse_numeric("-$1,234.56"), Some(-1234.56));
    assert_eq!(parse_numeric("($1,234.56)"), Some(-1234.56));
    assert_eq!(parse_numeric("€1,234.56"), Some(1234.56));
    assert_eq!(parse_numeric("£1,234.56"), Some(1234.56));
}

#[test]
fn test_parse_numeric_percentage() {
    assert_eq!(parse_numeric("15%"), Some(0.15));
    assert_eq!(parse_numeric("15.5%"), Some(0.155));
    assert_eq!(parse_numeric("100%"), Some(1.0));
    assert_eq!(parse_numeric("0%"), Some(0.0));
}

#[test]
fn test_parse_numeric_with_commas() {
    assert_eq!(parse_numeric("1,234"), Some(1234.0));
    assert_eq!(parse_numeric("1,234,567.89"), Some(1234567.89));
}

#[test]
fn test_format_default() {
    assert_eq!(format_default("$1,234.56"), Some("1234.56".to_string()));
    assert_eq!(format_default("1,234"), Some("1234".to_string()));
    assert_eq!(format_default("15%"), Some("0.15".to_string()));
    assert_eq!(format_default("123.45"), Some("123.45".to_string()));
    assert_eq!(format_default("123"), Some("123".to_string()));
    assert_eq!(format_default("abc"), None);
}

#[test]
fn test_format_commas() {
    assert_eq!(format_commas("1234567"), Some("1,234,567".to_string()));
    assert_eq!(format_commas("1234567.89"), Some("1,234,567.89".to_string()));
    assert_eq!(format_commas("123"), Some("123".to_string()));
    assert_eq!(format_commas("-1234567"), Some("-1,234,567".to_string()));
    assert_eq!(format_commas("abc"), None);
}

#[test]
fn test_format_currency() {
    assert_eq!(format_currency("1234.56", '$'), Some("$1,234.56".to_string()));
    assert_eq!(format_currency("1000000", '$'), Some("$1,000,000.00".to_string()));
    assert_eq!(format_currency("99.9", '$'), Some("$99.90".to_string()));
    assert_eq!(format_currency("-1234.56", '$'), Some("-$1,234.56".to_string()));
    assert_eq!(format_currency("0.5", '$'), Some("$0.50".to_string()));
    assert_eq!(format_currency("abc", '$'), None);
}

#[test]
fn test_format_scientific() {
    assert_eq!(format_scientific("1234", 2), Some("1.23e3".to_string()));
    assert_eq!(format_scientific("0.00123", 2), Some("1.23e-3".to_string()));
    assert_eq!(format_scientific("1", 2), Some("1.00e0".to_string()));
    assert_eq!(format_scientific("abc", 2), None);
}

#[test]
fn test_format_percentage() {
    assert_eq!(format_percentage("0.15", 0), Some("15%".to_string()));
    assert_eq!(format_percentage("0.155", 1), Some("15.5%".to_string()));
    assert_eq!(format_percentage("1.0", 0), Some("100%".to_string()));
    assert_eq!(format_percentage("0.5", 2), Some("50.00%".to_string()));
    assert_eq!(format_percentage("abc", 0), None);
}

#[test]
fn test_parse_number() {
    let expr = parse("42").unwrap();
    assert_eq!(expr, Expr::Literal(CalcType::Int(42)));

    let expr = parse("3.14159").unwrap();
    assert!(matches!(expr, Expr::Literal(CalcType::Float(n)) if (n - 3.14159).abs() < 0.00001));
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
    assert_eq!(tokens[0], Token::Literal(CalcType::Bool(true)));
    assert_eq!(tokens[1], Token::Literal(CalcType::Bool(false)));
    assert_eq!(tokens[2], Token::Literal(CalcType::Bool(true)));
    assert_eq!(tokens[3], Token::Literal(CalcType::Bool(false)));
    assert_eq!(tokens[4], Token::Literal(CalcType::Bool(true)));
    assert_eq!(tokens[5], Token::Literal(CalcType::Bool(false)));
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
    assert_eq!(expr, Expr::Literal(CalcType::Bool(true)));

    let expr = parse("FALSE").unwrap();
    assert_eq!(expr, Expr::Literal(CalcType::Bool(false)));

    let expr = parse("true").unwrap();
    assert_eq!(expr, Expr::Literal(CalcType::Bool(true)));
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
