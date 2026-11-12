//! CSV export (`MessAreal.printCSVData`) and import (`MainFrame.importCSV`).
//!
//! The two formats are different in the original:
//! * **Export** writes one line per (row, point, sample-block):
//!   `x[m],y[m],depth[m],value` – comma separated, numbers via `Double.toString` / `long`,
//!   no header, `\r\n` line ends (Windows `PrintWriter.println`).
//! * **Import** reads a `;`-separated matrix: one header line (its token count − 2 = number of
//!   points), then 160 lines `label;step;v0;v1;...` with German number format. Values are
//!   normalised to 0..255 and stretched to 2048 samples (12.5 samples per line).
//!
//! `MessReihe.tiefe`, `MessPunkt.getValue` and `MessReihe.addPoint` are re-implemented locally
//! (private helpers below) so the export does not depend on survey globals of `model.rs`.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::javafmt::{java_double, java_round, parse_german_number};
use crate::{Point, Row, Survey, NUM_SAMPLES};

/// Number of data lines read by the importer (`160` in `importCSV`).
pub const IMPORT_LINES: usize = 160;

/// The export menu action appended `.csv` when the chosen name did not end with it
/// (case-sensitive `String.endsWith(".csv")`).
pub fn with_csv_extension(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if s.ends_with(".csv") {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{s}.csv"))
    }
}

/// Exports to `path` (`.csv` is appended if missing, like the original menu action).
pub fn export(survey: &Survey, path: &std::path::Path) -> crate::Result<()> {
    let path = with_csv_extension(path);
    let f = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::new(f);
    write_csv(survey, &mut w)?;
    w.flush()?;
    Ok(())
}

/// `MessAreal.printCSVData(PrintWriter)`.
pub fn write_csv(survey: &Survey, w: &mut dyn Write) -> crate::Result<()> {
    // blockSize <= 0 would loop forever in the original; guard it.
    let step = survey.block_size.max(1) as usize;
    for (r, row) in survey.rows.iter().enumerate() {
        for (p, point) in row.points.iter().enumerate() {
            let mut j = 0i32;
            while j < survey.horizontal_cut {
                let x = p as f64 * survey.point_distance;
                let y = r as f64 * survey.row_distance;
                let z = 1.0 * tiefe(survey, j);
                let v = get_value(survey, point, j);
                write!(w, "{},{},{},{}\r\n", java_double(x), java_double(y), java_double(z), v)?;
                j += step as i32;
            }
        }
    }
    Ok(())
}

/// Imports into `survey` exactly like the original menu action does.
pub fn import(survey: &mut Survey, path: &std::path::Path) -> crate::Result<()> {
    let bytes = std::fs::read(path)?;
    let mut r = std::io::Cursor::new(bytes);
    import_from(survey, &mut r, &path.display().to_string())
}

/// Import from a reader; `file_name` is used for the survey name
/// (`"Importierte Daten (" + file + ")"`).
pub fn import_from(survey: &mut Survey, r: &mut dyn BufRead, file_name: &str) -> crate::Result<()> {
    let mut lines = JavaLines::new(r);
    log::debug!("Import CSV: Datei: {file_name}");
    let header = lines.next_line()?.ok_or_else(|| eof(1))?;
    let n = tokenize(&header).len() as i64 - 2;
    if n < 0 {
        // NegativeArraySizeException in the original
        return Err(crate::Error::Parse { line: 1, msg: "no point columns in header".into() });
    }
    let n = n as usize;
    log::debug!("Import CSV: {n} Punkte erkannt");

    let mut fstep = 0.0f64;
    let mut data = vec![vec![0.0f64; n]; IMPORT_LINES];
    for (i, row) in data.iter_mut().enumerate() {
        let line_no = i + 2;
        let line = lines.next_line()?.ok_or_else(|| eof(line_no))?;
        let toks = tokenize(&line);
        let mut it = toks.into_iter();
        let missing = || crate::Error::Parse { line: line_no, msg: "missing column".into() };
        let _t1 = it.next().ok_or_else(missing)?;
        let t2 = it.next().ok_or_else(missing)?;
        if i == 1 {
            match parse_german_number(t2.trim()) {
                Some(v) => {
                    fstep = (1000 * java_round(160.0 * v)) as f64;
                    log::debug!("Import CSV: fstep = {} Hz", java_double(fstep));
                }
                None => log::warn!(
                    "Import CSV: falsches Zahlenformat für step, (Zeile 3, Spalte 2)"
                ),
            }
        }
        for (j, cell) in row.iter_mut().enumerate() {
            let tok = it.next().ok_or_else(missing)?;
            match parse_german_number(tok.trim()) {
                Some(v) => *cell = v,
                None => log::warn!(
                    "Import CSV: falsches Zahlenformat in Zeile {} Reihe {}",
                    i + 2,
                    j + 2
                ),
            }
        }
    }

    // min / max
    let mut min = 1_000_000.0f64;
    let mut max = -1_000_000.0f64;
    for row in &data {
        for &v in row {
            if v > max {
                max = v;
            }
            if v < min {
                min = v;
            }
        }
    }
    log::debug!("Import CSV: min = {}, max = {}", java_double(min), java_double(max));
    let range = max - min;
    for row in data.iter_mut() {
        for v in row.iter_mut() {
            *v = 255.0 * (*v - min) / range;
        }
    }

    // build the data structure
    survey.rows.clear();
    let mut row = new_row();
    survey.name = format!("Importierte Daten ({file_name})");
    for j in 0..n {
        let mut p = new_point();
        p.f_range = fstep;
        p.acquired = true;
        for k in 0..NUM_SAMPLES {
            // Deviation: the original indexes data[floor(k / 12.5)] which exceeds the 160 lines
            // for k >= 2000 and throws ArrayIndexOutOfBoundsException (import aborted with an
            // empty row). We clamp to the last line instead.
            let idx = (java_round((k as f64 / 12.5).floor()) as usize).min(IMPORT_LINES - 1);
            p.values[k] = java_round(data[idx][j]);
        }
        add_point(&mut row, p);
    }
    survey.rows.push(row);
    // Deviation: the original left the static activeReihe untouched (could point past the
    // single new row); we select the imported row.
    survey.active_row = 0;
    log::debug!("Import CSV: fertig!");
    Ok(())
}

fn eof(line: usize) -> crate::Error {
    crate::Error::Parse { line, msg: "unexpected end of file".into() }
}

/// `new StringTokenizer(line, ";")` – empty tokens are skipped.
fn tokenize(line: &str) -> Vec<&str> {
    line.split(';').filter(|t| !t.is_empty()).collect()
}

/// `BufferedReader.readLine` over bytes decoded as Windows-1252 (platform default of the
/// original); lines end at `\n`, `\r` or `\r\n`.
struct JavaLines<'a> {
    r: &'a mut dyn BufRead,
    pending_cr: bool,
}

impl<'a> JavaLines<'a> {
    fn new(r: &'a mut dyn BufRead) -> Self {
        Self { r, pending_cr: false }
    }
    fn next_line(&mut self) -> crate::Result<Option<String>> {
        let mut buf = Vec::new();
        let mut any = false;
        loop {
            let mut byte = [0u8; 1];
            let n = self.r.read(&mut byte)?;
            if n == 0 {
                break;
            }
            let b = byte[0];
            if self.pending_cr {
                self.pending_cr = false;
                if b == b'\n' {
                    continue;
                }
            }
            any = true;
            if b == b'\n' {
                return Ok(Some(decode(&buf)));
            }
            if b == b'\r' {
                self.pending_cr = true;
                return Ok(Some(decode(&buf)));
            }
            buf.push(b);
        }
        Ok(if any { Some(decode(&buf)) } else { None })
    }
}

fn decode(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(b).0.into_owned(),
    }
}

/// `MessReihe.tiefe(int)` using the survey globals (`MainFrame.areal`).
fn tiefe(s: &Survey, i: i32) -> f64 {
    let d = 1.0 * i as f64;
    let f = 500.0 * s.f_range / 2048.0 * d;
    let t = 100.0 * s.kb * s.tm * f / (2.0 * s.dfh);
    java_round(t * 100.0) as f64 / 100.0
}

/// `MessPunkt.getValue(int)` incl. its `ArrayIndexOutOfBoundsException` fallbacks.
fn get_value(s: &Survey, p: &Point, i: i32) -> i64 {
    let fallback = |i: i32| -> i64 {
        if i >= 2048 {
            p.values.get(2047).copied().unwrap_or(0)
        } else {
            p.values.first().copied().unwrap_or(0)
        }
    };
    let at = |k: i64| -> Option<i64> {
        if k < 0 {
            None
        } else {
            p.values.get(k as usize).copied()
        }
    };
    let bs = s.block_size as i64;
    if bs == 1 {
        return at(i as i64).unwrap_or_else(|| fallback(i));
    }
    if bs == 0 {
        // ArithmeticException / NaN path in the original – undefined; return the raw value.
        return at(i as i64).unwrap_or_else(|| fallback(i));
    }
    let start = (java_round(1.0 * i as f64 / bs as f64) * bs) as i32 as i64;
    let mut sum = 0i64;
    let mut max = 0i64;
    let mut k = start;
    while k < start + bs {
        match at(k) {
            Some(v) => {
                sum += v;
                max = max.max(v);
            }
            None => return fallback(i),
        }
        k += 1;
    }
    if s.use_block_average {
        sum / bs
    } else {
        max
    }
}

/// `new MessReihe()` (contains one empty, not acquired point).
fn new_row() -> Row {
    Row::new()
}

/// `new MessPunkt()`.
fn new_point() -> Point {
    Point::new()
}

/// `MessReihe.addPoint`: a single not-acquired placeholder point is replaced.
fn add_point(row: &mut Row, p: Point) {
    row.add_point(p);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn survey() -> Survey {
        let mut s = Survey::bare();
        s.name = "t".into();
        s.point_distance = 2.0;
        s.row_distance = 5.0;
        s.kb = 1.1;
        s.tm = 1.0;
        s.dfh = 400.0;
        s.f_range = 300.0;
        s.horizontal_cut = 4;
        s.block_size = 1;
        s.use_block_average = false;
        s.active_row = 0;
        s.rows.clear();
        s
    }

    fn point(base: i64) -> Point {
        let mut p = new_point();
        p.acquired = true;
        for (i, v) in p.values.iter_mut().enumerate() {
            *v = base + i as i64;
        }
        p
    }

    #[test]
    fn export_format() {
        let mut s = survey();
        let mut r = new_row();
        add_point(&mut r, point(10));
        add_point(&mut r, point(20));
        s.rows.push(r.clone());
        s.rows.push(r);
        let mut out = Vec::new();
        write_csv(&s, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2 * 2 * 4);
        // tiefe(1) = round(100*(100*1.1*1*(500*300/2048*1)/(800)))/100 = 10.07
        assert_eq!(lines[0], "0.0,0.0,0.0,10");
        assert_eq!(lines[1], "0.0,0.0,10.07,11");
        assert_eq!(lines[4], "2.0,0.0,0.0,20");
        assert_eq!(lines[15], "2.0,5.0,30.21,23");
    }

    #[test]
    fn export_blocks() {
        let mut s = survey();
        s.block_size = 2;
        s.use_block_average = true;
        let mut r = new_row();
        add_point(&mut r, point(0));
        s.rows.push(r);
        let mut out = Vec::new();
        write_csv(&s, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2); // j = 0, 2
        // getValue(2): start = round(2/2)*2 = 2, avg(2,3) = 2
        assert!(lines[1].ends_with(",2"));
        s.use_block_average = false;
        assert_eq!(get_value(&s, &s.rows[0].points[0], 2), 3);
        // beyond the end → values[2047] for i >= 2048
        assert_eq!(get_value(&s, &s.rows[0].points[0], 3000), 2047);
    }

    fn import_text(n: usize) -> String {
        let mut t = String::from("Tiefe;Step");
        for j in 0..n {
            t.push_str(&format!(";P{j}"));
        }
        t.push_str("\r\n");
        for i in 0..IMPORT_LINES {
            t.push_str(&format!("{i};{}", if i == 1 { "0,1875" } else { "x" }));
            for j in 0..n {
                // value i + 10 j + 0.5 (German decimals)
                t.push_str(&format!(";{},5", i + j * 10));
            }
            t.push_str("\r\n");
        }
        t
    }

    #[test]
    fn import_matrix() {
        let mut s = survey();
        s.rows.push(new_row());
        s.rows.push(new_row());
        s.active_row = 1;
        let text = import_text(3);
        import_from(&mut s, &mut text.as_bytes(), "C:\\x\\a.csv").unwrap();
        assert_eq!(s.name, "Importierte Daten (C:\\x\\a.csv)");
        assert_eq!(s.rows.len(), 1);
        assert_eq!(s.active_row, 0);
        let row = &s.rows[0];
        assert_eq!(row.points.len(), 3);
        // fstep = 1000 * round(160 * 0.1875) = 30000
        assert_eq!(row.points[0].f_range, 30000.0);
        assert!(row.points[0].acquired);
        // min = 0.5 (line 0, point 0), max = 159 + 20 + 0.5 = 179.5; range 179
        let norm = |v: f64| java_round(255.0 * (v - 0.5) / 179.0);
        assert_eq!(row.points[0].values[0], 0);
        assert_eq!(row.points[2].values[2047], 255);
        // sample 25 → line 2, 12 → line 0, 13 → line 1
        assert_eq!(row.points[1].values[25], norm(2.0 + 10.0 + 0.5));
        assert_eq!(row.points[1].values[12], norm(10.5));
        assert_eq!(row.points[1].values[13], norm(11.5));
    }

    #[test]
    fn import_errors() {
        let mut s = survey();
        assert!(import_from(&mut s, &mut "a;b;c\n1;2;3\n".as_bytes(), "f").is_err());
        assert!(import_from(&mut s, &mut "".as_bytes(), "f").is_err());
    }

    #[test]
    fn export_import_files() {
        let dir = std::env::temp_dir().join(format!("stoneage-csv-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = survey();
        let mut r = new_row();
        add_point(&mut r, point(1));
        s.rows.push(r);
        export(&s, &dir.join("out")).unwrap();
        let text = std::fs::read_to_string(dir.join("out.csv")).unwrap();
        assert_eq!(text.lines().count(), 4);
        let inp = dir.join("in.csv");
        std::fs::write(&inp, import_text(2)).unwrap();
        import(&mut s, &inp).unwrap();
        assert_eq!(s.rows[0].points.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(with_csv_extension(Path::new("a.CSV")), PathBuf::from("a.CSV.csv"));
    }

    #[test]
    fn tokenizer_skips_empty() {
        assert_eq!(tokenize(";a;;b;"), vec!["a", "b"]);
    }
}
