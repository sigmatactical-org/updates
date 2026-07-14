//! Scan and serve Sigma Racer `.dbc` schema files from the local catalog directory.

mod dbc_file;

pub use dbc_file::DbcFile;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config;
use crate::listing::{self, ListCache};

pub use crate::listing::PublishError;

pub use crate::listing::DEFAULT_PER_PAGE;

/// Soft cap for a single uploaded `.dbc` (16 MiB).
pub const MAX_DBC_BYTES: u64 = 16 * 1024 * 1024;

/// One page of the DBC catalog.
pub type DbcPage = listing::Page<DbcFile>;

static LIST_CACHE: Mutex<Option<ListCache<DbcFile>>> = Mutex::new(None);

/// List every `*.dbc` in [`config::dbc_dir`], sorted by name.
pub fn list_dbc_files() -> Vec<DbcFile> {
    list_dbc_files_in(&config::dbc_dir())
}

/// Prefer the Sigma Racer schema, otherwise the first catalog entry.
pub fn latest_dbc_file() -> Option<DbcFile> {
    let files = list_dbc_files();
    files
        .iter()
        .find(|f| {
            f.filename == "sigma-racer.dbc"
                || f.name == "sigma-racer"
                || f.filename == "m7-draft.dbc"
                || f.name == "m7-draft"
        })
        .cloned()
        .or_else(|| files.into_iter().next())
}

/// Filter + paginate the DBC catalog (`page` is 1-based).
pub fn list_dbc_files_page(page: u32, per_page: u32, query: &str) -> DbcPage {
    let all = list_dbc_files();
    paginate_dbc_files(&all, page, per_page, query)
}

/// Filter with the DBC match predicate and paginate.
pub fn paginate_dbc_files(all: &[DbcFile], page: u32, per_page: u32, query: &str) -> DbcPage {
    listing::paginate(all, page, per_page, query, dbc_matches)
}

/// Whether a DBC entry matches a lowercase search needle.
fn dbc_matches(file: &DbcFile, needle: &str) -> bool {
    file.name.to_ascii_lowercase().contains(needle)
        || file.filename.to_ascii_lowercase().contains(needle)
}

/// List `dir` through the fingerprint cache.
pub fn list_dbc_files_in(dir: &Path) -> Vec<DbcFile> {
    listing::cached_list(&LIST_CACHE, dir, "dbc", scan_dbc_files)
}

/// Scan `dir` for well-named `.dbc` files, sorted by name.
fn scan_dbc_files(dir: &Path) -> Vec<DbcFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<DbcFile> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("dbc"))
        })
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
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

/// Resolve a downloadable DBC path; rejects path traversal.
pub fn dbc_path(filename: &str) -> Option<PathBuf> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = config::dbc_dir().join(filename);
    if path.is_file() { Some(path) } else { None }
}

/// Write (or replace) a `.dbc` under the catalog directory.
pub fn publish_dbc(filename: &str, bytes: &[u8]) -> Result<DbcFile, PublishError> {
    if !is_safe_filename(filename) {
        return Err(PublishError::InvalidFilename);
    }
    if bytes.is_empty() {
        return Err(PublishError::EmptyBody);
    }
    if bytes.len() as u64 > MAX_DBC_BYTES {
        return Err(PublishError::TooLarge);
    }
    let content = std::str::from_utf8(bytes)
        .map_err(|e| PublishError::InvalidContent(format!("not UTF-8: {e}")))?;
    if !content.contains("VERSION") || !content.contains("BO_") {
        return Err(PublishError::InvalidContent(
            "missing VERSION or BO_ sections".into(),
        ));
    }

    listing::atomic_publish(&config::dbc_dir(), filename, bytes)?;
    listing::invalidate(&LIST_CACHE);
    Ok(describe_path(filename, bytes.len() as u64))
}

/// Remove a published `.dbc`.
pub fn delete_dbc(filename: &str) -> Result<(), PublishError> {
    let Some(path) = dbc_path(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    listing::invalidate(&LIST_CACHE);
    Ok(())
}

/// Whether `name` is a plain `.dbc` filename (no path traversal).
pub fn is_safe_filename(name: &str) -> bool {
    listing::is_safe_filename(name, ".dbc")
}

/// Catalog entry for a file on disk.
fn describe_path(filename: &str, size_bytes: u64) -> DbcFile {
    let name = filename.trim_end_matches(".dbc").to_owned();
    DbcFile {
        filename: filename.to_owned(),
        name,
        size_bytes,
        download_path: format!("/dbc/{filename}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub(name: &str) -> DbcFile {
        DbcFile {
            filename: format!("{name}.dbc"),
            name: name.into(),
            size_bytes: 1,
            download_path: format!("/dbc/{name}.dbc"),
        }
    }

    #[test]
    fn rejects_traversal() {
        assert!(dbc_path("../etc/passwd.dbc").is_none());
        assert!(dbc_path("foo/bar.dbc").is_none());
    }

    #[test]
    fn paginates_and_filters() {
        let all: Vec<_> = (0..120).map(|i| stub(&format!("schema-{i:03}"))).collect();
        let page = paginate_dbc_files(&all, 2, 50, "");
        assert_eq!(page.total, 120);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.items.len(), 50);
        assert_eq!(page.items[0].name, "schema-050");

        let filtered = paginate_dbc_files(&all, 1, 50, "schema-11");
        assert_eq!(filtered.total, 10);
        assert!(filtered.items.iter().all(|f| f.name.contains("schema-11")));
    }
}
