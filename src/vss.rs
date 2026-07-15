//! Scan and serve mirrored VSS schemas (signal tree + CAN→VSS mapping docs).

mod vss_file;

pub use vss_file::VssFile;

use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::listing::{self, PublishError};

/// Extensions that belong in the VSS catalog.
pub const VSS_EXTENSIONS: &[&str] = &[".vspec", ".yaml", ".yml", ".json"];

/// Soft cap for a single mirrored VSS file (16 MiB).
pub const MAX_VSS_BYTES: u64 = 16 * 1024 * 1024;

/// One page of the VSS catalog.
pub type VssPage = listing::Page<VssFile>;

/// List every VSS file in [`config::vss_dir`], sorted by name.
pub fn list_vss_files() -> Vec<VssFile> {
    list_vss_files_in(&config::vss_dir())
}

/// Prefer the cluster signal tree, then the Sigma Racer mapping, then the
/// first catalog entry.
pub fn latest_vss_file() -> Option<VssFile> {
    let files = list_vss_files();
    files
        .iter()
        .find(|f| f.filename == "sigma-cluster.vspec")
        .or_else(|| files.iter().find(|f| f.name == "sigma-racer"))
        .cloned()
        .or_else(|| files.into_iter().next())
}

/// Filter + paginate the VSS catalog (`page` is 1-based).
pub fn list_vss_files_page(page: u32, per_page: u32, query: &str) -> VssPage {
    let all = list_vss_files();
    listing::paginate(&all, page, per_page, query, vss_matches)
}

/// Whether a VSS entry matches a lowercase search needle.
fn vss_matches(file: &VssFile, needle: &str) -> bool {
    file.name.to_ascii_lowercase().contains(needle)
        || file.filename.to_ascii_lowercase().contains(needle)
}

/// Scan `dir` for well-named VSS files, sorted by name. The catalog is a
/// handful of files, so no fingerprint cache like the package feed.
pub fn list_vss_files_in(dir: &Path) -> Vec<VssFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<VssFile> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let filename = path.file_name()?.to_str()?.to_owned();
            if !is_safe_filename(&filename) {
                return None;
            }
            Some(describe_path(&filename, meta.len()))
        })
        .collect();

    files.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.filename.cmp(&b.filename))
    });
    files
}

/// Resolve a downloadable VSS path; rejects path traversal.
pub fn vss_path(filename: &str) -> Option<PathBuf> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = config::vss_dir().join(filename);
    if path.is_file() { Some(path) } else { None }
}

/// Write (or replace) a VSS file under the catalog directory.
pub fn publish_vss(filename: &str, bytes: &[u8]) -> Result<VssFile, PublishError> {
    if !is_safe_filename(filename) {
        return Err(PublishError::InvalidFilename);
    }
    if bytes.is_empty() {
        return Err(PublishError::EmptyBody);
    }
    if bytes.len() as u64 > MAX_VSS_BYTES {
        return Err(PublishError::TooLarge);
    }
    std::str::from_utf8(bytes)
        .map_err(|e| PublishError::InvalidContent(format!("not UTF-8: {e}")))?;

    listing::atomic_publish(&config::vss_dir(), filename, bytes)?;
    Ok(describe_path(filename, bytes.len() as u64))
}

/// Remove a mirrored VSS file.
pub fn delete_vss(filename: &str) -> Result<(), PublishError> {
    let Some(path) = vss_path(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    Ok(())
}

/// Whether `name` is a plain filename with a VSS extension (no traversal).
pub fn is_safe_filename(name: &str) -> bool {
    VSS_EXTENSIONS
        .iter()
        .any(|ext| listing::is_safe_filename(name, ext))
}

/// Catalog entry for a file on disk.
fn describe_path(filename: &str, size_bytes: u64) -> VssFile {
    let name = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .to_owned();
    VssFile {
        filename: filename.to_owned(),
        name,
        size_bytes,
        download_path: format!("/vss/{filename}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_vss_extensions_only() {
        assert!(is_safe_filename("sigma-cluster.vspec"));
        assert!(is_safe_filename("sigma-racer.yaml"));
        assert!(!is_safe_filename("sigma-racer.dbc"));
        assert!(!is_safe_filename("../evil.yaml"));
        assert!(!is_safe_filename("a/b.vspec"));
    }

    #[test]
    fn rejects_traversal() {
        assert!(vss_path("../etc/passwd.yaml").is_none());
        assert!(vss_path("foo/bar.vspec").is_none());
    }
}
