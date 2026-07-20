//! [`VssCatalog`] — the mirrored VSS schema catalog.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config;
use crate::listing::{CatalogSpec, ListCache, is_safe_filename};

use super::vss_file::VssFile;

/// Extensions that belong in the VSS catalog.
pub const VSS_EXTENSIONS: &[&str] = &[".vspec", ".yaml", ".yml", ".json"];

/// The VSS mirror (signal tree + CAN mapping docs) in [`config::vss_dir`].
pub struct VssCatalog;

static CACHE: Mutex<Option<ListCache<VssFile>>> = ListCache::empty();

impl CatalogSpec for VssCatalog {
    type Item = VssFile;

    /// Soft cap for a single mirrored VSS file (16 MiB).
    const MAX_BYTES: u64 = 16 * 1024 * 1024;

    fn dir() -> PathBuf {
        config::vss_dir()
    }

    fn is_safe_filename(name: &str) -> bool {
        VSS_EXTENSIONS.iter().any(|ext| is_safe_filename(name, ext))
    }

    fn filename_of(file: &VssFile) -> &str {
        &file.filename
    }

    fn matches(file: &VssFile, needle: &str) -> bool {
        file.name.to_ascii_lowercase().contains(needle)
            || file.filename.to_ascii_lowercase().contains(needle)
    }

    fn sort(files: &mut [VssFile]) {
        files.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.filename.cmp(&b.filename))
        });
    }

    fn describe(_path: &Path, filename: &str, size_bytes: u64) -> VssFile {
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

    /// Requires UTF-8 text (the catalog serves `.vspec`/`.yaml`/`.json`).
    fn describe_validated(path: &Path, filename: &str, size_bytes: u64) -> Result<VssFile, String> {
        let content = std::fs::read(path).map_err(|e| e.to_string())?;
        std::str::from_utf8(&content).map_err(|e| format!("not UTF-8: {e}"))?;
        Ok(Self::describe(path, filename, size_bytes))
    }

    fn cache() -> &'static Mutex<Option<ListCache<VssFile>>> {
        &CACHE
    }
}
