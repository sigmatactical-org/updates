//! Mirror `.dbc` schemas from their single source of truth on GitHub.
//!
//! The catalog directory is only a local cache: a background thread lists the
//! configured repo path via the GitHub contents API, downloads every schema,
//! and prunes local files that no longer exist upstream. When GitHub is
//! unreachable the cached copies keep being served.

use std::fs;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::config;

const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// One entry of a GitHub contents-API directory listing.
#[derive(Debug, Deserialize)]
struct ContentsEntry {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    download_url: Option<String>,
}

/// Spawn the background mirror thread; the first pass runs immediately.
pub fn spawn() {
    thread::Builder::new()
        .name("dbc-github-sync".into())
        .spawn(|| {
            loop {
                match sync_once() {
                    Ok(n) => eprintln!(
                        "dbc-sync: {n} schema(s) mirrored from {}",
                        config::dbc_github_source()
                    ),
                    Err(e) => eprintln!(
                        "dbc-sync: {e}; serving cached copies from {}",
                        config::dbc_dir().display()
                    ),
                }
                thread::sleep(config::dbc_sync_interval());
            }
        })
        .expect("spawn dbc-github-sync thread");
}

/// One full mirror pass; returns how many schemas exist upstream.
fn sync_once() -> Result<usize, String> {
    let listing_url = format!(
        "https://api.github.com/repos/{}/contents/{}?ref={}",
        config::dbc_github_repo(),
        config::dbc_github_path(),
        config::dbc_github_ref()
    );
    let body = http_get(&listing_url)?;
    let entries: Vec<ContentsEntry> =
        serde_json::from_str(&body).map_err(|e| format!("contents listing JSON: {e}"))?;
    let remote = remote_schemas(entries);

    for (filename, download_url) in &remote {
        if let Err(e) = mirror_one(filename, download_url) {
            eprintln!("dbc-sync: {filename}: {e}; keeping cached copy");
        }
    }
    prune_removed(&remote);
    Ok(remote.len())
}

/// Keep only safe, downloadable `.dbc` files from a contents listing.
fn remote_schemas(entries: Vec<ContentsEntry>) -> Vec<(String, String)> {
    entries
        .into_iter()
        .filter(|e| e.kind == "file" && super::is_safe_filename(&e.name))
        .filter_map(|e| e.download_url.map(|url| (e.name, url)))
        .collect()
}

/// Download one schema and store it if it changed (validation included).
fn mirror_one(filename: &str, download_url: &str) -> Result<(), String> {
    let content = http_get(download_url)?;
    let cached = fs::read_to_string(config::dbc_dir().join(filename)).unwrap_or_default();
    if cached == content {
        return Ok(());
    }
    super::publish_dbc(filename, content.as_bytes())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Delete cached schemas that no longer exist upstream.
fn prune_removed(remote: &[(String, String)]) {
    for file in super::list_dbc_files() {
        if !remote.iter().any(|(name, _)| *name == file.filename) {
            eprintln!("dbc-sync: pruning {} (removed upstream)", file.filename);
            let _ = super::delete_dbc(&file.filename);
        }
    }
}

/// GET a URL as text with GitHub-friendly headers.
fn http_get(url: &str) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(HTTP_TIMEOUT))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .new_agent();
    let mut request = agent
        .get(url)
        .header("User-Agent", "sigma-updates")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = config::github_token() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    request
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read {url}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: &str, url: Option<&str>) -> ContentsEntry {
        ContentsEntry {
            name: name.into(),
            kind: kind.into(),
            download_url: url.map(Into::into),
        }
    }

    #[test]
    fn filters_listing_to_safe_dbc_files() {
        let entries = vec![
            entry("sigma-racer.dbc", "file", Some("https://x/sigma-racer.dbc")),
            entry("m7-draft.yaml", "file", Some("https://x/m7-draft.yaml")),
            entry("subdir", "dir", None),
            entry("../evil.dbc", "file", Some("https://x/evil.dbc")),
            entry("no-url.dbc", "file", None),
        ];
        let remote = remote_schemas(entries);
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].0, "sigma-racer.dbc");
    }
}
