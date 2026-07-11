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

/// Directory of Sigma Racer `.dbc` schema files (default `./dbc`).
#[must_use]
pub fn dbc_dir() -> std::path::PathBuf {
    std::env::var("UPDATES_DBC_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("dbc"))
}
