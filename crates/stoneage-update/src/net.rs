//! Network: GitHub REST API release check and verified package download (blocking; run it on a
//! background thread, see [`crate::job`]).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use ureq::http::{Response, StatusCode};

use crate::checksum::{expected_for, hex, parse_sha256sums};
use crate::release::{select_release, CheckOutcome, ReleaseInfo};
use crate::{UpdateError, USER_AGENT};

const API: &str = "https://api.github.com";

/// Download progress.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
    /// Average speed since the start of the download.
    pub bytes_per_sec: f64,
}

impl Progress {
    pub fn fraction(&self) -> Option<f32> {
        self.total.filter(|t| *t > 0).map(|t| (self.downloaded as f64 / t as f64).clamp(0.0, 1.0) as f32)
    }
}

fn agent(total_timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_global(total_timeout)
        .build()
        .into()
}

fn get(agent: &ureq::Agent, url: &str, accept: &str) -> Result<Response<ureq::Body>, UpdateError> {
    agent
        .get(url)
        .header("Accept", accept)
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))
}

/// Maps non-2xx responses (404 -> no release, 403/429 with exhausted quota -> rate limit).
fn status_error(resp: &Response<ureq::Body>, repo: &str) -> UpdateError {
    let status = resp.status();
    let header = |n: &str| resp.headers().get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
    if status == StatusCode::NOT_FOUND {
        return UpdateError::NoRelease(repo.to_string());
    }
    if status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN && header("x-ratelimit-remaining").as_deref() == Some("0"))
    {
        let reset_epoch = header("x-ratelimit-reset").and_then(|s| s.parse().ok());
        return UpdateError::RateLimited { reset_epoch };
    }
    UpdateError::Http(status.as_u16())
}

/// Asks GitHub for the newest release of `repo` and compares it with `current` (`CARGO_PKG_VERSION`).
/// Without pre-releases `/releases/latest` is used (GitHub excludes drafts and pre-releases there);
/// with pre-releases the newest of the last 30 releases.
pub fn check(repo: &str, current: &str, include_prerelease: bool) -> Result<CheckOutcome, UpdateError> {
    if !crate::is_valid_repo(repo) {
        return Err(UpdateError::NoRelease(repo.to_string()));
    }
    let url = if include_prerelease {
        format!("{API}/repos/{repo}/releases?per_page=30")
    } else {
        format!("{API}/repos/{repo}/releases/latest")
    };
    let agent = agent(Some(Duration::from_secs(60)));
    let mut resp = get(&agent, &url, "application/vnd.github+json")?;
    if !resp.status().is_success() {
        return Err(status_error(&resp, repo));
    }
    let body = resp.body_mut().read_to_string().map_err(|e| UpdateError::Network(e.to_string()))?;
    log::debug!("update check {url}: {} bytes", body.len());
    select_release(&body, current, crate::build_arch(), include_prerelease).map_err(|e| match e {
        UpdateError::NoRelease(_) => UpdateError::NoRelease(repo.to_string()),
        e => e,
    })
}

/// Downloads the package of `release` into `dir` and verifies its SHA-256 against the release's
/// `SHA256SUMS.txt`. Returns the path of the verified zip. On any error (including `cancel`) the
/// partial file is removed.
pub fn download(
    release: &ReleaseInfo,
    dir: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<PathBuf, UpdateError> {
    let agent = agent(None);

    // 1. Expected checksum (refuse unverifiable packages before downloading anything big).
    let sums_url = release.checksums_url.as_deref().ok_or_else(|| UpdateError::NoChecksum(release.asset.name.clone()))?;
    let mut resp = get(&agent, sums_url, "application/octet-stream")?;
    if !resp.status().is_success() {
        return Err(UpdateError::NoChecksum(release.asset.name.clone()));
    }
    let sums = resp.body_mut().read_to_string().map_err(|e| UpdateError::Network(e.to_string()))?;
    let expected = expected_for(&parse_sha256sums(&sums), &release.asset.name)
        .ok_or_else(|| UpdateError::NoChecksum(release.asset.name.clone()))?;

    // 2. Stream the package to "<name>.part", hashing on the fly.
    fs::create_dir_all(dir)?;
    let final_path = dir.join(&release.asset.name);
    let part = dir.join(format!("{}.part", release.asset.name));
    let result = stream_to(&agent, release, &part, cancel, progress);
    let actual = match result {
        Ok(h) => h,
        Err(e) => {
            let _ = fs::remove_file(&part);
            return Err(e);
        }
    };

    // 3. Verify.
    if !actual.eq_ignore_ascii_case(&expected) {
        let _ = fs::remove_file(&part);
        return Err(UpdateError::ChecksumMismatch { expected, actual });
    }
    let _ = fs::remove_file(&final_path);
    fs::rename(&part, &final_path)?;
    Ok(final_path)
}

fn stream_to(
    agent: &ureq::Agent,
    release: &ReleaseInfo,
    part: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<String, UpdateError> {
    let resp = get(agent, &release.asset.url, "application/octet-stream")?;
    if !resp.status().is_success() {
        return Err(UpdateError::Http(resp.status().as_u16()));
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .or((release.asset.size > 0).then_some(release.asset.size));
    let mut reader = resp.into_body().into_reader();
    let mut file = File::create(part)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let start = Instant::now();
    let mut last_report = Instant::now() - Duration::from_secs(1);
    let mut downloaded = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(UpdateError::Cancelled);
        }
        let n = reader.read(&mut buf).map_err(|e| UpdateError::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        if last_report.elapsed() >= Duration::from_millis(100) {
            last_report = Instant::now();
            progress(make_progress(downloaded, total, start));
        }
    }
    file.flush()?;
    file.sync_all()?;
    progress(make_progress(downloaded, total, start));
    if let Some(t) = total {
        if downloaded != t {
            return Err(UpdateError::Network(format!("incomplete download ({downloaded} of {t} bytes)")));
        }
    }
    Ok(hex(&hasher.finalize()))
}

fn make_progress(downloaded: u64, total: Option<u64>, start: Instant) -> Progress {
    let secs = start.elapsed().as_secs_f64().max(1e-3);
    Progress { downloaded, total, bytes_per_sec: downloaded as f64 / secs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_fraction() {
        let p = Progress { downloaded: 50, total: Some(200), bytes_per_sec: 1.0 };
        assert_eq!(p.fraction(), Some(0.25));
        assert_eq!(Progress { total: None, ..p }.fraction(), None);
        assert_eq!(Progress { total: Some(0), ..p }.fraction(), None);
    }

    /// Live network test: a repository without releases must give a clean "no release" error
    /// (or a network/rate-limit error when offline) – never a panic or a bogus update.
    /// Run with `cargo test -p stoneage-update -- --ignored`.
    #[test]
    #[ignore]
    fn nonexistent_repo_reports_no_release() {
        let r = check("Tamer-Mansour/this-repo-does-not-exist-4711", "2.0.0", false);
        eprintln!("{r:?}");
        assert!(matches!(
            r,
            Err(UpdateError::NoRelease(_)) | Err(UpdateError::Network(_)) | Err(UpdateError::RateLimited { .. })
        ));
        let r = check("Tamer-Mansour/this-repo-does-not-exist-4711", "2.0.0", true);
        assert!(r.is_err());
    }
}
