//! Fingerprint-keyed cache of a scanned directory listing.

use super::dir_fingerprint::DirFingerprint;

/// A scanned listing plus the fingerprint it was taken at.
pub(crate) struct ListCache<T> {
    /// Directory state when `items` was scanned.
    pub(crate) fingerprint: DirFingerprint,
    /// The cached listing.
    pub(crate) items: Vec<T>,
}
