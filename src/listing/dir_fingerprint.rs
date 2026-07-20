//! Cheap change detection for a catalog directory.

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Summary of a directory's matching files; equality means "unchanged".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirFingerprint {
    /// Number of matching files.
    pub(crate) count: u64,
    /// Sum of their sizes.
    pub(crate) total_bytes: u64,
    /// Most recent mtime (seconds since the epoch).
    pub(crate) newest_mtime_secs: u64,
}

/// Fingerprint every file in `dir` whose name is `accept`ed.
pub(crate) fn dir_fingerprint(dir: &Path, accept: impl Fn(&str) -> bool) -> DirFingerprint {
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
        if !entry.file_name().to_str().is_some_and(&accept) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
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
