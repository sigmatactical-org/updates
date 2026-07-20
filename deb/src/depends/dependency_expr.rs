//! [`DependencyExpr`].

use std::fmt;

use serde::{Deserialize, Serialize};

use super::dependency_clause::DependencyClause;

/// AND-combined dependency expression (one comma-separated entry).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyExpr {
    pub alternatives: Vec<DependencyClause>,
}

impl DependencyExpr {
    /// True when at least one alternative is satisfied by `available`.
    pub fn is_satisfied_by<'a, I>(&self, available: I) -> bool
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let pkgs: Vec<(&str, &str)> = available.into_iter().collect();
        self.alternatives.iter().any(|alt| {
            pkgs.iter()
                .any(|(name, ver)| name == &alt.package.name && alt.constraint.matches(ver))
        })
    }
}

impl fmt::Display for DependencyExpr {
    /// Debian control syntax: alternatives joined by ` | `.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, alt) in self.alternatives.iter().enumerate() {
            if i > 0 {
                write!(f, " | ")?;
            }
            write!(f, "{alt}")?;
        }
        Ok(())
    }
}
