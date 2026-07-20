//! [`CatalogSpec`] — what makes one directory-backed catalog different.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::list_cache::ListCache;

/// The type-specific half of a file catalog: where its files live, which
/// filenames it accepts, how an entry is described, and how entries are
/// searched and ordered. Everything else (caching, pagination, atomic
/// publish/delete) is generic over this trait in [`crate::listing`].
pub trait CatalogSpec: 'static {
    /// Catalog entry type (one published file).
    type Item: Clone + Send + 'static;

    /// Soft cap for a single published file.
    const MAX_BYTES: u64;

    /// Directory the catalog is served from.
    fn dir() -> PathBuf;

    /// Whether `name` is a plain filename this catalog accepts (no traversal).
    fn is_safe_filename(name: &str) -> bool;

    /// The on-disk filename of a catalog entry.
    fn filename_of(item: &Self::Item) -> &str;

    /// Whether `item` matches a lowercase search needle.
    fn matches(item: &Self::Item, needle: &str) -> bool;

    /// Order a freshly scanned listing.
    fn sort(items: &mut [Self::Item]);

    /// Describe a file on disk, falling back to filename-derived metadata when
    /// the contents cannot be inspected.
    fn describe(path: &Path, filename: &str, size_bytes: u64) -> Self::Item;

    /// Describe a file on disk, rejecting content this catalog will not serve.
    /// Runs on published uploads; the error message is surfaced to the client.
    fn describe_validated(
        path: &Path,
        filename: &str,
        size_bytes: u64,
    ) -> Result<Self::Item, String>;

    /// Process-wide listing cache for this catalog.
    fn cache() -> &'static Mutex<Option<ListCache<Self::Item>>>;
}
