//! Parse Debian binary package (`.deb`) control metadata.

#![forbid(unsafe_code)]

mod control;
mod depends;
mod read;

pub use control::DebControl;
pub use depends::{
    DependencyClause, DependencyExpr, PackageRef, VersionConstraint, parse_depends_field, satisfies,
};
pub use read::{DebError, inspect_deb_bytes, inspect_deb_file};
