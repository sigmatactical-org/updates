//! Scan and serve Sigma Racer `.dbc` schema files from the local catalog directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;
use thiserror::Error;

use crate::config;

#[derive(Debug, Clone, Serialize)]
pub struct DbcFile {
    pub filename: String,
    pub name: String,
    pub size_bytes: u64,
    pub download_path: String,
}

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("invalid DBC filename")]
    InvalidFilename,
    #[error("empty body")]
    EmptyBody,
    #[error("file too large")]
    TooLarge,
    #[error("DBC file not found")]
    NotFound,
    #[error("invalid DBC content")]
    InvalidContent(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Soft cap for a single uploaded `.dbc` (16 MiB).
pub const MAX_DBC_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirFingerprint {
    count: u64,
    total_bytes: u64,
    newest_mtime_secs: u64,
}

struct ListCache {
    fingerprint: DirFingerprint,
    files: Vec<DbcFile>,
}

static LIST_CACHE: Mutex<Option<ListCache>> = Mutex::new(None);

fn invalidate_list_cache() {
    if let Ok(mut guard) = LIST_CACHE.lock() {
        *guard = None;
    }
}

fn dir_fingerprint(dir: &Path) -> DirFingerprint {
    let Ok(entries) = fs::read_dir(dir) else {
        return DirFingerprint {
            count: 0,
            total_bytes: 0,
            newest_mtime_secs: 0,
        };
    };
    let mut count = 0u64;
    let mut total_bytes = 0u64;
    let mut newest_mtime_secs = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dbc"))
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        count += 1;
        total_bytes += meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        newest_mtime_secs = newest_mtime_secs.max(mtime);
    }
    DirFingerprint {
        count,
        total_bytes,
        newest_mtime_secs,
    }
}

/// Default page size for the JSON API.
pub const DEFAULT_PER_PAGE: u32 = 50;
/// Hard cap so a single response cannot dump the entire catalog.
pub const MAX_PER_PAGE: u32 = 500;

/// One page of the DBC catalog.
#[derive(Debug, Clone)]
pub struct DbcPage {
    pub files: Vec<DbcFile>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub query: String,
}

/// List every `*.dbc` in [`config::dbc_dir`], sorted by name.
pub fn list_dbc_files() -> Vec<DbcFile> {
    list_dbc_files_in(&config::dbc_dir())
}

/// Filter + paginate the DBC catalog (`page` is 1-based).
pub fn list_dbc_files_page(page: u32, per_page: u32, query: &str) -> DbcPage {
    let all = list_dbc_files();
    paginate_dbc_files(&all, page, per_page, query)
}

pub fn paginate_dbc_files(all: &[DbcFile], page: u32, per_page: u32, query: &str) -> DbcPage {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let q = query.trim();
    let filtered: Vec<DbcFile> = if q.is_empty() {
        all.to_vec()
    } else {
        let needle = q.to_ascii_lowercase();
        all.iter()
            .filter(|f| dbc_matches(f, &needle))
            .cloned()
            .collect()
    };

    let total = filtered.len();
    let total_pages = if total == 0 {
        1
    } else {
        (total as u32).div_ceil(per_page)
    };
    let page = page.clamp(1, total_pages);
    let start = ((page - 1) * per_page) as usize;
    let files = filtered
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();

    DbcPage {
        files,
        total,
        page,
        per_page,
        total_pages,
        query: q.to_owned(),
    }
}

fn dbc_matches(file: &DbcFile, needle: &str) -> bool {
    file.name.to_ascii_lowercase().contains(needle)
        || file.filename.to_ascii_lowercase().contains(needle)
}

pub fn list_dbc_files_in(dir: &Path) -> Vec<DbcFile> {
    let fingerprint = dir_fingerprint(dir);
    if let Ok(guard) = LIST_CACHE.lock()
        && let Some(cache) = guard.as_ref()
        && cache.fingerprint == fingerprint
    {
        return cache.files.clone();
    }

    let files = scan_dbc_files(dir);
    if let Ok(mut guard) = LIST_CACHE.lock() {
        *guard = Some(ListCache {
            fingerprint,
            files: files.clone(),
        });
    }
    files
}

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

    files.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.filename.cmp(&b.filename)));
    files
}

/// Resolve a downloadable DBC path; rejects path traversal.
pub fn dbc_path(filename: &str) -> Option<PathBuf> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = config::dbc_dir().join(filename);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
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

    let dir = config::dbc_dir();
    fs::create_dir_all(&dir)?;
    let dest = dir.join(filename);
    let tmp = dir.join(format!(".{filename}.tmp"));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, &dest)?;
    invalidate_list_cache();
    Ok(describe_path(filename, bytes.len() as u64))
}

/// Remove a published `.dbc`.
pub fn delete_dbc(filename: &str) -> Result<(), PublishError> {
    let Some(path) = dbc_path(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    invalidate_list_cache();
    Ok(())
}

pub fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name.ends_with(".dbc")
}

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
        let all: Vec<_> = (0..120)
            .map(|i| stub(&format!("schema-{i:03}")))
            .collect();
        let page = paginate_dbc_files(&all, 2, 50, "");
        assert_eq!(page.total, 120);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.files.len(), 50);
        assert_eq!(page.files[0].name, "schema-050");

        let filtered = paginate_dbc_files(&all, 1, 50, "schema-11");
        assert_eq!(filtered.total, 10);
        assert!(filtered
            .files
            .iter()
            .all(|f| f.name.contains("schema-11")));
    }
}
