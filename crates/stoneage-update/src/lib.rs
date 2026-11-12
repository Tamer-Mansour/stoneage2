//! Self-updater for StoneAge 2.
//!
//! Flow (driven by the app, never automatic installation):
//! 1. [`check`] asks the GitHub REST API for the latest release of [`UPDATE_REPO`] and compares its
//!    `vX.Y.Z` tag with the running version.
//! 2. [`download`] streams the zip for this build ([`asset_name`]) to a temp directory and verifies it
//!    against the release's `SHA256SUMS.txt` (mismatch -> file deleted, nothing replaced).
//! 3. [`install`] extracts the zip next to the exe into a staging folder, then swaps the exe with the
//!    rename dance (running exe -> `*.stoneage-old`, new exe -> exe) and copies the other files.
//! 4. [`restart`] starts the new exe; the next start calls [`after_restart`] which waits for the old
//!    process to finish and deletes the `*.stoneage-old` leftovers.
//!
//! [`job`] runs check / download on a background thread with progress and cancellation.

mod checksum;
mod install;
pub mod job;
mod net;
mod release;

pub use checksum::{parse_sha256sums, sha256_hex};
pub use install::{after_restart, cleanup_old_files, install, restart, InstallReport, OLD_SUFFIX, RESTART_ENV};
pub use job::{Event, Job};
pub use net::{check, download, Progress};
pub use release::{
    asset_for_arch, is_newer, notes_to_text, parse_version, select_release, Asset, CheckOutcome, ReleaseInfo,
};

/// GitHub repository (`owner/name`) whose Releases are checked. The ONE place to change it;
/// it can also be overridden at runtime with `UpdateRepo = owner/name` in `StoneAge.ini`.
pub const UPDATE_REPO: &str = "Tamer-Mansour/stoneage2";

/// `User-Agent` sent to GitHub (required by the API). No personal data is sent.
pub const USER_AGENT: &str = "StoneAge2-Updater";

/// Release asset holding the checksums (`sha256sum` format: `<hex>  <file name>`).
pub const CHECKSUMS_ASSET: &str = "SHA256SUMS.txt";

/// Build architecture shown to the user: "x86" (32-bit, needed for the PCSGU250 scope DLL) or "x64".
pub fn build_arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    }
}

/// Name of the release asset for this build, `None` if no package is published for it.
pub fn asset_name() -> Option<&'static str> {
    release::asset_name_for_arch(build_arch())
}

/// Effective repository: a valid `owner/name` override (StoneAge.ini `UpdateRepo`) or [`UPDATE_REPO`].
pub fn effective_repo(override_value: Option<&str>) -> String {
    match override_value.map(str::trim) {
        Some(r) if is_valid_repo(r) => r.to_string(),
        _ => UPDATE_REPO.to_string(),
    }
}

/// `owner/name` with GitHub-legal characters only.
pub fn is_valid_repo(r: &str) -> bool {
    let mut parts = r.split('/');
    let ok = |s: Option<&str>| {
        s.is_some_and(|s| {
            !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

/// Web page listing all releases.
pub fn releases_page(repo: &str) -> String {
    format!("https://github.com/{repo}/releases")
}

/// Everything that can go wrong. Errors before [`install`] never touch the installed files.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("GitHub is not reachable (offline?): {0}")]
    Network(String),
    #[error("GitHub API rate limit reached, try again later")]
    RateLimited { reset_epoch: Option<u64> },
    #[error("no release found for {0}")]
    NoRelease(String),
    #[error("GitHub returned HTTP {0}")]
    Http(u16),
    #[error("invalid release data: {0}")]
    Parse(String),
    #[error("release {0} has no package for this build ({1})")]
    NoAsset(String, String),
    #[error("release has no checksum for {0}")]
    NoChecksum(String),
    #[error("checksum mismatch (expected {expected}, got {actual}) - download discarded")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("cancelled")]
    Cancelled,
    #[error("file error: {0}")]
    Io(String),
    #[error("invalid update package: {0}")]
    Package(String),
}

impl From<std::io::Error> for UpdateError {
    fn from(e: std::io::Error) -> Self {
        UpdateError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_override() {
        assert_eq!(effective_repo(None), UPDATE_REPO);
        assert_eq!(effective_repo(Some(" me/app ")), "me/app");
        assert_eq!(effective_repo(Some("bad")), UPDATE_REPO);
        assert_eq!(effective_repo(Some("a/b/c")), UPDATE_REPO);
        assert_eq!(effective_repo(Some("a/b?x=1")), UPDATE_REPO);
        assert!(is_valid_repo("Tamer-Mansour/stoneage2"));
    }

    #[test]
    fn this_build_has_an_asset() {
        if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            assert!(asset_name().is_some());
        }
    }
}
