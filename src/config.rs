//! Runtime configuration for sigma-updates.

fn normalize_base_url(url: &str) -> String {
    let mut url = url.trim().to_string();
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

/// Public base URL of this service (trailing slash).
#[must_use]
pub fn public_base_url() -> String {
    std::env::var("UPDATES_PUBLIC_BASE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_base_url(&s))
        .unwrap_or_else(|| "http://127.0.0.1:8080/".to_string())
}

/// Public base without trailing slash (bundle URLs, CSP).
#[must_use]
pub fn public_base_url_trimmed() -> String {
    public_base_url().trim_end_matches('/').to_owned()
}

#[must_use]
pub fn identity_public_base_url() -> String {
    std::env::var("UPDATES_IDENTITY_PUBLIC_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_base_url(&s))
        .unwrap_or_else(|| "http://127.0.0.1:3000/".to_string())
}

#[must_use]
pub fn identity_public_origin() -> String {
    identity_public_base_url().trim_end_matches('/').to_string()
}

#[must_use]
pub fn contact_public_base_url() -> String {
    std::env::var("UPDATES_CONTACT_PUBLIC_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_base_url(&s))
        .unwrap_or_else(|| "http://127.0.0.1:8083/".to_string())
}

#[must_use]
pub fn cart_public_base_url() -> String {
    std::env::var("UPDATES_CART_PUBLIC_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| normalize_base_url(&s))
        .unwrap_or_else(|| "http://127.0.0.1:8084/".to_string())
}

/// Directory of `.deb` files this service publishes (default `./packages`).
#[must_use]
pub fn packages_dir() -> std::path::PathBuf {
    std::env::var("UPDATES_PACKAGES_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("packages"))
}

/// Directory of RAUC update bundles, one subdir per channel (default `./bundles`).
#[must_use]
pub fn bundles_dir() -> std::path::PathBuf {
    std::env::var("UPDATES_BUNDLES_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("bundles"))
}

/// Local cache directory for mirrored `.dbc` schemas (default `./dbc`).
#[must_use]
pub fn dbc_dir() -> std::path::PathBuf {
    std::env::var("UPDATES_DBC_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("dbc"))
}

fn env_or(var: &str, default: &str) -> String {
    std::env::var(var)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// GitHub `owner/repo` holding the canonical `.dbc` schemas.
#[must_use]
pub fn dbc_github_repo() -> String {
    env_or(
        "UPDATES_DBC_GITHUB_REPO",
        "sigmatactical-org/sigma-racer-wingman",
    )
}

/// Repo subdirectory the schemas are mirrored from.
#[must_use]
pub fn dbc_github_path() -> String {
    env_or("UPDATES_DBC_GITHUB_PATH", "schemas/can")
}

/// Git ref (branch, tag, or SHA) the schemas are mirrored from.
#[must_use]
pub fn dbc_github_ref() -> String {
    env_or("UPDATES_DBC_GITHUB_REF", "main")
}

/// Pause between DBC mirror passes (default 300s).
#[must_use]
pub fn dbc_sync_interval() -> std::time::Duration {
    let secs = std::env::var("UPDATES_DBC_SYNC_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    std::time::Duration::from_secs(secs)
}

/// Optional GitHub token (rate limits / private mirrors).
#[must_use]
pub fn github_token() -> Option<String> {
    std::env::var("UPDATES_GITHUB_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Human-readable mirror source for logs.
#[must_use]
pub fn dbc_github_source() -> String {
    format!(
        "{}:{}@{}",
        dbc_github_repo(),
        dbc_github_path(),
        dbc_github_ref()
    )
}
