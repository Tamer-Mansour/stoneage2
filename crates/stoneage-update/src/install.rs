//! Installation: extract the verified zip next to the exe, swap the running exe (rename dance),
//! copy the remaining files, restart; cleanup of leftovers on the next start.
//!
//! Windows allows renaming (not deleting/overwriting) a running exe, so the running
//! `stoneage.exe` is renamed to `stoneage.exe.stoneage-old`, the new exe takes its name, and the
//! `.stoneage-old` file is deleted by the next process ([`after_restart`] / [`cleanup_old_files`]).

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::UpdateError;

/// Suffix of files replaced during an update (deleted on the next start).
pub const OLD_SUFFIX: &str = ".stoneage-old";
/// Environment variable set for the process started by [`restart`].
pub const RESTART_ENV: &str = "STONEAGE_UPDATE_RESTART";
/// Staging folder (inside the exe directory, so the final moves are same-volume renames).
const STAGING: &str = ".stoneage-update";
/// Files in the package that never overwrite an existing local copy (user configuration).
const KEEP_IF_PRESENT: &[&str] = &["StoneAge.ini"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstallReport {
    /// Files written (including the exe).
    pub files: usize,
    /// Non-fatal problems (a secondary file could not be replaced).
    pub warnings: Vec<String>,
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Extracts `zip` into `dest` (rejecting entries that would escape it).
fn extract(zip: &Path, dest: &Path) -> Result<(), UpdateError> {
    let file = File::open(zip)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| UpdateError::Package(e.to_string()))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| UpdateError::Package(e.to_string()))?;
        let rel = entry
            .enclosed_name()
            .ok_or_else(|| UpdateError::Package(format!("unsafe path in package: {}", entry.name())))?;
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = File::create(&out)?;
        io::copy(&mut entry, &mut f).map_err(|e| UpdateError::Package(format!("{}: {e}", entry.name())))?;
    }
    Ok(())
}

/// Finds the new exe in the staging folder: at the root, or inside a single top-level folder.
fn find_new_exe(staging: &Path, exe_name: &std::ffi::OsStr) -> Option<PathBuf> {
    let candidates = [exe_name.to_owned(), "stoneage.exe".into()];
    for name in &candidates {
        let p = staging.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    let dirs: Vec<PathBuf> = fs::read_dir(staging).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    if dirs.len() == 1 {
        for name in &candidates {
            let p = dirs[0].join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            walk(&p, out)?;
        } else {
            out.push(p);
        }
    }
    Ok(())
}

/// Moves `old` aside (to `old + OLD_SUFFIX`, or a numbered variant if that is still locked).
fn move_aside(target: &Path) -> io::Result<PathBuf> {
    for n in 0..20 {
        let aside = if n == 0 { with_suffix(target, OLD_SUFFIX) } else { with_suffix(target, &format!(".{n}{OLD_SUFFIX}")) };
        if aside.exists() && fs::remove_file(&aside).is_err() {
            continue;
        }
        fs::rename(target, &aside)?;
        return Ok(aside);
    }
    Err(io::Error::other("no free name to move the old file aside"))
}

/// Installs the verified package `zip` over the installation containing `exe` (normally
/// `std::env::current_exe()`). Nothing is replaced unless the whole package extracted fine and
/// contains the exe; if swapping the exe fails, the old exe is restored.
pub fn install(zip: &Path, exe: &Path) -> Result<InstallReport, UpdateError> {
    let exe_dir = exe.parent().ok_or_else(|| UpdateError::Io("exe has no directory".into()))?;
    let exe_name = exe.file_name().ok_or_else(|| UpdateError::Io("exe has no file name".into()))?;
    let staging = exe_dir.join(STAGING);
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)
        .map_err(|e| UpdateError::Io(format!("{}: {e} (install folder not writable?)", exe_dir.display())))?;

    let result = install_staged(zip, exe, exe_dir, &staging, exe_name);
    let _ = fs::remove_dir_all(&staging);
    result
}

fn install_staged(
    zip: &Path,
    exe: &Path,
    exe_dir: &Path,
    staging: &Path,
    exe_name: &std::ffi::OsStr,
) -> Result<InstallReport, UpdateError> {
    extract(zip, staging)?;
    let new_exe = find_new_exe(staging, exe_name)
        .ok_or_else(|| UpdateError::Package(format!("{} not found in the package", exe_name.to_string_lossy())))?;
    let root = new_exe.parent().unwrap_or(staging).to_path_buf();
    let mut files = Vec::new();
    walk(&root, &mut files)?;

    // --- swap the exe (rename dance) --------------------------------------------------------
    let aside = if exe.exists() { Some(move_aside(exe)?) } else { None };
    if let Err(e) = fs::rename(&new_exe, exe).or_else(|_| fs::copy(&new_exe, exe).map(|_| ())) {
        if let Some(a) = &aside {
            let _ = fs::rename(a, exe);
        }
        return Err(UpdateError::Io(format!("{}: {e}", exe.display())));
    }
    let mut report = InstallReport { files: 1, warnings: Vec::new() };

    // --- the remaining files ----------------------------------------------------------------
    for src in files.iter().filter(|p| **p != new_exe) {
        let Ok(rel) = src.strip_prefix(&root) else { continue };
        let target = exe_dir.join(rel);
        let keep = rel.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            KEEP_IF_PRESENT.iter().any(|k| k.eq_ignore_ascii_case(n))
        });
        if keep && target.exists() {
            continue;
        }
        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                report.warnings.push(format!("{}: {e}", parent.display()));
                continue;
            }
        }
        let copied = fs::copy(src, &target).or_else(|_| {
            // Locked (e.g. a DLL in use): move it aside like the exe, then copy.
            move_aside(&target).and_then(|_| fs::copy(src, &target))
        });
        match copied {
            Ok(_) => report.files += 1,
            Err(e) => report.warnings.push(format!("{}: {e}", target.display())),
        }
    }
    Ok(report)
}

/// Deletes leftovers of a previous update in `dir` (files ending in [`OLD_SUFFIX`], max. 3 folder
/// levels deep, and the staging folder). Returns the number of files that could not be deleted yet.
pub fn cleanup_old_files(dir: &Path) -> usize {
    fn visit(dir: &Path, depth: u32, failed: &mut usize) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 3 {
                    visit(&p, depth + 1, failed);
                }
            } else if p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(OLD_SUFFIX))
                && fs::remove_file(&p).is_err()
            {
                *failed += 1;
            }
        }
    }
    let _ = fs::remove_dir_all(dir.join(STAGING));
    let mut failed = 0;
    visit(dir, 0, &mut failed);
    failed
}

/// Starts the freshly installed `exe` with the same arguments. The caller then closes itself.
pub fn restart(exe: &Path) -> io::Result<Child> {
    Command::new(exe).args(std::env::args_os().skip(1)).env(RESTART_ENV, "1").spawn()
}

/// To be called first thing in `main`. If this process was started by [`restart`], waits (max.
/// 20 s) until the old process released `lock_file` (it is removed when the old window is closed),
/// then deletes the `*.stoneage-old` leftovers next to the exe (retrying while the old exe is
/// still locked). Without an update restart only the cheap leftover cleanup runs.
pub fn after_restart(lock_file: &Path) {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(dir) = exe.parent().map(Path::to_path_buf) else { return };
    let restarted = std::env::var_os(RESTART_ENV).is_some();
    if restarted {
        let start = Instant::now();
        while lock_file.exists() && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(100));
        }
    } else if !with_suffix(&exe, OLD_SUFFIX).exists() && !dir.join(STAGING).exists() {
        // No traces of an update: skip scanning the folder.
        return;
    }
    let start = Instant::now();
    loop {
        let failed = cleanup_old_files(&dir);
        if failed == 0 || !restarted || start.elapsed() > Duration::from_secs(3) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("stoneage-update-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let f = File::create(path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn install_swaps_exe_and_copies_files() {
        let d = tmp("install");
        let app = d.join("app");
        fs::create_dir_all(&app).unwrap();
        let exe = app.join("stoneage.exe");
        fs::write(&exe, b"old exe").unwrap();
        fs::write(app.join("StoneAge.ini"), b"user settings").unwrap();
        let zip = d.join("pkg.zip");
        make_zip(&zip, &[
            ("stoneage.exe", b"new exe"),
            ("README.md", b"readme"),
            ("licenses/LICENSE.txt", b"lic"),
            ("StoneAge.ini", b"default settings"),
        ]);
        let r = install(&zip, &exe).unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert_eq!(r.files, 3);
        assert_eq!(fs::read(&exe).unwrap(), b"new exe");
        assert_eq!(fs::read(with_suffix(&exe, OLD_SUFFIX)).unwrap(), b"old exe");
        assert_eq!(fs::read(app.join("licenses/LICENSE.txt")).unwrap(), b"lic");
        assert_eq!(fs::read(app.join("StoneAge.ini")).unwrap(), b"user settings");
        assert!(!app.join(STAGING).exists());
        assert_eq!(cleanup_old_files(&app), 0);
        assert!(!with_suffix(&exe, OLD_SUFFIX).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn package_in_subfolder() {
        let d = tmp("subfolder");
        let exe = d.join("stoneage.exe");
        fs::write(&exe, b"old").unwrap();
        let zip = d.join("pkg.zip");
        make_zip(&zip, &[("stoneage-2.1.0/stoneage.exe", b"new"), ("stoneage-2.1.0/README.md", b"r")]);
        install(&zip, &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert_eq!(fs::read(d.join("README.md")).unwrap(), b"r");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_package_replaces_nothing() {
        let d = tmp("bad");
        let exe = d.join("stoneage.exe");
        fs::write(&exe, b"old").unwrap();
        // no exe in the package
        let zip = d.join("noexe.zip");
        make_zip(&zip, &[("README.md", b"r")]);
        assert!(matches!(install(&zip, &exe), Err(UpdateError::Package(_))));
        // not a zip at all
        let junk = d.join("junk.zip");
        fs::write(&junk, b"not a zip").unwrap();
        assert!(matches!(install(&junk, &exe), Err(UpdateError::Package(_))));
        assert_eq!(fs::read(&exe).unwrap(), b"old");
        assert!(!d.join("README.md").exists());
        assert!(!with_suffix(&exe, OLD_SUFFIX).exists());
        let _ = fs::remove_dir_all(&d);
    }
}
