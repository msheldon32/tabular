/// Cell formatting operations for visual mode
///
/// Format operations are destructive - they modify the actual cell content.
/// All operations return None if the cell is not a valid number.

/// Parse a string that may contain formatted numbers (currency, percentages, etc.)
/// Returns the numeric value if parseable.
///
/// Handles:
/// - Regular numbers: "123.45", "-123.45"
/// - Currency: "$1,234.56", "-$1,234.56", "€1.234,56"
/// - Percentages: "15%", "15.5%" (returns 0.15, 0.155)
/// - Scientific notation: "1.23e-5" (handled by standard parse)
pub fn parse_numeric(s: &str) -> Option<f64> {
    let trimmed = s.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Try standard parse first (handles scientific notation too)
    if let Ok(n) = trimmed.parse::<f64>() {
        return Some(n);
    }

    // Check for percentage
    if trimmed.ends_with('%') {
        let without_pct = trimmed.trim_end_matches('%').trim();
        // Remove commas and try to parse
        let cleaned: String = without_pct.chars().filter(|c| *c != ',').collect();
        if let Ok(n) = cleaned.parse::<f64>() {
            return Some(n / 100.0);
        }
    }

    // Check for currency (common symbols: $, €, £, ¥)
    let currency_chars = ['$', '€', '£', '¥'];
    let mut s = trimmed.to_string();

    // Handle negative currency: -$123 or ($123)
    let is_negative = s.starts_with('-') || (s.starts_with('(') && s.ends_with(')'));

    if is_negative {
        if s.starts_with('-') {
            s = s[1..].to_string();
        } else if s.starts_with('(') && s.ends_with(')') {
            s = s[1..s.len()-1].to_string();
        }
    }

    // Remove currency symbol
    let s = s.trim();
    let has_currency = currency_chars.iter().any(|&c| s.starts_with(c));

    if has_currency {
        let without_symbol: String = s.chars().skip(1).collect();
        // Remove commas (thousand separators)
        let cleaned: String = without_symbol.chars().filter(|c| *c != ',').collect();
        if let Ok(n) = cleaned.trim().parse::<f64>() {
            return Some(if is_negative { -n } else { n });
        }
    }

    // Try removing commas as a last resort (for numbers like "1,234.56")
    let cleaned: String = trimmed.chars().filter(|c| *c != ',').collect();
    if let Ok(n) = cleaned.parse::<f64>() {
        return Some(n);
    }

    None
}

/// Count decimal places in a string representation of a number
fn decimal_places(s: &str) -> usize {
    // Handle scientific notation
    if s.contains('e') || s.contains('E') {
        return 0; // Already in scientific notation
    }

    if let Some(dot_pos) = s.find('.') {
        s.len() - dot_pos - 1
    } else {
        0
    }
}

/// Reduce decimal places by one (e.g., 123.456 -> 123.46)
pub fn reduce_decimal(val: &str) -> Option<String> {
    let trimmed = val.trim();
    let n: f64 = trimmed.parse().ok()?;

    let current = decimal_places(trimmed);
    if current == 0 {
        // Already an integer, nothing to reduce
        return Some(format!("{}", n as i64));
    }

    let new_places = current.saturating_sub(1);
    if new_places == 0 {
        Some(format!("{}", n.round() as i64))
    } else {
        Some(format!("{:.prec$}", n, prec = new_places))
    }
}

/// Increase decimal places by one (e.g., 123.4 -> 123.40)
pub fn increase_decimal(val: &str) -> Option<String> {
    let trimmed = val.trim();
    let n: f64 = trimmed.parse().ok()?;

    let current = decimal_places(trimmed);
    let new_places = current + 1;

    Some(format!("{:.prec$}", n, prec = new_places))
}

/// Format as currency with symbol and thousands separators (e.g., 1234.56 -> $1,234.56)
pub fn format_currency(val: &str, symbol: char) -> Option<String> {
    let trimmed = val.trim();
    let n: f64 = trimmed.parse().ok()?;

    let is_negative = n < 0.0;
    let abs_n = n.abs();

    // Split into integer and decimal parts
    let integer_part = abs_n.trunc() as u64;
    let decimal_part = ((abs_n.fract() * 100.0).round() as u64) % 100;

    // Format integer with commas
    let int_str = integer_part.to_string();
    let with_commas: String = int_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",");

    if is_negative {
        Some(format!("-{}{}.{:02}", symbol, with_commas, decimal_part))
    } else {
        Some(format!("{}{}.{:02}", symbol, with_commas, decimal_part))
    }
}

/// Format in scientific notation (e.g., 0.00001234 -> 1.23e-5)
pub fn format_scientific(val: &str, precision: usize) -> Option<String> {
    let trimmed = val.trim();
    let n: f64 = trimmed.parse().ok()?;

    if n == 0.0 {
        return Some(format!("0.{}e0", "0".repeat(precision)));
    }

    Some(format!("{:.prec$e}", n, prec = precision))
}

/// Format as percentage (e.g., 0.15 -> 15%)
pub fn format_percentage(val: &str, decimals: usize) -> Option<String> {
    let trimmed = val.trim();
    let n: f64 = trimmed.parse().ok()?;

    let pct = n * 100.0;
    if decimals == 0 {
        Some(format!("{}%", pct.round() as i64))
    } else {
        Some(format!("{:.prec$}%", pct, prec = decimals))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_decimal_places() {
        assert_eq!(decimal_places("123"), 0);
        assert_eq!(decimal_places("123.4"), 1);
        assert_eq!(decimal_places("123.45"), 2);
        assert_eq!(decimal_places("123.456"), 3);
        assert_eq!(decimal_places("0.1"), 1);
    }

    #[test]
    fn test_reduce_decimal() {
        assert_eq!(reduce_decimal("123.456"), Some("123.46".to_string()));
        assert_eq!(reduce_decimal("123.45"), Some("123.5".to_string()));
        assert_eq!(reduce_decimal("123.4"), Some("123".to_string()));
        assert_eq!(reduce_decimal("123"), Some("123".to_string()));
        assert_eq!(reduce_decimal("0.999"), Some("1.00".to_string()));
        assert_eq!(reduce_decimal("abc"), None);
    }

    #[test]
    fn test_increase_decimal() {
        assert_eq!(increase_decimal("123"), Some("123.0".to_string()));
        assert_eq!(increase_decimal("123.4"), Some("123.40".to_string()));
        assert_eq!(increase_decimal("123.45"), Some("123.450".to_string()));
        assert_eq!(increase_decimal("abc"), None);
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
}
