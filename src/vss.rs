//! The mirrored VSS catalog (signal tree + CAN→VSS mapping docs).

mod vss_catalog;
mod vss_file;

pub use vss_catalog::VssCatalog;
pub use vss_file::VssFile;

use crate::listing;

/// Prefer the cluster signal tree, then the Sigma Racer mapping, then the
/// first catalog entry.
pub fn latest_vss_file() -> Option<VssFile> {
    let files = listing::list::<VssCatalog>();
    files
        .iter()
        .find(|f| f.filename == "sigma-cluster.vspec")
        .or_else(|| files.iter().find(|f| f.name == "sigma-racer"))
        .cloned()
        .or_else(|| files.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing::CatalogSpec;

    #[test]
    fn accepts_vss_extensions_only() {
        assert!(VssCatalog::is_safe_filename("sigma-cluster.vspec"));
        assert!(VssCatalog::is_safe_filename("sigma-racer.yaml"));
        assert!(!VssCatalog::is_safe_filename("sigma-racer.dbc"));
        assert!(!VssCatalog::is_safe_filename("../evil.yaml"));
        assert!(!VssCatalog::is_safe_filename("a/b.vspec"));
    }

    #[test]
    fn rejects_traversal() {
        assert!(listing::path::<VssCatalog>("../etc/passwd.yaml").is_none());
        assert!(listing::path::<VssCatalog>("foo/bar.vspec").is_none());
    }
}
