//! [`PushReport`].

use sigma_updates_deb::DebPackage;

/// Outcome of a completed push.
#[derive(Debug, Clone)]
pub struct PushReport {
    pub published: Vec<DebPackage>,
}
