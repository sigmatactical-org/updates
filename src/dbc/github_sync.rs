//! Mirror schema catalogs from their single source of truth on GitHub.
//!
//! The catalog directories are only local caches: a background thread lists
//! the configured repo paths via the GitHub contents API, downloads every
//! schema, and prunes local files that no longer exist upstream. Two catalogs
//! are mirrored — CAN dictionaries (`.dbc` from the DBC path) and VSS files
//! (`schemas/vss/*` plus the `.yaml` CAN-mapping companions that live next to
//! the DBCs). When GitHub is unreachable the cached copies keep being served.

mod contents_entry;

use contents_entry::ContentsEntry;

use std::fs;
use std::thread;
use std::time::Duration;

use sigma_updates_client::shared_agent;

use crate::config;
use crate::listing::{self, CatalogSpec};
use crate::vss::VssCatalog;

use super::DbcCatalog;

/// Retry pace until the first successful pass. Startup loses the race
/// against the Istio sidecar, and waiting a full sync interval would leave
/// fresh pods serving an empty catalog for minutes.
const FIRST_SYNC_RETRY: Duration = Duration::from_secs(5);

/// Spawn the background mirror thread; the first pass runs immediately and
/// retries quickly until it succeeds once.
pub fn spawn() {
    thread::Builder::new()
        .name("dbc-github-sync".into())
        .spawn(|| {
            let mut synced_once = false;
            loop {
                match sync_once() {
                    Ok((dbc, vss)) => {
                        synced_once = true;
                        eprintln!(
                            "dbc-sync: {dbc} dictionary(ies) + {vss} VSS file(s) mirrored from {}",
                            config::dbc_github_source()
                        );
                    }
                    Err(e) => eprintln!(
                        "dbc-sync: {e}; serving cached copies from {}",
                        config::dbc_dir().display()
                    ),
                }
                thread::sleep(if synced_once {
                    config::dbc_sync_interval()
                } else {
                    FIRST_SYNC_RETRY
                });
            }
        })
        .expect("spawn dbc-github-sync thread");
}

/// One full mirror pass; returns upstream (dictionary, VSS) counts.
fn sync_once() -> Result<(usize, usize), String> {
    let can_listing = list_remote(&config::dbc_github_path())?;
    let vss_listing = list_remote(&config::vss_github_path())?;

    let dictionaries = filter_schemas(&can_listing, DbcCatalog::is_safe_filename);
    // The VSS catalog is the signal tree plus the CAN-mapping yaml companions
    // that live in the DBC directory.
    let mut vss_files = filter_schemas(&vss_listing, VssCatalog::is_safe_filename);
    vss_files.extend(filter_schemas(&can_listing, VssCatalog::is_safe_filename));

    mirror_all::<DbcCatalog>(&dictionaries);
    mirror_all::<VssCatalog>(&vss_files);

    prune_removed::<DbcCatalog>(&dictionaries);
    prune_removed::<VssCatalog>(&vss_files);
    Ok((dictionaries.len(), vss_files.len()))
}

/// Download and publish every listed file into `S`, keeping the cached copy
/// when a download or validation fails.
fn mirror_all<S: CatalogSpec>(remote: &[(String, String)]) {
    let dir = S::dir();
    for (filename, url) in remote {
        if let Err(e) = mirror_one::<S>(&dir, filename, url) {
            eprintln!("dbc-sync: {filename}: {e}; keeping cached copy");
        }
    }
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

/// Download one schema and publish it if it changed.
fn mirror_one<S: CatalogSpec>(
    dir: &std::path::Path,
    filename: &str,
    download_url: &str,
) -> Result<(), String> {
    let content = http_get(download_url)?;
    let cached = fs::read_to_string(dir.join(filename)).unwrap_or_default();
    if cached == content {
        return Ok(());
    }
    listing::publish::<S>(filename, content.as_bytes())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Delete cached files that no longer exist upstream.
fn prune_removed<S: CatalogSpec>(remote: &[(String, String)]) {
    for item in listing::list::<S>() {
        let filename = S::filename_of(&item);
        if !remote.iter().any(|(name, _)| name == filename) {
            eprintln!("dbc-sync: pruning {filename} (removed upstream)");
            let _ = listing::delete::<S>(filename);
        }
    }
}

/// GET a URL as text with GitHub-friendly headers, reusing the process-wide
/// HTTP agent (connection pool + TLS session reuse).
fn http_get(url: &str) -> Result<String, String> {
    let mut request = shared_agent()
        .get(url)
        .header("User-Agent", "sigma-updates")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = config::github_token() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let mut response = request.call().map_err(|e| format!("GET {url}: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read {url}: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("GET {url}: status {status}: {body}"));
    }
    Ok(body)
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
        let remote = filter_schemas(&entries, DbcCatalog::is_safe_filename);
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
        let remote = filter_schemas(&entries, VssCatalog::is_safe_filename);
        let names: Vec<_> = remote.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["sigma-racer.yaml", "m7-draft.yaml"]);
    }
}
