//! Scan and serve `.deb` packages from the local packages directory.

mod deb_package;

pub use deb_package::DebPackage;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sigma_updates_deb::{DebControl, inspect_deb_bytes, inspect_deb_file};

use crate::config;
use crate::listing::{self, ListCache};

pub use crate::listing::PublishError;

pub use crate::listing::DEFAULT_PER_PAGE;

/// Soft cap for a single uploaded `.deb` (512 MiB — covers large -dbg packages).
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;

/// One page of the package index.
pub type PackagePage = listing::Page<DebPackage>;

static LIST_CACHE: Mutex<Option<ListCache<DebPackage>>> = Mutex::new(None);

/// List every `*.deb` in [`config::packages_dir`], sorted by name then version.
pub fn list_packages() -> Vec<DebPackage> {
    let dir = config::packages_dir();
    list_packages_in(&dir)
}

/// Filter + paginate the package index (`page` is 1-based).
pub fn list_packages_page(page: u32, per_page: u32, query: &str) -> PackagePage {
    let all = list_packages();
    paginate_packages(&all, page, per_page, query)
}

/// Filter with the package match predicate and paginate.
pub fn paginate_packages(all: &[DebPackage], page: u32, per_page: u32, query: &str) -> PackagePage {
    listing::paginate(all, page, per_page, query, package_matches)
}

/// Whether a package matches a lowercase search needle.
fn package_matches(pkg: &DebPackage, needle: &str) -> bool {
    pkg.name.to_ascii_lowercase().contains(needle)
        || pkg.filename.to_ascii_lowercase().contains(needle)
        || pkg.version.to_ascii_lowercase().contains(needle)
        || pkg.architecture.to_ascii_lowercase().contains(needle)
        || pkg
            .description
            .as_deref()
            .is_some_and(|d| d.to_ascii_lowercase().contains(needle))
}

/// List `dir` through the fingerprint cache.
pub fn list_packages_in(dir: &Path) -> Vec<DebPackage> {
    listing::cached_list(&LIST_CACHE, dir, "deb", scan_packages)
}

/// Scan `dir` for well-named `.deb` files, sorted by name then newest version.
fn scan_packages(dir: &Path) -> Vec<DebPackage> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut packages: Vec<DebPackage> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("deb"))
        })
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            let filename = path.file_name()?.to_str()?.to_owned();
            if !is_safe_filename(&filename) {
                return None;
            }
            Some(describe_path(&path, &filename, meta.len()))
        })
        .collect();

    packages.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| b.version.cmp(&a.version))
            .then_with(|| a.architecture.cmp(&b.architecture))
    });
    packages
}

/// Resolve a downloadable package path; rejects path traversal.
pub fn package_path(filename: &str) -> Option<PathBuf> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = config::packages_dir().join(filename);
    if path.is_file() { Some(path) } else { None }
}

/// Write (or replace) a `.deb` under the packages directory.
pub fn publish_package(filename: &str, bytes: &[u8]) -> Result<DebPackage, PublishError> {
    if !is_safe_filename(filename) {
        return Err(PublishError::InvalidFilename);
    }
    if bytes.is_empty() {
        return Err(PublishError::EmptyBody);
    }
    if bytes.len() as u64 > MAX_PACKAGE_BYTES {
        return Err(PublishError::TooLarge);
    }
    let control =
        inspect_deb_bytes(bytes).map_err(|e| PublishError::InvalidContent(e.to_string()))?;

    listing::atomic_publish(&config::packages_dir(), filename, bytes)?;
    listing::invalidate(&LIST_CACHE);
    Ok(from_control(filename, bytes.len() as u64, &control))
}

/// Remove a published `.deb`.
pub fn delete_package(filename: &str) -> Result<(), PublishError> {
    let Some(path) = package_path(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    listing::invalidate(&LIST_CACHE);
    Ok(())
}

/// Whether `name` is a plain `.deb` filename (no path traversal).
pub fn is_safe_filename(name: &str) -> bool {
    listing::is_safe_filename(name, ".deb")
}

/// Package metadata for a file on disk, falling back to filename parsing when
/// the archive cannot be inspected.
fn describe_path(path: &Path, filename: &str, size_bytes: u64) -> DebPackage {
    match inspect_deb_file(path) {
        Ok(control) => from_control(filename, size_bytes, &control),
        Err(_) => {
            let (name, version, architecture) = parse_deb_filename(filename);
            DebPackage {
                filename: filename.to_owned(),
                name,
                version,
                architecture,
                size_bytes,
                download_path: format!("/packages/{filename}"),
                depends: Vec::new(),
                pre_depends: Vec::new(),
                provides: Vec::new(),
                description: None,
            }
        }
    }
}

/// Package metadata from a parsed control file.
fn from_control(filename: &str, size_bytes: u64, control: &DebControl) -> DebPackage {
    DebPackage {
        filename: filename.to_owned(),
        name: control.package.clone(),
        version: control.version.clone(),
        architecture: control.architecture.clone(),
        size_bytes,
        download_path: format!("/packages/{filename}"),
        depends: control.depends.clone(),
        pre_depends: control.pre_depends.clone(),
        provides: control.provides.clone(),
        description: control.description.clone(),
    }
}

/// Debian pool naming: `{name}_{version}_{arch}.deb` (name may contain hyphens).
fn parse_deb_filename(filename: &str) -> (String, String, String) {
    let stem = filename.trim_end_matches(".deb");
    let parts: Vec<&str> = stem.rsplitn(3, '_').collect();
    match parts.as_slice() {
        [arch, version, name] => (
            (*name).to_owned(),
            (*version).to_owned(),
            (*arch).to_owned(),
        ),
        _ => (stem.to_owned(), "—".into(), "—".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_standard_deb_name() {
        let (n, v, a) = parse_deb_filename("sigma-updates-sample_0.1.0-1_all.deb");
        assert_eq!(n, "sigma-updates-sample");
        assert_eq!(v, "0.1.0-1");
        assert_eq!(a, "all");
    }

    #[test]
    fn rejects_traversal() {
        assert!(package_path("../etc/passwd.deb").is_none());
        assert!(package_path("foo/bar.deb").is_none());
    }

    #[test]
    fn paginates_and_filters() {
        let all: Vec<_> = (0..120)
            .map(|i| stub(&format!("pkg-{i:03}"), "1"))
            .collect();
        let page = paginate_packages(&all, 2, 50, "");
        assert_eq!(page.total, 120);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.items.len(), 50);
        assert_eq!(page.items[0].name, "pkg-050");

        let filtered = paginate_packages(&all, 1, 50, "pkg-11");
        assert_eq!(filtered.total, 10); // pkg-110 .. pkg-119
        assert!(filtered.items.iter().all(|p| p.name.contains("pkg-11")));
    }
}
