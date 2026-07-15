//! Mirror schema catalogs from their single source of truth on GitHub.
//!
//! The catalog directories are only local caches: a background thread lists
//! the configured repo paths via the GitHub contents API, downloads every
//! schema, and prunes local files that no longer exist upstream. Two catalogs
//! are mirrored — CAN dictionaries (`.dbc` from the DBC path) and VSS files
//! (`schemas/vss/*` plus the `.yaml` CAN-mapping companions that live next to
//! the DBCs). When GitHub is unreachable the cached copies keep being served.

use std::fs;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::config;
use crate::vss;

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
                    Ok((dbc, vss)) => eprintln!(
                        "dbc-sync: {dbc} dictionary(ies) + {vss} VSS file(s) mirrored from {}",
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

/// One full mirror pass; returns upstream (dictionary, VSS) counts.
fn sync_once() -> Result<(usize, usize), String> {
    let can_listing = list_remote(&config::dbc_github_path())?;
    let vss_listing = list_remote(&config::vss_github_path())?;

    let dictionaries = filter_schemas(&can_listing, super::is_safe_filename);
    // The VSS catalog is the signal tree plus the CAN-mapping yaml companions
    // that live in the DBC directory.
    let mut vss_files = filter_schemas(&vss_listing, vss::is_safe_filename);
    vss_files.extend(filter_schemas(&can_listing, vss::is_safe_filename));

    for (filename, url) in &dictionaries {
        if let Err(e) = mirror_one(&config::dbc_dir(), filename, url, |f, c| {
            super::publish_dbc(f, c)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }) {
            eprintln!("dbc-sync: {filename}: {e}; keeping cached copy");
        }
    }
    for (filename, url) in &vss_files {
        if let Err(e) = mirror_one(&config::vss_dir(), filename, url, |f, c| {
            vss::publish_vss(f, c)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }) {
            eprintln!("dbc-sync: {filename}: {e}; keeping cached copy");
        }
    }

    prune_removed(
        &dictionaries,
        super::list_dbc_files().iter().map(|f| f.filename.clone()),
        |f| {
            let _ = super::delete_dbc(f);
        },
    );
    prune_removed(
        &vss_files,
        vss::list_vss_files().iter().map(|f| f.filename.clone()),
        |f| {
            let _ = vss::delete_vss(f);
        },
    );
    Ok((dictionaries.len(), vss_files.len()))
}

/// GET one directory listing from the GitHub contents API.
fn list_remote(path: &str) -> Result<Vec<ContentsEntry>, String> {
    let url = format!(
        "https://api.github.com/repos/{}/contents/{}?ref={}",
        config::dbc_github_repo(),
        path,
        config::dbc_github_ref()
    );
    let body = http_get(&url)?;
    serde_json::from_str(&body).map_err(|e| format!("contents listing JSON: {e}"))
}

/// Keep only safe, downloadable files accepted by `is_safe`.
fn filter_schemas(
    entries: &[ContentsEntry],
    is_safe: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    entries
        .iter()
        .filter(|e| e.kind == "file" && is_safe(&e.name))
        .filter_map(|e| e.download_url.clone().map(|url| (e.name.clone(), url)))
        .collect()
}

/// Download one schema and store it via `publish` if it changed.
fn mirror_one(
    dir: &std::path::Path,
    filename: &str,
    download_url: &str,
    publish: impl Fn(&str, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let content = http_get(download_url)?;
    let cached = fs::read_to_string(dir.join(filename)).unwrap_or_default();
    if cached == content {
        return Ok(());
    }
    publish(filename, content.as_bytes())
}

/// Delete cached files that no longer exist upstream.
fn prune_removed(
    remote: &[(String, String)],
    local: impl Iterator<Item = String>,
    remove: impl Fn(&str),
) {
    for filename in local {
        if !remote.iter().any(|(name, _)| *name == filename) {
            eprintln!("dbc-sync: pruning {filename} (removed upstream)");
            remove(&filename);
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
        let remote = filter_schemas(&entries, crate::dbc::is_safe_filename);
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].0, "sigma-racer.dbc");
    }

    #[test]
    fn same_listing_yields_yaml_companions_for_vss() {
        let entries = vec![
            entry("sigma-racer.dbc", "file", Some("https://x/sigma-racer.dbc")),
            entry(
                "sigma-racer.yaml",
                "file",
                Some("https://x/sigma-racer.yaml"),
            ),
            entry("m7-draft.yaml", "file", Some("https://x/m7-draft.yaml")),
        ];
        let remote = filter_schemas(&entries, crate::vss::is_safe_filename);
        let names: Vec<_> = remote.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["sigma-racer.yaml", "m7-draft.yaml"]);
    }
}
