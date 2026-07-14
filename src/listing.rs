//! Shared machinery for directory-backed file catalogs.
//!
//! Both the `.deb` package feed and the `.dbc` schema catalog are "a directory
//! of files served with caching, pagination, and atomic publish/delete". The
//! type-specific parts (item struct, match predicate, content validation) stay
//! in [`crate::packages`] / [`crate::dbc`]; everything else lives here once.

mod dir_fingerprint;
mod list_cache;
mod page;
mod publish_error;

pub(crate) use dir_fingerprint::dir_fingerprint;
pub(crate) use list_cache::ListCache;
pub use page::Page;
pub use publish_error::PublishError;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Default page size for the HTML UI and JSON API.
pub const DEFAULT_PER_PAGE: u32 = 50;
/// Hard cap so a single response cannot dump an entire multi-thousand feed.
pub const MAX_PER_PAGE: u32 = 500;

/// Whether `name` is a plain filename with the expected extension — rejects
/// path traversal and separators.
pub fn is_safe_filename(name: &str, ext: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name.to_ascii_lowercase().ends_with(ext)
}

/// List `dir` through the fingerprint-keyed `cache`, rescanning with `scan`
/// only when the directory contents changed.
pub(crate) fn cached_list<T: Clone>(
    cache: &Mutex<Option<ListCache<T>>>,
    dir: &Path,
    ext: &str,
    scan: impl FnOnce(&Path) -> Vec<T>,
) -> Vec<T> {
    let fingerprint = dir_fingerprint(dir, ext);
    if let Ok(guard) = cache.lock()
        && let Some(entry) = guard.as_ref()
        && entry.fingerprint == fingerprint
    {
        return entry.items.clone();
    }

    let items = scan(dir);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(ListCache {
            fingerprint,
            items: items.clone(),
        });
    }
    items
}

/// Drop a list cache (after publish/delete).
pub(crate) fn invalidate<T>(cache: &Mutex<Option<ListCache<T>>>) {
    if let Ok(mut guard) = cache.lock() {
        *guard = None;
    }
}

/// Filter with `matches` and paginate (`page` is 1-based, `per_page` clamped
/// to [`MAX_PER_PAGE`]).
pub fn paginate<T: Clone>(
    all: &[T],
    page: u32,
    per_page: u32,
    query: &str,
    matches: impl Fn(&T, &str) -> bool,
) -> Page<T> {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let q = query.trim();
    let filtered: Vec<T> = if q.is_empty() {
        all.to_vec()
    } else {
        let needle = q.to_ascii_lowercase();
        all.iter()
            .filter(|t| matches(t, &needle))
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
    let items = filtered
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();

    Page {
        items,
        total,
        page,
        per_page,
        total_pages,
        query: q.to_owned(),
    }
}

/// Write `bytes` to `dir/filename` atomically (temp file + fsync + rename).
pub fn atomic_publish(dir: &Path, filename: &str, bytes: &[u8]) -> Result<PathBuf, PublishError> {
    fs::create_dir_all(dir)?;
    let dest = dir.join(filename);
    let tmp = dir.join(format!(".{filename}.tmp"));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &dest)?;
    Ok(dest)
}
