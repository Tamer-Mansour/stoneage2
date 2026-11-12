//! Small helpers that reproduce Java runtime formatting/rounding semantics the original relied on
//! (`Double.toString`, `Math.round(double)`, `NumberFormat.getInstance(locale).format`).
//!
//! OWNER: core-misc. Public so other modules (e.g. `.saf` writing) can reuse them.

/// `java.lang.Math.round(double)` (Java 5/6 definition: `(long) Math.floor(x + 0.5)`).
/// NaN gives 0 and out-of-range values saturate, exactly like the Java `d2l` conversion.
pub fn java_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// `java.lang.Double.toString(double)` / `StringBuilder.append(double)`.
///
/// Uses the shortest round-tripping digit string (what modern JVMs print) and Java's layout:
/// plain notation for `1e-3 <= |d| < 1e7` (always at least one fraction digit, e.g. `2.0`),
/// otherwise computerized scientific notation (`1.0E7`, `1.5E-4`).
pub fn java_double(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let neg = d < 0.0;
    let a = d.abs();
    let sci = format!("{a:e}");
    let (mant, exp) = sci.split_once('e').expect("{:e} always contains 'e'");
    let exp: i32 = exp.parse().expect("valid exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if (1e-3..1e7).contains(&a) {
        if exp >= 0 {
            let int_len = (exp + 1) as usize;
            if digits.len() <= int_len {
                out.push_str(&digits);
                for _ in digits.len()..int_len {
                    out.push('0');
                }
                out.push_str(".0");
            } else {
                out.push_str(&digits[..int_len]);
                out.push('.');
                out.push_str(&digits[int_len..]);
            }
        } else {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        out.push('E');
        out.push_str(&exp.to_string());
    }
    out
}

/// Separators of `NumberFormat.getInstance(locale)` for the supported locales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberSymbols {
    pub grouping: char,
    pub decimal: char,
}

impl NumberSymbols {
    pub const EN_US: NumberSymbols = NumberSymbols { grouping: ',', decimal: '.' };
    /// de_DE and tr_TR use the same symbols.
    pub const DE_DE: NumberSymbols = NumberSymbols { grouping: '.', decimal: ',' };
}

/// `NumberFormat.getInstance(locale).format(x)` – pattern `#,##0.###`, HALF_EVEN rounding.
pub fn format_number(x: f64, sym: NumberSymbols) -> String {
    if x.is_nan() {
        return "\u{FFFD}".into(); // DecimalFormatSymbols.getNaN()
    }
    if x.is_infinite() {
        return if x > 0.0 { "\u{221E}".into() } else { "-\u{221E}".into() };
    }
    let s = format!("{:.3}", x.abs());
    let (int_part, frac) = s.split_once('.').unwrap_or((&s, ""));
    let frac = frac.trim_end_matches('0');
    let is_zero = int_part.chars().all(|c| c == '0') && frac.is_empty();
    let mut out = String::new();
    if x < 0.0 && !is_zero {
        out.push('-');
    }
    out.push_str(&group(int_part, sym.grouping));
    if !frac.is_empty() {
        out.push(sym.decimal);
        out.push_str(frac);
    }
    out
}

/// `NumberFormat.getInstance(locale).format(long)`.
pub fn format_integer(x: i64, sym: NumberSymbols) -> String {
    let mut out = String::new();
    if x < 0 {
        out.push('-');
    }
    out.push_str(&group(&x.unsigned_abs().to_string(), sym.grouping));
    out
}

fn group(int_digits: &str, sep: char) -> String {
    let n = int_digits.len();
    let mut out = String::with_capacity(n + n / 3);
    for (i, c) in int_digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// `NumberFormat.getInstance(Locale.GERMANY).parse(s)` (lenient prefix parse, as `DecimalFormat` does):
/// optional `-`, digits with `.` grouping, `,` decimal point, optional `E` exponent. Parsing stops at
/// the first character that does not fit; `None` if no digit could be parsed (Java `ParseException`).
pub fn parse_german_number(s: &str) -> Option<f64> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut neg = false;
    if chars.first() == Some(&'-') {
        neg = true;
        i = 1;
    }
    let mut num = String::new();
    let mut seen_digit = false;
    let mut seen_decimal = false;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            num.push(c);
            seen_digit = true;
        } else if c == '.' && !seen_decimal {
            // grouping separator: only accepted when followed by a digit
            if !chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()) {
                break;
            }
        } else if c == ',' && !seen_decimal {
            seen_decimal = true;
            num.push('.');
        } else {
            break;
        }
        i += 1;
    }
    if !seen_digit {
        return None;
    }
    // exponent
    if i < chars.len() && chars[i] == 'E' {
        let mut j = i + 1;
        let mut e = String::new();
        if j < chars.len() && (chars[j] == '-' || chars[j] == '+') {
            if chars[j] == '-' {
                e.push('-');
            }
            j += 1;
        }
        let start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            e.push(chars[j]);
            j += 1;
        }
        if j > start {
            num.push('e');
            num.push_str(&e);
        }
    }
    let v: f64 = num.parse().ok()?;
    Some(if neg { -v } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_to_string() {
        assert_eq!(java_double(2.0), "2.0");
        assert_eq!(java_double(0.0), "0.0");
        assert_eq!(java_double(-0.0), "-0.0");
        assert_eq!(java_double(400.0), "400.0");
        assert_eq!(java_double(1.1), "1.1");
        assert_eq!(java_double(0.001), "0.001");
        assert_eq!(java_double(0.0001), "1.0E-4");
        assert_eq!(java_double(1.5e-4), "1.5E-4");
        assert_eq!(java_double(1e7), "1.0E7");
        assert_eq!(java_double(12345678.9), "1.23456789E7");
        assert_eq!(java_double(9999999.0), "9999999.0");
        assert_eq!(java_double(-3.25), "-3.25");
        assert_eq!(java_double(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(java_double(100.0), "100.0");
    }

    #[test]
    fn round() {
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(-2.6), -3);
        assert_eq!(java_round(f64::NAN), 0);
    }

    #[test]
    fn number_format() {
        assert_eq!(format_integer(2048, NumberSymbols::EN_US), "2,048");
        assert_eq!(format_integer(2048, NumberSymbols::DE_DE), "2.048");
        assert_eq!(format_integer(-1234567, NumberSymbols::EN_US), "-1,234,567");
        assert_eq!(format_integer(12, NumberSymbols::EN_US), "12");
        assert_eq!(format_number(1.5, NumberSymbols::DE_DE), "1,5");
        assert_eq!(format_number(1234.56789, NumberSymbols::EN_US), "1,234.568");
        assert_eq!(format_number(3.0, NumberSymbols::EN_US), "3");
        assert_eq!(format_number(0.0625, NumberSymbols::EN_US), "0.062"); // HALF_EVEN
        assert_eq!(format_number(-0.0001, NumberSymbols::EN_US), "0");
    }

    #[test]
    fn german_parse() {
        assert_eq!(parse_german_number("1,5"), Some(1.5));
        assert_eq!(parse_german_number("1.234,5"), Some(1234.5));
        assert_eq!(parse_german_number("-0,25"), Some(-0.25));
        assert_eq!(parse_german_number("12abc"), Some(12.0));
        assert_eq!(parse_german_number("1,5E3"), Some(1500.0));
        assert_eq!(parse_german_number("abc"), None);
        assert_eq!(parse_german_number(""), None);
    }
}
