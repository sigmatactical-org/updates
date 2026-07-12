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
}

/// One alternative in a dependency clause (`pkg (>= 1) | other`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyClause {
    pub package: PackageRef,
    pub constraint: VersionConstraint,
}

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

/// Parse a Debian `Depends` / `Pre-Depends` field into AND-of-OR expressions.
pub fn parse_depends_field(field: &str) -> Vec<DependencyExpr> {
    field
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|and_group| DependencyExpr {
            alternatives: and_group
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .filter_map(parse_clause)
                .collect(),
        })
        .filter(|e| !e.alternatives.is_empty())
        .collect()
}

fn parse_clause(raw: &str) -> Option<DependencyClause> {
    // Strip architecture qualifiers like [amd64] and build-profiles <...>
    let mut s = raw.trim();
    if let Some(idx) = s.find('[') {
        s = s[..idx].trim();
    }
    if let Some(idx) = s.find('<') {
        s = s[..idx].trim();
    }
    if s.is_empty() {
        return None;
    }

    if let Some(open) = s.find('(') {
        let name = s[..open].trim();
        let close = s.rfind(')')?;
        let inside = s[open + 1..close].trim();
        let constraint = parse_constraint(inside)?;
        if name.is_empty() {
            return None;
        }
        Some(DependencyClause {
            package: PackageRef::new(name),
            constraint,
        })
    } else {
        // Drop :any / :native arch qualifiers on the name
        let name = s.split_once(':').map(|(n, _)| n).unwrap_or(s).trim();
        if name.is_empty() {
            return None;
        }
        Some(DependencyClause {
            package: PackageRef::new(name),
            constraint: VersionConstraint::Any,
        })
    }
}

type ConstraintCtor = fn(String) -> VersionConstraint;

fn parse_constraint(inside: &str) -> Option<VersionConstraint> {
    let inside = inside.trim();
    let makers: [(&str, ConstraintCtor); 8] = [
        (">=", VersionConstraint::Ge),
        ("<=", VersionConstraint::Le),
        (">>", VersionConstraint::Gt),
        ("<<", VersionConstraint::Lt),
        ("!=", VersionConstraint::Ne),
        ("=", VersionConstraint::Eq),
        (">", VersionConstraint::Gt),
        ("<", VersionConstraint::Lt),
    ];
    for (op, ctor) in makers {
        if let Some(rest) = inside.strip_prefix(op) {
            let ver = rest.trim().to_owned();
            if ver.is_empty() {
                return None;
            }
            return Some(ctor(ver));
        }
    }
    None
}

/// Check whether every dependency expression is satisfied by the available package set.
///
/// `available` is `(name, version)` pairs — published packages and/or Provides aliases
/// mapped to the providing package's version.
pub fn satisfies<'a, I>(depends: &[DependencyExpr], available: I) -> Result<(), Vec<DependencyExpr>>
where
    I: IntoIterator<Item = (&'a str, &'a str)> + Clone,
{
    let missing: Vec<_> = depends
        .iter()
        .filter(|expr| !expr.is_satisfied_by(available.clone()))
        .cloned()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_or_and_version() {
        let exprs = parse_depends_field("foo (>= 1.0) | bar, baz");
        assert_eq!(exprs.len(), 2);
        assert_eq!(exprs[0].alternatives.len(), 2);
        assert!(exprs[0].is_satisfied_by([("foo", "1.2")]));
        assert!(!exprs[0].is_satisfied_by([("foo", "0.9")]));
        assert!(exprs[0].is_satisfied_by([("bar", "1")]));
    }
}
