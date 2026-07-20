//! Parse and evaluate Debian dependency fields.

mod dependency_clause;
mod dependency_expr;
mod package_ref;
mod version_constraint;

pub use dependency_clause::DependencyClause;
pub use dependency_expr::DependencyExpr;
pub use package_ref::PackageRef;
pub use version_constraint::VersionConstraint;

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

    #[test]
    fn displays_debian_control_syntax() {
        let exprs = parse_depends_field("foo (>= 1.0) | bar, baz");
        assert_eq!(exprs[0].to_string(), "foo (>= 1.0) | bar");
        assert_eq!(exprs[1].to_string(), "baz");
    }
}
