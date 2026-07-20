//! [`PushPlan`].

use super::local_package::LocalPackage;
use super::missing_dependency::MissingDependency;

/// What a push would do: publish order plus unsatisfied dependencies.
#[derive(Debug, Clone)]
pub struct PushPlan {
    /// Packages to publish, dependencies first.
    pub order: Vec<LocalPackage>,
    pub missing: Vec<MissingDependency>,
}
