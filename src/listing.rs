//! Directory-backed file catalogs, generic over their item type.
//!
//! The `.deb` package feed, the `.dbc` schema catalog and the VSS mirror are
//! all "a directory of files served with caching, pagination, and atomic
//! publish/delete". Only the differences live in each module's
//! [`CatalogSpec`] impl; the scan/paginate/publish/delete machinery lives here
//! once and is shared by every catalog.

mod catalog_spec;
mod dir_fingerprint;
mod list_cache;
mod page;
mod publish_error;

pub use catalog_spec::CatalogSpec;
pub(crate) use dir_fingerprint::dir_fingerprint;
pub use list_cache::ListCache;
pub use page::Page;
pub use publish_error::PublishError;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bytes::Buf;
use futures_util::{Stream, StreamExt};
use tokio::io::AsyncWriteExt;

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

/// List the catalog directory, through the listing cache.
pub fn list<S: CatalogSpec>() -> Vec<S::Item> {
    list_in::<S>(&S::dir())
}

/// List `dir` as this catalog, rescanning only when the directory changed.
pub fn list_in<S: CatalogSpec>(dir: &Path) -> Vec<S::Item> {
    let cache = S::cache();
    let fingerprint = dir_fingerprint(dir, S::is_safe_filename);
    if let Ok(guard) = cache.lock()
        && let Some(entry) = guard.as_ref()
        && entry.fingerprint == fingerprint
    {
        return entry.items.clone();
    }

    let items = scan::<S>(dir);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(ListCache {
            fingerprint,
            items: items.clone(),
        });
    }
    items
}

/// Scan `dir` for files this catalog accepts, in catalog order.
fn scan<S: CatalogSpec>(dir: &Path) -> Vec<S::Item> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut items: Vec<S::Item> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let path = entry.path();
            let filename = path.file_name()?.to_str()?.to_owned();
            if !S::is_safe_filename(&filename) {
                return None;
            }
            Some(S::describe(&path, &filename, meta.len()))
        })
        .collect();

    S::sort(&mut items);
    items
}

/// Filter + paginate the whole catalog (`page` is 1-based).
pub fn page<S: CatalogSpec>(page: u32, per_page: u32, query: &str) -> Page<S::Item> {
    paginate(&list::<S>(), page, per_page, query, S::matches)
}

/// Filter with `matches` and paginate (`page` is 1-based, `per_page` clamped
/// to [`MAX_PER_PAGE`]). Only the requested page is cloned.
pub fn paginate<T: Clone>(
    all: &[T],
    page: u32,
    per_page: u32,
    query: &str,
    matches: impl Fn(&T, &str) -> bool,
) -> Page<T> {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let q = query.trim();
    let filtered: Vec<&T> = if q.is_empty() {
        all.iter().collect()
    } else {
        let needle = q.to_ascii_lowercase();
        all.iter().filter(|t| matches(t, &needle)).collect()
    };

    let total = filtered.len();
    let total_pages = if total == 0 {
        1
    } else {
        (total as u32).div_ceil(per_page)
    };
    let page = page.clamp(1, total_pages);
    let start = (((page - 1) * per_page) as usize).min(total);
    let end = start.saturating_add(per_page as usize).min(total);
    let items = filtered[start..end].iter().map(|t| (*t).clone()).collect();

    Page {
        items,
        total,
        page,
        per_page,
        total_pages,
        query: q.to_owned(),
    }
}

/// Resolve a downloadable path in this catalog; rejects path traversal.
pub fn path<S: CatalogSpec>(filename: &str) -> Option<PathBuf> {
    if !S::is_safe_filename(filename) {
        return None;
    }
    let path = S::dir().join(filename);
    if path.is_file() { Some(path) } else { None }
}

/// Publish in-memory `bytes` (small mirrored files); validates before the
/// rename, so a rejected upload never becomes visible.
pub fn publish<S: CatalogSpec>(filename: &str, bytes: &[u8]) -> Result<S::Item, PublishError> {
    if !S::is_safe_filename(filename) {
        return Err(PublishError::InvalidFilename);
    }
    if bytes.is_empty() {
        return Err(PublishError::EmptyBody);
    }
    if bytes.len() as u64 > S::MAX_BYTES {
        return Err(PublishError::TooLarge);
    }

    let dir = S::dir();
    let (dest, tmp) = (dir.join(filename), tmp_path(&dir, filename));
    fs::create_dir_all(&dir)?;
    write_tmp(&tmp, bytes)?;

    let described = S::describe_validated(&tmp, filename, bytes.len() as u64);
    finish_publish::<S>(described, &tmp, &dest)
}

/// Stream an upload straight to disk, then validate and publish it atomically.
///
/// Nothing larger than one chunk is ever held in memory, and the (blocking,
/// potentially expensive) archive inspection runs on the blocking pool.
pub async fn publish_stream<S, St, B, E>(
    filename: &str,
    mut body: St,
) -> Result<S::Item, PublishError>
where
    S: CatalogSpec,
    St: Stream<Item = Result<B, E>> + Unpin,
    B: Buf,
    E: std::fmt::Display,
{
    if !S::is_safe_filename(filename) {
        return Err(PublishError::InvalidFilename);
    }

    let dir = S::dir();
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(filename);
    let tmp = tmp_path(&dir, filename);

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut written: u64 = 0;
    while let Some(chunk) = body.next().await {
        let mut chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                return abort_publish(
                    file,
                    &tmp,
                    PublishError::InvalidContent(format!("upload aborted: {err}")),
                )
                .await;
            }
        };
        written += chunk.remaining() as u64;
        if written > S::MAX_BYTES {
            return abort_publish(file, &tmp, PublishError::TooLarge).await;
        }
        while chunk.has_remaining() {
            let part = chunk.chunk();
            file.write_all(part).await?;
            let n = part.len();
            chunk.advance(n);
        }
    }
    if written == 0 {
        return abort_publish(file, &tmp, PublishError::EmptyBody).await;
    }
    file.sync_all().await?;
    drop(file);

    let (inspect_tmp, inspect_name) = (tmp.clone(), filename.to_owned());
    let described = tokio::task::spawn_blocking(move || {
        S::describe_validated(&inspect_tmp, &inspect_name, written)
    })
    .await
    .unwrap_or_else(|e| Err(format!("inspection failed: {e}")));

    finish_publish::<S>(described, &tmp, &dest)
}

/// Remove a published file.
pub fn delete<S: CatalogSpec>(filename: &str) -> Result<(), PublishError> {
    let Some(path) = path::<S>(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    invalidate::<S>();
    Ok(())
}

/// Drop the listing cache (after publish/delete).
pub fn invalidate<S: CatalogSpec>() {
    if let Ok(mut guard) = S::cache().lock() {
        *guard = None;
    }
}

fn tmp_path(dir: &Path, filename: &str) -> PathBuf {
    dir.join(format!(".{filename}.tmp"))
}

fn write_tmp(tmp: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    let mut file = fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

/// Discard a half-written upload and report why.
async fn abort_publish<T>(
    file: tokio::fs::File,
    tmp: &Path,
    err: PublishError,
) -> Result<T, PublishError> {
    drop(file);
    let _ = tokio::fs::remove_file(tmp).await;
    Err(err)
}

/// Rename a validated temp file into place (or clean it up on rejection).
fn finish_publish<S: CatalogSpec>(
    described: Result<S::Item, String>,
    tmp: &Path,
    dest: &Path,
) -> Result<S::Item, PublishError> {
    match described {
        Ok(item) => {
            fs::rename(tmp, dest)?;
            invalidate::<S>();
            Ok(item)
        }
        Err(msg) => {
            let _ = fs::remove_file(tmp);
            Err(PublishError::InvalidContent(msg))
        }
    }
}
