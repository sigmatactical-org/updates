//! Parse Debian binary package (`.deb`) control metadata.

#![forbid(unsafe_code)]

mod control;
mod deb_package;
mod depends;
mod human_size;
mod read;

pub use control::DebControl;
pub use deb_package::DebPackage;
pub use depends::{
    DependencyClause, DependencyExpr, PackageRef, VersionConstraint, parse_depends_field, satisfies,
};
pub use human_size::human_size;
pub use read::{DebError, inspect_deb_bytes, inspect_deb_file, inspect_deb_read};
