//! Fingerprint-keyed cache of a scanned directory listing.

use super::dir_fingerprint::DirFingerprint;

/// A scanned listing plus the fingerprint it was taken at.
pub struct ListCache<T> {
    /// Directory state when `items` was scanned.
    pub(crate) fingerprint: DirFingerprint,
    /// The cached listing.
    pub(crate) items: Vec<T>,
}

impl<T> ListCache<T> {
    /// An empty, never-populated cache slot.
    pub const fn empty() -> std::sync::Mutex<Option<Self>> {
        std::sync::Mutex::new(None)
    }
}
