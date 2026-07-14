//! Scan and serve `.deb` packages from the local packages directory.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;
use sigma_updates_deb::{
    DebControl, DependencyExpr, PackageRef, inspect_deb_bytes, inspect_deb_file,
};
use thiserror::Error;

use crate::config;

#[derive(Debug, Clone, Serialize)]
pub struct DebPackage {
    pub filename: String,
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub size_bytes: u64,
    pub download_path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<DependencyExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_depends: Vec<DependencyExpr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<PackageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("invalid package filename")]
    InvalidFilename,
    #[error("empty package body")]
    EmptyBody,
    #[error("package too large")]
    TooLarge,
    #[error("package not found")]
    NotFound,
    #[error("invalid deb: {0}")]
    InvalidDeb(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Soft cap for a single uploaded `.deb` (512 MiB — covers large -dbg packages).
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirFingerprint {
    count: u64,
    total_bytes: u64,
    newest_mtime_secs: u64,
}

struct ListCache {
    fingerprint: DirFingerprint,
    packages: Vec<DebPackage>,
}

static LIST_CACHE: Mutex<Option<ListCache>> = Mutex::new(None);

fn invalidate_list_cache() {
    if let Ok(mut guard) = LIST_CACHE.lock() {
        *guard = None;
    }
}

fn dir_fingerprint(dir: &Path) -> DirFingerprint {
    let Ok(entries) = fs::read_dir(dir) else {
        return DirFingerprint {
            count: 0,
            total_bytes: 0,
            newest_mtime_secs: 0,
        };
    };
    let mut count = 0u64;
    let mut total_bytes = 0u64;
    let mut newest_mtime_secs = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("deb"))
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        count += 1;
        total_bytes += meta.len();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        newest_mtime_secs = newest_mtime_secs.max(mtime);
    }
    DirFingerprint {
        count,
        total_bytes,
        newest_mtime_secs,
    }
}

/// Default page size for the HTML UI and JSON API.
pub const DEFAULT_PER_PAGE: u32 = 50;
/// Hard cap so a single response cannot dump the entire multi-thousand feed.
pub const MAX_PER_PAGE: u32 = 500;

/// One page of the package index.
#[derive(Debug, Clone)]
pub struct PackagePage {
    pub packages: Vec<DebPackage>,
    pub total: usize,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub query: String,
}

impl PackagePage {
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }
}

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

pub fn paginate_packages(all: &[DebPackage], page: u32, per_page: u32, query: &str) -> PackagePage {
    let per_page = per_page.clamp(1, MAX_PER_PAGE);
    let q = query.trim();
    let filtered: Vec<DebPackage> = if q.is_empty() {
        all.to_vec()
    } else {
        let needle = q.to_ascii_lowercase();
        all.iter()
            .filter(|p| package_matches(p, &needle))
            .cloned()
            .collect()
    };

    let total = filtered.len();
    let total_pages = if total == 0 {
        1
    } else {
        (total as u32).div_ceil(per_page)
    };
    let page = page.clamp(1, total_pages);
    let start = ((page - 1) * per_page) as usize;
    let packages = filtered
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();

    PackagePage {
        packages,
        total,
        page,
        per_page,
        total_pages,
        query: q.to_owned(),
    }
}

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

pub fn list_packages_in(dir: &Path) -> Vec<DebPackage> {
    let fingerprint = dir_fingerprint(dir);
    if let Ok(guard) = LIST_CACHE.lock()
        && let Some(cache) = guard.as_ref()
        && cache.fingerprint == fingerprint
    {
        return cache.packages.clone();
    }

    let packages = scan_packages(dir);
    if let Ok(mut guard) = LIST_CACHE.lock() {
        *guard = Some(ListCache {
            fingerprint,
            packages: packages.clone(),
        });
    }
    packages
}

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
    let control = inspect_deb_bytes(bytes).map_err(|e| PublishError::InvalidDeb(e.to_string()))?;

    let dir = config::packages_dir();
    fs::create_dir_all(&dir)?;
    let dest = dir.join(filename);
    let tmp = dir.join(format!(".{filename}.tmp"));
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &dest)?;
    invalidate_list_cache();
    Ok(from_control(filename, bytes.len() as u64, &control))
}

/// Remove a published `.deb`.
pub fn delete_package(filename: &str) -> Result<(), PublishError> {
    let Some(path) = package_path(filename) else {
        return Err(PublishError::NotFound);
    };
    fs::remove_file(path)?;
    invalidate_list_cache();
    Ok(())
}

pub fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name.ends_with(".deb")
}

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
        assert_eq!(page.packages.len(), 50);
        assert_eq!(page.packages[0].name, "pkg-050");

        let filtered = paginate_packages(&all, 1, 50, "pkg-11");
        assert_eq!(filtered.total, 10); // pkg-110 .. pkg-119
        assert!(filtered.packages.iter().all(|p| p.name.contains("pkg-11")));
    }
}
