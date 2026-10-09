//! Claude Code releases: the `latest` / `stable` channels and the list of published versions.

use std::process::Command;

use serde::Serialize;

const DOWNLOADS: &str = "https://downloads.claude.ai/claude-code-releases";
const REGISTRY: &str = "https://registry.npmjs.org/@anthropic-ai/claude-code";
/// Newest versions offered in the dashboard.
const KEEP: usize = 40;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Releases {
    pub latest: String,
    pub stable: String,
    /// Newest first.
    pub versions: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReleasesError {
    #[error("could not reach {0}: {1}")]
    Fetch(String, String),
    #[error("unexpected answer from {0}")]
    Parse(String),
}

fn get(url: &str, accept: Option<&str>) -> Result<String, ReleasesError> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", "--max-time", "15"]);
    if let Some(a) = accept {
        cmd.args(["-H", &format!("Accept: {a}")]);
    }
    let out = cmd.arg(url).output().map_err(|e| ReleasesError::Fetch(url.into(), e.to_string()))?;
    if !out.status.success() {
        return Err(ReleasesError::Fetch(url.into(), String::from_utf8_lossy(&out.stderr).trim().to_owned()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn version_key(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

pub fn fetch() -> Result<Releases, ReleasesError> {
    let latest = get(&format!("{DOWNLOADS}/latest"), None)?;
    let stable = get(&format!("{DOWNLOADS}/stable"), None)?;
    if version_key(&latest).is_none() || version_key(&stable).is_none() {
        return Err(ReleasesError::Parse(DOWNLOADS.into()));
    }
    let doc = get(REGISTRY, Some("application/vnd.npm.install-v1+json"))?;
    let json: serde_json::Value = serde_json::from_str(&doc).map_err(|_| ReleasesError::Parse(REGISTRY.into()))?;
    let mut versions: Vec<String> = json["versions"]
        .as_object()
        .ok_or_else(|| ReleasesError::Parse(REGISTRY.into()))?
        .keys()
        .filter(|v| version_key(v).is_some() && !v.contains('-'))
        .cloned()
        .collect();
    versions.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
    versions.truncate(KEEP);
    Ok(Releases { latest, stable, versions })
}
