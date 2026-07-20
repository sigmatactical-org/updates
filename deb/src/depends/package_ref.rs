//! [`PackageRef`].

use serde::{Deserialize, Serialize};

/// A package name, optionally with an architecture qualifier (`pkg:amd64`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PackageRef {
    pub name: String,
}

impl PackageRef {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
