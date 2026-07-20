//! [`PackageCatalog`] — the `.deb` package feed.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sigma_updates_deb::{DebPackage, inspect_deb_file};

use crate::config;
use crate::listing::{CatalogSpec, ListCache, is_safe_filename};

/// The `.deb` feed served from [`config::packages_dir`].
pub struct PackageCatalog;

static CACHE: Mutex<Option<ListCache<DebPackage>>> = ListCache::empty();

impl CatalogSpec for PackageCatalog {
    type Item = DebPackage;

    /// Soft cap for a single uploaded `.deb` (512 MiB — covers large -dbg packages).
    const MAX_BYTES: u64 = 512 * 1024 * 1024;

    fn dir() -> PathBuf {
        config::packages_dir()
    }

    fn is_safe_filename(name: &str) -> bool {
        is_safe_filename(name, ".deb")
    }

    fn filename_of(pkg: &DebPackage) -> &str {
        &pkg.filename
    }

    fn matches(pkg: &DebPackage, needle: &str) -> bool {
        pkg.name.to_ascii_lowercase().contains(needle)
            || pkg.filename.to_ascii_lowercase().contains(needle)
            || pkg.version.to_ascii_lowercase().contains(needle)
            || pkg.architecture.to_ascii_lowercase().contains(needle)
            || pkg
                .description
                .as_deref()
                .is_some_and(|d| d.to_ascii_lowercase().contains(needle))
    }

    /// Name, then newest version, then architecture.
    fn sort(packages: &mut [DebPackage]) {
        packages.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| b.version.cmp(&a.version))
                .then_with(|| a.architecture.cmp(&b.architecture))
        });
    }

    /// Control metadata when the archive can be read, filename parsing otherwise.
    fn describe(path: &Path, filename: &str, size_bytes: u64) -> DebPackage {
        Self::describe_validated(path, filename, size_bytes).unwrap_or_else(|_| {
            let (name, version, architecture) = parse_deb_filename(filename);
            DebPackage {
                filename: filename.to_owned(),
                name,
                version,
                architecture,
                size_bytes,
                download_path: download_path(filename),
                depends: Vec::new(),
                pre_depends: Vec::new(),
                provides: Vec::new(),
                description: None,
            }
        })
    }

    /// Requires a readable `.deb` archive with a parseable control file.
    fn describe_validated(
        path: &Path,
        filename: &str,
        size_bytes: u64,
    ) -> Result<DebPackage, String> {
        let control = inspect_deb_file(path).map_err(|e| e.to_string())?;
        Ok(DebPackage::from_control(
            filename,
            size_bytes,
            download_path(filename),
            &control,
        ))
    }

    fn cache() -> &'static Mutex<Option<ListCache<DebPackage>>> {
        &CACHE
    }
}

fn download_path(filename: &str) -> String {
    format!("/packages/{filename}")
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

    #[test]
    fn parses_standard_deb_name() {
        let (n, v, a) = parse_deb_filename("sigma-updates-sample_0.1.0-1_all.deb");
        assert_eq!(n, "sigma-updates-sample");
        assert_eq!(v, "0.1.0-1");
        assert_eq!(a, "all");
    }

    #[test]
    fn unparseable_names_fall_back_to_placeholders() {
        let (n, v, a) = parse_deb_filename("weird.deb");
        assert_eq!(n, "weird");
        assert_eq!((v.as_str(), a.as_str()), ("—", "—"));
    }
}
