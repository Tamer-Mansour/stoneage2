//! UI strings in two layers, merged per language (the app layer wins on duplicate keys):
//!
//! * base layer – the keys of the original `StoneAge_<lang>.properties` (`MainFrame.*`,
//!   `OptionPanel.*`, ...): `assets/original/i18n/` when present locally, otherwise the own
//!   wording in `assets/public/i18n/` (see `build.rs`);
//! * app layer – the `App.*` keys of the modern UI in `assets/i18n/app_<lang>.properties`.
//!   New keys always go there (all three languages), never into the base layer.
//!
//! * [`parse_properties`] implements `java.util.Properties.load` syntax (comments, `=`/`:`/blank
//!   separators, escapes incl. `\uXXXX`, continuation lines).
//! * [`I18n::trf`] implements `java.text.MessageFormat.format` for the `{n}` patterns used by the
//!   original, including the locale number formatting it applied to `Integer`/`Double` arguments.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::javafmt::{format_integer, format_number, NumberSymbols};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    German,
    English,
    Turkish,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::German, Language::English, Language::Turkish];
    /// e.g. "de_DE"
    pub fn code(self) -> &'static str {
        match self {
            Language::German => "de_DE",
            Language::English => "en_US",
            Language::Turkish => "tr_TR",
        }
    }
    /// ISO-639 language as written to `StoneAge.ini` (`Locale.getLanguage()`).
    pub fn ini_language(self) -> &'static str {
        &self.code()[..2]
    }
    /// ISO-3166 country as written to `StoneAge.ini` (`Locale.getCountry()`).
    pub fn ini_country(self) -> &'static str {
        &self.code()[3..]
    }
    /// Maps the `Language` / `Country` keys of `StoneAge.ini` to a bundle.
    ///
    /// The original did `ResourceBundle.getBundle("StoneAge", new Locale(Language, Country))`;
    /// only `de_DE`, `en_US` and `tr_TR` bundles exist. An exact match wins; otherwise the
    /// language alone decides (`de`, `en`, `tr`, case-insensitive); anything else falls back to
    /// English (the original would have fallen back to the JVM default locale's bundle).
    pub fn from_ini(language: &str, country: &str) -> Self {
        let lang = language.trim().to_ascii_lowercase();
        let country = country.trim().to_ascii_uppercase();
        let code = format!("{lang}_{country}");
        if let Some(l) = Language::ALL.iter().find(|l| l.code() == code) {
            return *l;
        }
        match lang.as_str() {
            "de" => Language::German,
            "tr" => Language::Turkish,
            _ => Language::English,
        }
    }
    /// Number symbols used by `NumberFormat.getInstance(locale)` for this language.
    pub fn number_symbols(self) -> NumberSymbols {
        match self {
            Language::English => NumberSymbols::EN_US,
            Language::German | Language::Turkish => NumberSymbols::DE_DE,
        }
    }
}

/// Parses Java `.properties` text (`Properties.load` semantics) into `(key, value)` pairs
/// in file order. Later duplicates are kept in order (callers decide; a map takes the last).
pub fn parse_properties(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    // `str::lines` splits on \n and strips a trailing \r; also handle lone \r line ends.
    let mut physical: Vec<&str> = Vec::new();
    for l in lines.by_ref() {
        for part in l.split('\r') {
            physical.push(part);
        }
    }
    let mut i = 0;
    while i < physical.len() {
        let line = physical[i].trim_start_matches([' ', '\t', '\u{c}']);
        i += 1;
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        // build logical line
        let mut logical = String::new();
        let mut cur = line.to_string();
        loop {
            let trailing = cur.chars().rev().take_while(|c| *c == '\\').count();
            if trailing % 2 == 1 {
                cur.pop();
                logical.push_str(&cur);
                if i < physical.len() {
                    cur = physical[i].trim_start_matches([' ', '\t', '\u{c}']).to_string();
                    i += 1;
                    continue;
                }
                break;
            }
            logical.push_str(&cur);
            break;
        }
        let (k, v) = split_key_value(&logical);
        out.push((unescape(&k), unescape(&v)));
    }
    out
}

fn split_key_value(line: &str) -> (String, String) {
    let chars: Vec<char> = line.chars().collect();
    let mut key_end = chars.len();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            i += 2;
            continue;
        }
        if c == '=' || c == ':' || c == ' ' || c == '\t' || c == '\u{c}' {
            key_end = i;
            break;
        }
        i += 1;
    }
    let key: String = chars[..key_end.min(chars.len())].iter().collect();
    let mut j = key_end;
    while j < chars.len() && matches!(chars[j], ' ' | '\t' | '\u{c}') {
        j += 1;
    }
    if j < chars.len() && (chars[j] == '=' || chars[j] == ':') {
        j += 1;
        while j < chars.len() && matches!(chars[j], ' ' | '\t' | '\u{c}') {
            j += 1;
        }
    }
    let value: String = chars[j.min(chars.len())..].iter().collect();
    (key, value)
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{c}'),
            Some('u') => {
                let hex: String = (0..4).filter_map(|_| it.next()).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) if hex.len() == 4 => out.push(ch),
                    // Java throws "Malformed \uxxxx encoding"; be lenient and keep the text.
                    _ => {
                        out.push_str("\\u");
                        out.push_str(&hex);
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Escapes a key or value for writing a `.properties` file readable by `Properties.load`
/// (non-ASCII → `\uXXXX`, like `Properties.store`).
pub fn escape_properties(s: &str, is_key: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for (idx, c) in s.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            ' ' if is_key || idx == 0 => out.push_str("\\ "),
            '=' | ':' | '#' | '!' if is_key => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04X}", u));
                }
            }
            c => out.push(c),
        }
    }
    out
}

use crate::assets::{APP_I18N_DE, APP_I18N_EN, APP_I18N_TR, I18N_DE, I18N_EN, I18N_TR};

fn table(lang: Language) -> &'static HashMap<String, String> {
    static TABLES: OnceLock<[HashMap<String, String>; 3]> = OnceLock::new();
    let t = TABLES.get_or_init(|| {
        // base first, then the app layer, so app keys override base keys
        let mk = |base: &str, app: &str| {
            parse_properties(base).into_iter().chain(parse_properties(app)).collect::<HashMap<_, _>>()
        };
        [mk(I18N_DE, APP_I18N_DE), mk(I18N_EN, APP_I18N_EN), mk(I18N_TR, APP_I18N_TR)]
    });
    match lang {
        Language::German => &t[0],
        Language::English => &t[1],
        Language::Turkish => &t[2],
    }
}

/// A `MessageFormat` argument with explicit Java type semantics.
#[derive(Clone, Debug, PartialEq)]
pub enum MsgArg {
    /// `String` – inserted verbatim.
    Text(String),
    /// `Integer`/`Long` – formatted with `NumberFormat.getInstance(locale)` (grouping).
    Int(i64),
    /// `Double` – formatted with `NumberFormat.getInstance(locale)` (max. 3 fraction digits).
    Num(f64),
}

impl From<&str> for MsgArg {
    fn from(s: &str) -> Self {
        MsgArg::Text(s.to_string())
    }
}
impl From<String> for MsgArg {
    fn from(s: String) -> Self {
        MsgArg::Text(s)
    }
}
impl From<i32> for MsgArg {
    fn from(v: i32) -> Self {
        MsgArg::Int(v as i64)
    }
}
impl From<i64> for MsgArg {
    fn from(v: i64) -> Self {
        MsgArg::Int(v)
    }
}
impl From<usize> for MsgArg {
    fn from(v: usize) -> Self {
        MsgArg::Int(v as i64)
    }
}
impl From<f64> for MsgArg {
    fn from(v: f64) -> Self {
        MsgArg::Num(v)
    }
}

pub struct I18n {
    pub lang: Language,
    table: &'static HashMap<String, String>,
    fallback: &'static HashMap<String, String>,
}

impl I18n {
    pub fn new(lang: Language) -> Self {
        Self { lang, table: table(lang), fallback: table(Language::English) }
    }
    /// Look up a key such as "MainFrame.Menu.File.Caption". Falls back to English, then to the key.
    pub fn tr(&self, key: &str) -> String {
        self.table
            .get(key)
            .or_else(|| self.fallback.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
    /// True if the key exists in the current language or the English fallback.
    pub fn has(&self, key: &str) -> bool {
        self.table.contains_key(key) || self.fallback.contains_key(key)
    }
    /// `MessageFormat`-style substitution of `{0}`, `{1}`, ...
    ///
    /// `Display` arguments whose text is a plain integer or decimal number (as produced by
    /// `i32`/`usize`/`f64` `Display`) are formatted like Java formatted `Integer`/`Double`
    /// arguments (`NumberFormat.getInstance(locale)`: grouping, max. 3 fraction digits);
    /// everything else is inserted verbatim. Use [`I18n::trf_args`] for explicit typing.
    pub fn trf(&self, key: &str, args: &[&dyn std::fmt::Display]) -> String {
        let args: Vec<MsgArg> = args.iter().map(|a| classify(&a.to_string())).collect();
        self.trf_args(key, &args)
    }
    /// `MessageFormat.format(tr(key), args)` with explicitly typed arguments.
    pub fn trf_args(&self, key: &str, args: &[MsgArg]) -> String {
        message_format(&self.tr(key), args, self.lang.number_symbols())
    }
    /// Formats a number like `NumberFormat.getInstance(locale)` of this language.
    pub fn format_number(&self, x: f64) -> String {
        format_number(x, self.lang.number_symbols())
    }
}

fn classify(s: &str) -> MsgArg {
    let t = s.trim();
    let numeric_shape = !t.is_empty()
        && t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.')
        && t.chars().any(|c| c.is_ascii_digit())
        && !t[1..].contains('-');
    if numeric_shape {
        if let Ok(i) = t.parse::<i64>() {
            return MsgArg::Int(i);
        }
        if let Ok(f) = t.parse::<f64>() {
            return MsgArg::Num(f);
        }
    }
    MsgArg::Text(s.to_string())
}

/// `java.text.MessageFormat.format(pattern, args)` for `{n}` / `{n,number}` / `{n,number,integer}`
/// elements, with `'...'` quoting and `''` for a literal apostrophe. Missing arguments are
/// printed as `{n}` like Java.
pub fn message_format(pattern: &str, args: &[MsgArg], sym: NumberSymbols) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut i = 0;
    let mut in_quote = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if in_quote || c != '{' {
            out.push(c);
            i += 1;
            continue;
        }
        // argument element
        let Some(close) = chars[i + 1..].iter().position(|&c| c == '}') else {
            // Java: "Unmatched braces in the pattern" – keep the text
            out.extend(&chars[i..]);
            break;
        };
        let body: String = chars[i + 1..i + 1 + close].iter().collect();
        i += close + 2;
        let mut parts = body.splitn(3, ',').map(str::trim);
        let idx = parts.next().unwrap_or("");
        let ty = parts.next().unwrap_or("");
        let style = parts.next().unwrap_or("");
        let Ok(n) = idx.parse::<usize>() else {
            out.push('{');
            out.push_str(&body);
            out.push('}');
            continue;
        };
        match args.get(n) {
            None => {
                out.push('{');
                out.push_str(idx);
                out.push('}');
            }
            Some(a) => {
                let integer_style = ty == "number" && style == "integer";
                match a {
                    MsgArg::Text(s) => out.push_str(s),
                    MsgArg::Int(v) => out.push_str(&format_integer(*v, sym)),
                    MsgArg::Num(v) if integer_style => {
                        // HALF_EVEN to integer
                        out.push_str(&format_integer(v.round_ties_even() as i64, sym))
                    }
                    MsgArg::Num(v) => out.push_str(&format_number(*v, sym)),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_syntax() {
        let text = "# comment\n! other comment\n\n  key1 = value one \nkey2:v2\nkey3 v3\n\
                    key\\ 4 = a\\tb\\u00fc\\\\\nlong = first \\\n     second\\\n\tthird\n\
                    empty=\nweird\\=key=x\r\ncr=yes\r";
        let p = parse_properties(text);
        let m: HashMap<_, _> = p.iter().cloned().collect();
        assert_eq!(m["key1"], "value one ");
        assert_eq!(m["key2"], "v2");
        assert_eq!(m["key3"], "v3");
        assert_eq!(m["key 4"], "a\tbü\\");
        assert_eq!(m["long"], "first secondthird"); // continuation strips leading blanks
        assert_eq!(m["empty"], "");
        assert_eq!(m["weird=key"], "x");
        assert_eq!(m["cr"], "yes");
        assert_eq!(p.len(), 8);
    }

    #[test]
    fn escape_roundtrip() {
        let k = "a key=:#";
        let v = " lead\tü€\\";
        let line = format!("{} = {}", escape_properties(k, true), escape_properties(v, false));
        let p = parse_properties(&line);
        assert_eq!(p, vec![(k.to_string(), v.to_string())]);
    }

    #[test]
    fn embedded_bundles() {
        let en = I18n::new(Language::English);
        let de = I18n::new(Language::German);
        let tr = I18n::new(Language::Turkish);
        assert_eq!(en.tr("MainFrame.Menu.File.Caption"), "File");
        assert_eq!(de.tr("MainFrame.Menu.File.Caption"), "Datei");
        assert_eq!(tr.tr("MainFrame.Menu.File.Caption"), "Dosya");
        let turkish = en.tr("MainFrame.Menu.Edit.Languages.tr_TR.Caption");
        if crate::assets::ORIGINAL_ASSETS {
            assert_eq!(turkish, "turkish (türkçe)");
        } else {
            assert_eq!(turkish, "Turkish (Türkçe)");
        }
        assert_eq!(en.tr("No.Such.Key"), "No.Such.Key");
        // every App.* key exists in all three app layers, and only there
        for (k, _) in parse_properties(APP_I18N_EN) {
            assert!(k.starts_with("App."), "app layer key without App. prefix: {k}");
            assert!(de.table.contains_key(&k), "de missing {k}");
            assert!(tr.table.contains_key(&k), "tr missing {k}");
        }
        for base in [I18N_DE, I18N_EN, I18N_TR] {
            assert!(parse_properties(base).iter().all(|(k, _)| !k.starts_with("App.")));
        }
        assert!(en.has("App.Section.Files"));
    }

    /// The own (public) base layer has the same keys and placeholders as the original one, and
    /// the list values the code maps to numbers keep their numbers.
    #[test]
    fn public_base_layer_matches_original_keys() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).ok().map(|t| parse_properties(&t));
        let placeholders = |v: &str| {
            let mut p: Vec<String> = v.match_indices('{').filter_map(|(i, _)| {
                v[i..].find('}').map(|j| v[i..i + j + 1].to_string())
            }).collect();
            p.sort();
            p
        };
        let digits = |v: &str| v.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect::<String>();
        for (orig, public_name) in [
            ("de_DE.properties", "de_DE.properties"),
            ("en_US.properties", "en_US.properties"),
            ("tr_TR.utf8.properties", "tr_TR.properties"),
        ] {
            let public: HashMap<_, _> =
                read(dir.join("public/i18n").join(public_name)).unwrap().into_iter().collect();
            let en_public: HashMap<_, _> =
                read(dir.join("public/i18n/en_US.properties")).unwrap().into_iter().collect();
            assert_eq!(
                public.keys().collect::<std::collections::BTreeSet<_>>(),
                en_public.keys().collect::<std::collections::BTreeSet<_>>(),
                "public key sets differ"
            );
            for (k, v) in &public {
                if k.starts_with("OptionPanel.valid") {
                    assert_eq!(digits(v), digits(&en_public[k]), "{k}");
                }
            }
            // Original files exist only in local builds.
            let Some(orig) = read(dir.join("original/i18n").join(orig)) else { continue };
            for (k, v) in orig {
                let p = public.get(&k).unwrap_or_else(|| panic!("public {public_name} missing {k}"));
                assert_eq!(placeholders(p), placeholders(&v), "placeholders of {k}");
                if k.starts_with("OptionPanel.valid") {
                    assert_eq!(digits(p), digits(&v), "numbers of {k}");
                }
            }
        }
    }

    #[test]
    fn from_ini() {
        assert_eq!(Language::from_ini("de", "DE"), Language::German);
        assert_eq!(Language::from_ini("en", "US"), Language::English);
        assert_eq!(Language::from_ini("tr", "TR"), Language::Turkish);
        assert_eq!(Language::from_ini("DE", "at"), Language::German);
        assert_eq!(Language::from_ini("fr", "FR"), Language::English);
        assert_eq!(Language::German.ini_language(), "de");
        assert_eq!(Language::Turkish.ini_country(), "TR");
    }

    #[test]
    fn message_format_works() {
        let en = I18n::new(Language::English);
        let de = I18n::new(Language::German);
        assert_eq!(en.trf("MainFrame.lbl_Punkt.Caption", &[&3, &12]), "Point 3 / 12");
        assert_eq!(en.trf("MainFrame.lbl_Reihe.Caption", &[&1, &2048]), "Row 1 / 2,048");
        assert_eq!(de.trf_args("MessKarte.strHorizontalSchnitt", &[1.25.into()]), {
            let p = de.tr("MessKarte.strHorizontalSchnitt");
            p.replace("{0}", "1,25")
        });
        let sym = NumberSymbols::EN_US;
        assert_eq!(
            message_format("'{0}' is {0}, it''s {1}{2}", &["x".into(), 2.5.into()], sym),
            "{0} is x, it's 2.5{2}"
        );
        assert_eq!(message_format("{0,number,integer}", &[2.5.into()], sym), "2");
        assert_eq!(message_format("{0}", &[1.23456.into()], sym), "1.235");
        assert_eq!(en.trf("App.Countdown", &[&5]), "Recording in 5 s");
        assert_eq!(classify("abc"), MsgArg::Text("abc".into()));
        assert_eq!(classify("-12"), MsgArg::Int(-12));
        assert_eq!(classify("0.5"), MsgArg::Num(0.5));
        assert_eq!(classify("1-2"), MsgArg::Text("1-2".into()));
    }
}
