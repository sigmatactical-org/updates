//! [`DependencyClause`].

use std::fmt;

use serde::{Deserialize, Serialize};

use super::package_ref::PackageRef;
use super::version_constraint::VersionConstraint;

/// One alternative in a dependency clause (`pkg (>= 1) | other`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyClause {
    pub package: PackageRef,
    pub constraint: VersionConstraint,
}

impl fmt::Display for DependencyClause {
    /// Debian control syntax: `pkg` or `pkg (>= 1.0)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.package.name)?;
        if let Some((op, version)) = self.constraint.op_version() {
            write!(f, " ({op} {version})")?;
        }
        Ok(())
    }
}
