//! [`LocalPackage`].

use std::path::PathBuf;

use sigma_updates_deb::DebControl;

/// A `.deb` on the local filesystem, with its parsed control metadata.
#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub path: PathBuf,
    pub filename: String,
    pub control: DebControl,
}
