//! [`DbcCatalog`] — the CAN dictionary catalog.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config;
use crate::listing::{CatalogSpec, ListCache, is_safe_filename};

use super::dbc_file::DbcFile;

/// The `.dbc` catalog mirrored into [`config::dbc_dir`].
pub struct DbcCatalog;

static CACHE: Mutex<Option<ListCache<DbcFile>>> = ListCache::empty();

impl CatalogSpec for DbcCatalog {
    type Item = DbcFile;

    /// Soft cap for a single `.dbc` (16 MiB).
    const MAX_BYTES: u64 = 16 * 1024 * 1024;

    fn dir() -> PathBuf {
        config::dbc_dir()
    }

    fn is_safe_filename(name: &str) -> bool {
        is_safe_filename(name, ".dbc")
    }

    fn filename_of(file: &DbcFile) -> &str {
        &file.filename
    }

    fn matches(file: &DbcFile, needle: &str) -> bool {
        file.name.to_ascii_lowercase().contains(needle)
            || file.filename.to_ascii_lowercase().contains(needle)
    }

    fn sort(files: &mut [DbcFile]) {
        files.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.filename.cmp(&b.filename))
        });
    }

    fn describe(_path: &Path, filename: &str, size_bytes: u64) -> DbcFile {
        DbcFile {
            filename: filename.to_owned(),
            name: filename.trim_end_matches(".dbc").to_owned(),
            size_bytes,
            download_path: format!("/dbc/{filename}"),
        }
    }

    /// Requires UTF-8 text with the `VERSION` and `BO_` sections every CAN
    /// dictionary carries.
    fn describe_validated(path: &Path, filename: &str, size_bytes: u64) -> Result<DbcFile, String> {
        let content = std::fs::read(path).map_err(|e| e.to_string())?;
        let text = std::str::from_utf8(&content).map_err(|e| format!("not UTF-8: {e}"))?;
        if !text.contains("VERSION") || !text.contains("BO_") {
            return Err("missing VERSION or BO_ sections".into());
        }
        Ok(Self::describe(path, filename, size_bytes))
    }

    fn cache() -> &'static Mutex<Option<ListCache<DbcFile>>> {
        &CACHE
    }
}
