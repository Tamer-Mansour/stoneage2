//! `.saf` StoneAge file format (`MessAreal/MessReihe/MessPunkt.printData/readData`).
//! Must read files written by the original 1.49 byte-for-byte compatibly, and write files
//! the original can read.
//!
//! Container (`MainFrame` actions "Save" / "Load"): a ZIP archive with two DEFLATE entries,
//! in this order:
//! 1. `StoneAgeData - karte.jpg` – the map image as JPEG ([`MAP_ENTRY`]),
//! 2. `StoneAgeData - Format 003 - 2006.12.28` – the data as text ([`DATA_ENTRY`]).
//!
//! The text is Windows-1252 with CRLF line ends (Java `PrintWriter` on Windows) and consists of
//! two comment lines, `MessAreal.printData` and a trailing comment line.
//!
//! [`write`]/[`read`] handle the text entry only; [`save`]/[`load`] handle the whole zip.

use std::io::{BufRead, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::model::{Point, Row, NUM_SAMPLES, NUM_SIGNAL_SAMPLES};
use crate::{Error, Survey};

/// Name of the zip entry holding the map image.
pub const MAP_ENTRY: &str = "StoneAgeData - karte.jpg";
/// Name of the zip entry holding the survey data (`"StoneAgeData - Format " + fileFormat`).
pub const DATA_ENTRY: &str = "StoneAgeData - Format 003 - 2006.12.28";
/// Line separator written by the original (Java `println` on Windows).
pub const NEWLINE: &str = "\r\n";
/// Name the original gives the survey when loading fails (`MainFrame` "Load").
pub const LOAD_ERROR_NAME: &str = "Fehler beim Laden!";
/// JPEG quality of `ImageIO.write(img, "jpg", …)` (default 0.75).
const JPEG_QUALITY: u8 = 75;

/// Complete text of the data entry, exactly as the original writes it (with CRLF).
pub fn to_text(survey: &Survey) -> String {
    let mut s = String::with_capacity(64 + survey.rows.iter().map(|r| r.points.len()).sum::<usize>() * 20_000);
    s.push_str("# Programmversion: ");
    s.push_str(crate::ORIGINAL_VERSION);
    s.push_str(NEWLINE);
    s.push_str("# Diese Datei wurde automatisch generiert, bitte nicht veraendern!");
    s.push_str(NEWLINE);
    survey.print_data(&mut s, NEWLINE);
    s.push_str("# Daten Ende - Ende der Datei!");
    s.push_str(NEWLINE);
    s
}

/// Writes the data entry (Windows-1252, CRLF). Characters that Windows-1252 cannot represent
/// become `?` like Java's encoder.
pub fn write(survey: &Survey, w: &mut dyn Write) -> crate::Result<()> {
    w.write_all(&encode_cp1252(&to_text(survey)))?;
    Ok(())
}

/// Reads the data entry (Windows-1252 text, any line ending) into a new survey that starts from
/// the `MessAreal` constructor defaults ([`Survey::bare`]).
pub fn read(r: &mut dyn BufRead) -> crate::Result<Survey> {
    let mut s = Survey::bare();
    read_into(&mut s, r)?;
    Ok(s)
}

/// `MessAreal.readData(BufferedReader)` into an existing survey: `name`, `bemerkung` and the rows
/// are reset, every other field keeps its value unless the file sets it.
pub fn read_into(survey: &mut Survey, r: &mut dyn BufRead) -> crate::Result<()> {
    let mut bytes = Vec::new();
    r.read_to_end(&mut bytes)?;
    let text = decode_cp1252(&bytes);
    parse_text_into(survey, &text)
}

/// Parses the data text (already decoded) into `survey` (`MessAreal.readData`).
pub fn parse_text_into(survey: &mut Survey, text: &str) -> crate::Result<()> {
    let mut lines = Lines::new(text);
    read_areal(survey, &mut lines)?;
    // Deviations for robustness (the original would crash later): never leave the survey
    // without rows and keep the active row in range.
    if survey.rows.is_empty() {
        survey.rows.push(Row::new());
    }
    if survey.active_row >= survey.rows.len() {
        survey.active_row = survey.rows.len() - 1;
    }
    Ok(())
}

/// Writes a complete `.saf` file (zip with map JPEG + data text) like the original "Save".
/// The path is used as given (the original's file dialog appends `.saf`, see [`with_saf_extension`]).
pub fn save(survey: &Survey, path: &Path) -> crate::Result<()> {
    let mut buf = Cursor::new(Vec::new());
    write_container(survey, &mut buf)?;
    std::fs::write(path, buf.into_inner())?;
    Ok(())
}

/// `path` with `.saf` appended unless it already ends with `.saf` (original "Save" behaviour).
pub fn with_saf_extension(path: &Path) -> PathBuf {
    if path.to_string_lossy().ends_with(".saf") {
        path.to_path_buf()
    } else {
        let mut s = path.as_os_str().to_owned();
        s.push(".saf");
        PathBuf::from(s)
    }
}

/// Writes the zip container to any seekable writer.
pub fn write_container<W: Write + Seek>(survey: &Survey, w: W) -> crate::Result<()> {
    let mut zip = zip::ZipWriter::new(w);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(MAP_ENTRY, opts).map_err(zip_err)?;
    zip.write_all(&map_jpeg(survey)?)?;
    zip.start_file(DATA_ENTRY, opts).map_err(zip_err)?;
    write(survey, &mut zip)?;
    zip.finish().map_err(zip_err)?;
    Ok(())
}

/// Reads a complete `.saf` file like the original "Load" into a new survey.
pub fn load(path: &Path) -> crate::Result<Survey> {
    let mut s = Survey::bare();
    load_into(&mut s, path)?;
    Ok(s)
}

/// Original "Load" into the current survey. If the data cannot be parsed, the survey is reset
/// like the original (`MessAreal.init()`, name = "Fehler beim Laden!") and the error is returned.
/// I/O errors (file missing, not a zip) leave the survey untouched.
pub fn load_into(survey: &mut Survey, path: &Path) -> crate::Result<()> {
    let bytes = std::fs::read(path)?;
    read_container_into(survey, &bytes)
}

/// Like [`load_into`] for in-memory file contents.
///
/// Accepts (lenient extension) also a bare text data entry that is not zipped.
pub fn read_container_into(survey: &mut Survey, bytes: &[u8]) -> crate::Result<()> {
    if !bytes.starts_with(b"PK") {
        return load_text_or_reset(survey, bytes);
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(zip_err)?;
    // The original reads the first entry as image and the second as data, regardless of names.
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(zip_err)?;
        let mut data = Vec::new();
        f.read_to_end(&mut data)?;
        entries.push((f.name().to_string(), data));
    }
    log::debug!("Load: {:?}", entries.iter().map(|e| e.0.as_str()).collect::<Vec<_>>());
    let data_idx = entries
        .iter()
        .position(|(n, _)| n.starts_with("StoneAgeData - Format"))
        .or(if entries.len() >= 2 { Some(1) } else { None })
        .ok_or_else(|| Error::Format("no data entry in .saf archive".into()))?;
    let img_idx = entries
        .iter()
        .position(|(n, _)| n.ends_with(".jpg"))
        .or(if data_idx != 0 { Some(0) } else { None });
    if let Some(i) = img_idx {
        match image::load_from_memory(&entries[i].1) {
            Ok(img) => survey.map_image = Some(img.to_rgba8()),
            Err(e) => {
                log::warn!("Load: map image not readable: {e}");
                survey.map_image = None;
            }
        }
    }
    load_text_or_reset(survey, &entries[data_idx].1)
}

fn load_text_or_reset(survey: &mut Survey, bytes: &[u8]) -> crate::Result<()> {
    let text = decode_cp1252(bytes);
    match parse_text_into(survey, &text) {
        Ok(()) => Ok(()),
        Err(e) => {
            log::warn!("Load fehlgeschlagen: {e}");
            survey.init_rows();
            survey.name = LOAD_ERROR_NAME.to_string();
            Err(e)
        }
    }
}

fn zip_err(e: zip::result::ZipError) -> Error {
    match e {
        zip::result::ZipError::Io(io) => Error::Io(io),
        other => Error::Format(other.to_string()),
    }
}

/// JPEG bytes of the map image (default: white 800x600 like `MessKarte.jbInit`).
fn map_jpeg(survey: &Survey) -> crate::Result<Vec<u8>> {
    let rgb: image::RgbImage = match &survey.map_image {
        Some(img) => image::DynamicImage::ImageRgba8(img.clone()).to_rgb8(),
        None => {
            let (w, h) = crate::model::DEFAULT_MAP_SIZE;
            image::RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]))
        }
    };
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    rgb.write_with_encoder(enc).map_err(|e| Error::Image(e.to_string()))?;
    Ok(out)
}

/// Windows-1252 decoding (Java default charset on the original's platform).
pub fn decode_cp1252(bytes: &[u8]) -> String {
    let (s, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    s.into_owned()
}

/// Windows-1252 encoding; unmappable characters become `?` (like Java's `OutputStreamWriter`).
pub fn encode_cp1252(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut tmp = [0u8; 4];
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c as u8);
        } else {
            let (b, _, err) = encoding_rs::WINDOWS_1252.encode(c.encode_utf8(&mut tmp));
            if err || b.len() != 1 {
                out.push(b'?');
            } else {
                out.push(b[0]);
            }
        }
    }
    out
}

/// `BufferedReader.readLine()` over a string (line ends `\n`, `\r`, `\r\n`), with line numbers.
struct Lines<'a> {
    rest: &'a str,
    line_no: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        Self { rest: text, line_no: 0 }
    }
    fn next_line(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        self.line_no += 1;
        match self.rest.find(['\r', '\n']) {
            Some(i) => {
                let line = &self.rest[..i];
                let skip = if self.rest[i..].starts_with("\r\n") { 2 } else { 1 };
                self.rest = &self.rest[i + skip..];
                Some(line)
            }
            None => {
                let line = self.rest;
                self.rest = "";
                Some(line)
            }
        }
    }
    fn err(&self, msg: impl Into<String>) -> Error {
        Error::Parse { line: self.line_no, msg: msg.into() }
    }
}

/// Java `String.trim()` (removes all chars <= U+0020 at both ends).
fn jtrim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `line.substring(prefix.length())` – error if the line is shorter (Java would throw).
fn after<'a>(lines: &Lines, line: &'a str, prefix: &str) -> crate::Result<&'a str> {
    line.get(prefix.len()..).ok_or_else(|| lines.err(format!("line too short for '{prefix}'")))
}

/// Private `readString(line, prefix)`: "" if the line is not longer than the prefix,
/// else `line.substring(prefix.length())` (not trimmed).
fn read_string(line: &str, prefix: &str) -> String {
    if line.len() <= prefix.len() {
        log::trace!("readString: empty Paramter!");
        String::new()
    } else {
        line.get(prefix.len()..).unwrap_or("").to_string()
    }
}

/// `Double.parseDouble` (accepts a trailing `d`/`D`/`f`/`F` like Java).
fn parse_f64(lines: &Lines, s: &str) -> crate::Result<f64> {
    let t = jtrim(s);
    let core = t.strip_suffix(['d', 'D', 'f', 'F']).filter(|c| !c.is_empty()).unwrap_or(t);
    core.parse::<f64>().map_err(|_| lines.err(format!("not a number: '{t}'")))
}

/// `Integer.parseInt`
fn parse_i32(lines: &Lines, s: &str) -> crate::Result<i32> {
    let t = jtrim(s);
    t.parse::<i32>().map_err(|_| lines.err(format!("not an int: '{t}'")))
}

/// `Long.parseLong`
fn parse_i64(lines: &Lines, s: &str) -> crate::Result<i64> {
    let t = jtrim(s);
    t.parse::<i64>().map_err(|_| lines.err(format!("not a long: '{t}'")))
}

fn parse_usize_index(lines: &Lines, s: &str) -> crate::Result<usize> {
    let v = parse_i32(lines, s)?;
    Ok(v.max(0) as usize)
}

fn read_areal(a: &mut Survey, lines: &mut Lines) -> crate::Result<()> {
    a.description.clear();
    a.name.clear();
    a.rows.clear();
    while let Some(raw) = lines.next_line() {
        let l = jtrim(raw);
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with("Dateiformat") {
            log::debug!("Areal: Dateiformat{} gefunden!", after(lines, l, "Dateiformat = ").unwrap_or(""));
        } else if l.starts_with("name") {
            a.name = read_string(l, "name = ");
        } else if l.starts_with("bemerkung") {
            a.description.push_str(&read_string(l, "bemerkung = "));
            a.description.push('\n');
        } else if l.starts_with("datum") {
            a.date = read_string(l, "datum = ");
        } else if l.starts_with("messpunktAbstand") {
            a.point_distance = parse_f64(lines, after(lines, l, "messpunktAbstand =")?)?;
        } else if l.starts_with("messreihenAbstand") {
            a.row_distance = parse_f64(lines, after(lines, l, "messreihenAbstand =")?)?;
        } else if l.starts_with("Kb") {
            a.kb = parse_f64(lines, after(lines, l, "Kb =")?)?;
        } else if l.starts_with("Tm") {
            a.tm = parse_f64(lines, after(lines, l, "Tm =")?)?;
        } else if l.starts_with("dfh") {
            a.dfh = parse_f64(lines, after(lines, l, "dfh =")?)?;
        } else if l.starts_with("fRange") {
            a.f_range = parse_f64(lines, after(lines, l, "fRange =")?)?;
        } else if l.starts_with("disp_stepx") {
            a.disp_step_x = parse_i32(lines, after(lines, l, "disp_stepx =")?)?;
        } else if l.starts_with("disp_stepy") {
            a.disp_step_y = parse_i32(lines, after(lines, l, "disp_stepy =")?)?;
        } else if l.starts_with("disp_stepz") {
            a.disp_step_z = parse_i32(lines, after(lines, l, "disp_stepz =")?)?;
        } else if l.starts_with("blockSize") {
            a.block_size = parse_i32(lines, after(lines, l, "blockSize =")?)?;
        } else if l.starts_with("useBlockAverage") {
            a.use_block_average = jtrim(after(lines, l, "useBlockAverage =")?) == "true";
        } else if l.starts_with("showGrid") {
            a.show_grid = jtrim(after(lines, l, "showGrid =")?) == "true";
        } else if l.starts_with("interpolate") {
            a.interpolate = jtrim(after(lines, l, "interpolate =")?) == "true";
        } else if l.starts_with("showTiefe") {
            let _ = jtrim(after(lines, l, "showTiefe =")?) == "true";
            // Original: "readData: showTiefe is set to true!" – the value is always overridden.
            a.show_depth = true;
        } else if l.starts_with("colorModel =") {
            a.color_model = parse_i32(lines, after(lines, l, "colorModel =")?)?;
        } else if l.starts_with("colorModelBase") {
            a.color_model_base = parse_i32(lines, after(lines, l, "colorModelBase =")?)?;
        } else if l.starts_with("reverseColor") {
            a.reverse_color = jtrim(after(lines, l, "reverseColor =")?) == "true";
        } else if l.starts_with("activeReihe") {
            a.active_row = parse_usize_index(lines, after(lines, l, "activeReihe =")?)?;
        } else if l.starts_with("kartePosX") {
            a.map.pos_x = parse_i32(lines, after(lines, l, "kartePosX =")?)?;
        } else if l.starts_with("kartePosY") {
            a.map.pos_y = parse_i32(lines, after(lines, l, "kartePosY =")?)?;
        } else if l.starts_with("karteScale") {
            a.map.scale = parse_f64(lines, after(lines, l, "karteScale =")?)?;
        } else if l.starts_with("karteRotation") {
            a.map.rotation = parse_f64(lines, after(lines, l, "karteRotation =")?)?;
        } else if l.starts_with("ReiheStart") {
            let mut r = Row::new();
            read_reihe(&mut r, lines)?;
            a.rows.push(r);
        } else {
            log::debug!("Areal: readData: unknown Linedata: {l}");
        }
    }
    Ok(())
}

/// `MessReihe.readData` – until a line starting with `ReihenEnde` (end of file ends it too).
fn read_reihe(r: &mut Row, lines: &mut Lines) -> crate::Result<()> {
    r.points.clear();
    while let Some(raw) = lines.next_line() {
        if raw.starts_with("ReihenEnde") {
            return Ok(());
        }
        let l = jtrim(raw);
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with("activePunkt") {
            r.active_point = parse_usize_index(lines, after(lines, l, "activePunkt =")?)?;
        } else if l.starts_with("validBase") {
            r.valid_base = parse_i32(lines, after(lines, l, "validBase =")?)?;
        } else if l.starts_with("validStep") {
            r.valid_step = parse_i32(lines, after(lines, l, "validStep =")?)?;
        } else if l.starts_with("MessPunktStart") {
            let mut p = Point::new();
            read_punkt(&mut p, lines)?;
            r.add_point(p);
        } else {
            log::debug!("Reihe: readData: unknown Linedata: {l}");
        }
    }
    log::warn!("Reihe: readData: end of file before ReihenEnde");
    Ok(())
}

/// `MessPunkt.readData` – until a line starting with `MessPunktEnde`.
///
/// Like the original only the first 2047 `Values` and 4095 `SignalValues` are stored
/// (`values[2047]` / `signal_values[4095]` keep their previous value, 0 for a new point).
fn read_punkt(p: &mut Point, lines: &mut Lines) -> crate::Result<()> {
    while let Some(raw) = lines.next_line() {
        if raw.starts_with("MessPunktEnde") {
            return Ok(());
        }
        let l = jtrim(raw);
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with("fRange") {
            p.f_range = parse_f64(lines, after(lines, l, "fRange =")?)?;
        } else if l.starts_with("sampleRate") {
            p.sample_rate = parse_i32(lines, after(lines, l, "sampleRate =")?)? as i64;
        } else if l.starts_with("fullScale") {
            p.full_scale = parse_i32(lines, after(lines, l, "fullScale =")?)? as i64;
        } else if l.starts_with("timeStamp") {
            p.time_stamp = parse_i64(lines, after(lines, l, "timeStamp =")?)?;
        } else if l.starts_with("grndLevel") {
            p.grnd_level = parse_i32(lines, after(lines, l, "grndLevel =")?)? as i64;
        } else if l.starts_with("acquired") {
            p.acquired = jtrim(after(lines, l, "acquired =")?) == "true";
        } else if l.starts_with("spectrumIsSource") {
            p.spectrum_is_source = jtrim(after(lines, l, "spectrumIsSource =")?) == "true";
        } else if l.starts_with("Value") {
            read_values(lines, l, &mut p.values, NUM_SAMPLES, "Punktdaten")?;
        } else if l.starts_with("SignalValue") {
            read_values(lines, l, &mut p.signal_values, NUM_SIGNAL_SAMPLES, "SignalPunktdaten")?;
        } else {
            log::debug!("Punkt: readData: unknown Linedata: {l}");
        }
    }
    log::warn!("Punkt: readData: end of file before MessPunktEnde");
    Ok(())
}

/// `StringTokenizer` over "Values = v0 v1 …": skip two tokens, store up to `cap - 1` values.
fn read_values(lines: &Lines, l: &str, dst: &mut Vec<i64>, cap: usize, what: &str) -> crate::Result<()> {
    dst.resize(cap, 0);
    let mut toks = l.split([' ', '\t', '\n', '\r', '\u{c}']).filter(|t| !t.is_empty());
    let _ = toks.next();
    if toks.next().is_none() {
        // Java: NoSuchElementException
        return Err(lines.err("missing '=' in value line"));
    }
    let mut n = 0usize;
    for t in toks {
        if n < cap - 1 {
            dst[n] = parse_i64(lines, t)?;
        }
        n += 1;
    }
    if n > cap {
        log::debug!("Punkt: readData: zuviele {what}! fand {n} erlaubt sind {cap}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp1252_roundtrip() {
        let s = "Prüfung äöüß € \u{4e00}";
        let b = encode_cp1252(s);
        assert_eq!(b[2], 0xFC);
        assert_eq!(*b.last().unwrap(), b'?');
        assert_eq!(decode_cp1252(&b), "Prüfung äöüß € ?");
    }

    #[test]
    fn lines_like_readline() {
        let mut l = Lines::new("a\r\nb\rc\n\nd");
        let v: Vec<_> = std::iter::from_fn(|| l.next_line()).collect();
        assert_eq!(v, vec!["a", "b", "c", "", "d"]);
    }

    #[test]
    fn saf_extension() {
        assert_eq!(with_saf_extension(Path::new("x/a")), PathBuf::from("x/a.saf"));
        assert_eq!(with_saf_extension(Path::new("x/a.saf")), PathBuf::from("x/a.saf"));
    }
}
