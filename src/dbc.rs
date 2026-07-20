//! The Sigma Racer `.dbc` schema catalog (a read-only GitHub mirror).

mod dbc_catalog;
mod dbc_file;
mod github_sync;

pub use dbc_catalog::DbcCatalog;
pub use dbc_file::DbcFile;
pub use github_sync::spawn as spawn_github_sync;

use crate::listing;

/// Prefer the Sigma Racer schema, then the M7 draft, then the first entry.
pub fn latest_dbc_file() -> Option<DbcFile> {
    let files = listing::list::<DbcCatalog>();
    files
        .iter()
        .find(|f| f.name == "sigma-racer")
        .or_else(|| files.iter().find(|f| f.name == "m7-draft"))
        .cloned()
        .or_else(|| files.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing::CatalogSpec;

    fn stub(name: &str) -> DbcFile {
        DbcFile {
            filename: format!("{name}.dbc"),
            name: name.into(),
            size_bytes: 1,
            download_path: format!("/dbc/{name}.dbc"),
        }
    }

    #[test]
    fn rejects_traversal() {
        assert!(listing::path::<DbcCatalog>("../etc/passwd.dbc").is_none());
        assert!(listing::path::<DbcCatalog>("foo/bar.dbc").is_none());
    }

    #[test]
    fn paginates_and_filters() {
        let all: Vec<_> = (0..120).map(|i| stub(&format!("schema-{i:03}"))).collect();
        let page = listing::paginate(&all, 2, 50, "", DbcCatalog::matches);
        assert_eq!(page.total, 120);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.items.len(), 50);
        assert_eq!(page.items[0].name, "schema-050");

        let filtered = listing::paginate(&all, 1, 50, "schema-11", DbcCatalog::matches);
        assert_eq!(filtered.total, 10);
        assert!(filtered.items.iter().all(|f| f.name.contains("schema-11")));
    }
}
