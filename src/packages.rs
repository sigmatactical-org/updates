//! The `.deb` package feed.
//!
//! Scanning, pagination, publish and delete are the generic catalog
//! operations in [`crate::listing`], specialised by [`PackageCatalog`].

mod package_catalog;

pub use package_catalog::PackageCatalog;
pub use sigma_updates_deb::DebPackage;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing::{self, CatalogSpec};

    fn stub(name: &str, version: &str) -> DebPackage {
        DebPackage {
            filename: format!("{name}_{version}_all.deb"),
            name: name.into(),
            version: version.into(),
            architecture: "all".into(),
            size_bytes: 1,
            download_path: format!("/packages/{name}_{version}_all.deb"),
            depends: Vec::new(),
            pre_depends: Vec::new(),
            provides: Vec::new(),
            description: None,
        }
    }

    #[test]
    fn rejects_traversal() {
        assert!(listing::path::<PackageCatalog>("../etc/passwd.deb").is_none());
        assert!(listing::path::<PackageCatalog>("foo/bar.deb").is_none());
    }

    #[test]
    fn paginates_and_filters() {
        let all: Vec<_> = (0..120)
            .map(|i| stub(&format!("pkg-{i:03}"), "1"))
            .collect();
        let page = listing::paginate(&all, 2, 50, "", PackageCatalog::matches);
        assert_eq!(page.total, 120);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.items.len(), 50);
        assert_eq!(page.items[0].name, "pkg-050");

        let filtered = listing::paginate(&all, 1, 50, "pkg-11", PackageCatalog::matches);
        assert_eq!(filtered.total, 10); // pkg-110 .. pkg-119
        assert!(filtered.items.iter().all(|p| p.name.contains("pkg-11")));
    }

    #[test]
    fn last_page_is_clamped_and_partial() {
        let all: Vec<_> = (0..25).map(|i| stub(&format!("pkg-{i:02}"), "1")).collect();
        let page = listing::paginate(&all, 99, 10, "", PackageCatalog::matches);
        assert_eq!(page.page, 3);
        assert_eq!(page.items.len(), 5);
    }
}
