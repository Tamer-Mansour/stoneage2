//! Pure logic: GitHub release JSON, version comparison, asset selection, release-notes text.

use semver::Version;
use serde::Deserialize;

use crate::UpdateError;

/// One downloadable file of a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// A release that is newer than the running version and has a package for this build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: Version,
    pub name: String,
    /// Release notes (GitHub markdown, raw).
    pub notes: String,
    /// ISO 8601 (`2026-11-12T10:00:00Z`).
    pub published_at: Option<String>,
    pub html_url: String,
    pub prerelease: bool,
    /// Package for this build.
    pub asset: Asset,
    /// `SHA256SUMS.txt` download URL.
    pub checksums_url: Option<String>,
}

impl ReleaseInfo {
    /// `YYYY-MM-DD` part of [`Self::published_at`].
    pub fn date(&self) -> Option<&str> {
        self.published_at.as_deref().map(|s| s.get(..10).unwrap_or(s))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Running version is the newest (or newer). `latest` = newest published tag, if any.
    UpToDate { latest: Option<String> },
    Available(ReleaseInfo),
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GhAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    pub browser_download_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

/// `v2.1.0` / `V2.1.0` / `2.1.0` -> semver. `None` for tags that are not versions.
pub fn parse_version(tag: &str) -> Option<Version> {
    let t = tag.trim();
    let t = t.strip_prefix('v').or_else(|| t.strip_prefix('V')).unwrap_or(t);
    Version::parse(t).ok()
}

/// `true` if release `tag` is strictly newer than `current` (semver precedence, pre-releases lower).
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(t), Some(c)) => t > c,
        _ => false,
    }
}

pub(crate) fn asset_name_for_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86" => Some("stoneage-windows-x86.zip"),
        "x64" => Some("stoneage-windows-x64.zip"),
        _ => None,
    }
}

/// Picks the package for `arch` ("x86" / "x64") out of a list of asset names.
pub fn asset_for_arch<'a>(names: impl IntoIterator<Item = &'a str>, arch: &str) -> Option<&'a str> {
    let want = asset_name_for_arch(arch)?;
    names.into_iter().find(|n| n.eq_ignore_ascii_case(want))
}

/// Chooses the newest usable release out of a `/releases` list (drafts and non-version tags are
/// ignored; pre-releases only when `include_prerelease`).
pub(crate) fn pick_newest(list: Vec<GhRelease>, include_prerelease: bool) -> Option<(GhRelease, Version)> {
    list.into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| parse_version(&r.tag_name).map(|v| (r, v)))
        .filter(|(r, v)| include_prerelease || (!r.prerelease && v.pre.is_empty()))
        .max_by(|a, b| a.1.cmp(&b.1))
}

/// Compares a release with the running version and selects the package for `arch`.
pub(crate) fn evaluate(rel: GhRelease, version: Version, current: &str, arch: &str) -> Result<CheckOutcome, UpdateError> {
    let cur = parse_version(current).ok_or_else(|| UpdateError::Parse(format!("bad current version {current}")))?;
    if version <= cur {
        return Ok(CheckOutcome::UpToDate { latest: Some(rel.tag_name) });
    }
    let asset_name = asset_for_arch(rel.assets.iter().map(|a| a.name.as_str()), arch)
        .ok_or_else(|| UpdateError::NoAsset(rel.tag_name.clone(), arch.to_string()))?
        .to_string();
    let a = rel.assets.iter().find(|a| a.name == asset_name).expect("asset present");
    let asset = Asset { name: a.name.clone(), url: a.browser_download_url.clone(), size: a.size };
    let checksums_url = rel
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(crate::CHECKSUMS_ASSET))
        .map(|a| a.browser_download_url.clone());
    Ok(CheckOutcome::Available(ReleaseInfo {
        name: rel.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| rel.tag_name.clone()),
        tag: rel.tag_name,
        version,
        notes: rel.body.unwrap_or_default(),
        published_at: rel.published_at,
        html_url: rel.html_url,
        prerelease: rel.prerelease,
        asset,
        checksums_url,
    }))
}

/// Parses a `/releases` JSON list (or a single `/releases/latest` object) and evaluates it.
pub fn select_release(json: &str, current: &str, arch: &str, include_prerelease: bool) -> Result<CheckOutcome, UpdateError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let list: Vec<GhRelease> = if value.is_array() {
        serde_json::from_value(value).map_err(|e| UpdateError::Parse(e.to_string()))?
    } else {
        vec![serde_json::from_value(value).map_err(|e| UpdateError::Parse(e.to_string()))?]
    };
    let empty = list.is_empty();
    match pick_newest(list, include_prerelease) {
        Some((rel, v)) => evaluate(rel, v, current, arch),
        None if empty => Err(UpdateError::NoRelease(String::new())),
        None => Ok(CheckOutcome::UpToDate { latest: None }),
    }
}

/// Renders GitHub markdown release notes as plain text for a label: headings without `#`,
/// bullets as `•`, `**bold**` / `` `code` `` markers and link targets removed, CRLF normalised.
pub fn notes_to_text(md: &str) -> String {
    let mut out = Vec::new();
    for line in md.replace("\r\n", "\n").lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];
        let mut l = if let Some(h) = trimmed.strip_prefix('#') {
            h.trim_start_matches('#').trim().to_string()
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            format!("{indent}• {rest}")
        } else {
            line.to_string()
        };
        l = l.replace("**", "").replace("__", "").replace('`', "");
        out.push(strip_links(&l));
    }
    // Collapse runs of blank lines.
    let mut text = String::new();
    let mut blank = false;
    for l in out {
        if l.trim().is_empty() {
            if !blank && !text.is_empty() {
                text.push('\n');
            }
            blank = true;
        } else {
            text.push_str(&l);
            text.push('\n');
            blank = false;
        }
    }
    text.trim_end().to_string()
}

/// `[text](url)` -> `text`.
fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        if let Some(close) = after.find("](") {
            if let Some(end) = after[close + 2..].find(')') {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[close + 2 + end + 1..];
                continue;
            }
        }
        out.push_str(&rest[..=open]);
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v2.0.1", "2.0.0"));
        assert!(is_newer("V2.1.0", "2.0.9"));
        assert!(is_newer("3.0.0", "2.99.99"));
        assert!(!is_newer("v2.0.0", "2.0.0"));
        assert!(!is_newer("v1.9.9", "2.0.0"));
        assert!(!is_newer("v2.1.0-beta.1", "2.1.0"));
        assert!(is_newer("v2.1.0-beta.2", "2.1.0-beta.1"));
        assert!(is_newer("v2.1.0", "2.1.0-rc.1"));
        assert!(!is_newer("latest", "2.0.0"));
        assert!(!is_newer("v2.1", "2.0.0"));
    }

    #[test]
    fn asset_selection() {
        let names = ["SHA256SUMS.txt", "stoneage-windows-x64.zip", "stoneage-windows-x86.zip"];
        assert_eq!(asset_for_arch(names, "x86"), Some("stoneage-windows-x86.zip"));
        assert_eq!(asset_for_arch(names, "x64"), Some("stoneage-windows-x64.zip"));
        assert_eq!(asset_for_arch(names, "arm64"), None);
        assert_eq!(asset_for_arch(["stoneage-windows-x64.zip"], "x86"), None);
    }

    fn rel(tag: &str, pre: bool, draft: bool) -> String {
        format!(
            r#"{{"tag_name":"{tag}","name":"StoneAge {tag}","body":"Changes\r\n- **Fixed** it","draft":{draft},"prerelease":{pre},
            "published_at":"2026-11-10T08:00:00Z","html_url":"https://github.com/o/r/releases/tag/{tag}",
            "assets":[{{"name":"stoneage-windows-x86.zip","size":1000,"browser_download_url":"https://x/{tag}/x86.zip"}},
                      {{"name":"stoneage-windows-x64.zip","size":2000,"browser_download_url":"https://x/{tag}/x64.zip"}},
                      {{"name":"SHA256SUMS.txt","size":200,"browser_download_url":"https://x/{tag}/sums"}}]}}"#
        )
    }

    #[test]
    fn select_from_list() {
        let json = format!("[{},{},{},{}]", rel("v2.0.0", false, false), rel("v2.2.0-beta.1", true, false),
            rel("v2.1.0", false, false), rel("v9.0.0", false, true));
        match select_release(&json, "2.0.0", "x86", false).unwrap() {
            CheckOutcome::Available(r) => {
                assert_eq!(r.tag, "v2.1.0");
                assert_eq!(r.asset.url, "https://x/v2.1.0/x86.zip");
                assert_eq!(r.asset.size, 1000);
                assert_eq!(r.checksums_url.as_deref(), Some("https://x/v2.1.0/sums"));
                assert_eq!(r.date(), Some("2026-11-10"));
            }
            o => panic!("{o:?}"),
        }
        match select_release(&json, "2.0.0", "x64", true).unwrap() {
            CheckOutcome::Available(r) => assert_eq!(r.tag, "v2.2.0-beta.1"),
            o => panic!("{o:?}"),
        }
        assert_eq!(
            select_release(&json, "2.1.0", "x64", false).unwrap(),
            CheckOutcome::UpToDate { latest: Some("v2.1.0".into()) }
        );
        assert!(matches!(select_release(&json, "2.0.0", "arm64", false), Err(UpdateError::NoAsset(..))));
        assert!(matches!(select_release("[]", "2.0.0", "x64", false), Err(UpdateError::NoRelease(_))));
        assert!(matches!(select_release("{oops", "2.0.0", "x64", false), Err(UpdateError::Parse(_))));
        // single object (= /releases/latest)
        assert!(matches!(select_release(&rel("v2.0.5", false, false), "2.0.0", "x64", false),
            Ok(CheckOutcome::Available(_))));
    }

    #[test]
    fn notes_text() {
        let md = "## What's new\r\n\r\n\r\n- **Faster** plots\n  * nested `code`\nSee [the docs](https://x.y/z).";
        assert_eq!(notes_to_text(md), "What's new\n\n• Faster plots\n  • nested code\nSee the docs.");
        assert_eq!(strip_links("a [b] c"), "a [b] c");
    }
}
