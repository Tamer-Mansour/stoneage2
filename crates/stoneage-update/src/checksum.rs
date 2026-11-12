//! `SHA256SUMS.txt` parsing and SHA-256 helpers.

use sha2::{Digest, Sha256};

/// Parses `sha256sum` output: one `<64 hex>  <name>` (or `<hex> *<name>`) per line. Lines that do
/// not match are ignored. Returns `(lower-case hex, file name)`.
pub fn parse_sha256sums(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim().trim_start_matches('\u{feff}');
            let (hash, rest) = line.split_once(char::is_whitespace)?;
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let name = rest.trim_start().trim_start_matches('*').trim();
            // Accept paths like "./dist/x.zip" by keeping only the file name.
            let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
            (!name.is_empty()).then(|| (hash.to_ascii_lowercase(), name.to_string()))
        })
        .collect()
}

/// Expected hash for `file` (case-insensitive name match).
pub(crate) fn expected_for(sums: &[(String, String)], file: &str) -> Option<String> {
    sums.iter().find(|(_, n)| n.eq_ignore_ascii_case(file)).map(|(h, _)| h.clone())
}

/// Lower-case hex SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn hashing() {
        assert_eq!(sha256_hex(b"abc"), ABC);
    }

    #[test]
    fn parse_sums() {
        let text = format!(
            "\u{feff}{}  stoneage-windows-x64.zip\r\n{} *stoneage-windows-x86.zip\n\
             garbage line\n1234  short.zip\n{}  ./dist/other.zip\n",
            ABC.to_uppercase(),
            ABC,
            ABC
        );
        let sums = parse_sha256sums(&text);
        assert_eq!(sums.len(), 3);
        assert_eq!(sums[0], (ABC.to_string(), "stoneage-windows-x64.zip".to_string()));
        assert_eq!(sums[1].1, "stoneage-windows-x86.zip");
        assert_eq!(sums[2].1, "other.zip");
        assert_eq!(expected_for(&sums, "STONEAGE-windows-x86.zip").as_deref(), Some(ABC));
        assert_eq!(expected_for(&sums, "missing.zip"), None);
    }
}
