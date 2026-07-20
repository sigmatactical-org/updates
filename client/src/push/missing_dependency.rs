//! [`MissingDependency`].

use sigma_updates_deb::DependencyExpr;

/// Dependencies of one package that nothing available satisfies.
#[derive(Debug, Clone)]
pub struct MissingDependency {
    pub package: String,
    pub missing: Vec<DependencyExpr>,
}
