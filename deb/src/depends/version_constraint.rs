//! [`VersionConstraint`].

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VersionConstraint {
    /// No version constraint.
    Any,
    Eq(String),
    Ne(String),
    Gt(String),
    Ge(String),
    Lt(String),
    Le(String),
}

impl VersionConstraint {
    pub fn matches(&self, version: &str) -> bool {
        use debversion::Version;
        let Ok(have) = version.parse::<Version>() else {
            return matches!(self, Self::Any) || matches!(self, Self::Eq(v) if v == version);
        };
        match self {
            Self::Any => true,
            Self::Eq(v) => v.parse::<Version>().ok().is_some_and(|need| have == need),
            Self::Ne(v) => v.parse::<Version>().ok().is_some_and(|need| have != need),
            Self::Gt(v) => v.parse::<Version>().ok().is_some_and(|need| have > need),
            Self::Ge(v) => v.parse::<Version>().ok().is_some_and(|need| have >= need),
            Self::Lt(v) => v.parse::<Version>().ok().is_some_and(|need| have < need),
            Self::Le(v) => v.parse::<Version>().ok().is_some_and(|need| have <= need),
        }
    }

    /// Debian relation operator and version, if constrained (`(">=", "1.0")`).
    pub(crate) fn op_version(&self) -> Option<(&'static str, &str)> {
        match self {
            Self::Any => None,
            Self::Eq(v) => Some(("=", v)),
            Self::Ne(v) => Some(("!=", v)),
            Self::Gt(v) => Some((">>", v)),
            Self::Ge(v) => Some((">=", v)),
            Self::Lt(v) => Some(("<<", v)),
            Self::Le(v) => Some(("<=", v)),
        }
    }
}
