//! HTTP client for sigma-updates package index.

mod oidc;
mod push;
mod types;

pub use oidc::{client_credentials_token, token_url_from_issuer};
pub use push::{
    MissingDependency, PushPlan, PushReport, check_packages, collect_deb_paths,
    load_local_packages, plan_push, push_packages,
};
pub use types::{DebPackage, PackagesResponse};

use std::path::Path;
use std::time::Duration;

use thiserror::Error;

use crate::types::PackagesResponse as PackagesResponseBody;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(String),
    #[error("unexpected status {status}: {body}")]
    Status { status: u16, body: String },
    #[error("json error: {0}")]
    Json(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

/// Client for the sigma-updates HTTP API.
#[derive(Debug, Clone)]
pub struct UpdatesClient {
    base_url: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl UpdatesClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .build(),
            )
            .build()
            .new_agent();
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token: None,
            agent,
        }
    }

    /// Shared-secret token (`SIGMA_INTERNAL_TOKEN`) for publish/delete.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        if !token.trim().is_empty() {
            self.token = Some(token);
        }
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn list_packages(&self) -> Result<Vec<DebPackage>, ClientError> {
        let mut all = Vec::new();
        let mut page = 1u32;
        loop {
            let batch = self.list_packages_page(page, 500, "")?;
            all.extend(batch.packages);
            if page >= batch.total_pages || batch.total_pages == 0 {
                break;
            }
            page += 1;
        }
        Ok(all)
    }

    pub fn list_packages_page(
        &self,
        page: u32,
        per_page: u32,
        query: &str,
    ) -> Result<PackagesResponseBody, ClientError> {
        let mut url = format!(
            "{}/v1/packages?page={page}&per_page={per_page}",
            self.base_url
        );
        if !query.trim().is_empty() {
            url.push_str("&q=");
            url.push_str(&urlencoding_lite(query.trim()));
        }
        let mut resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(ClientError::Status { status, body });
        }
        serde_json::from_str(&body).map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn publish_file(&self, path: &Path) -> Result<DebPackage, ClientError> {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ClientError::Message("invalid package path".into()))?
            .to_owned();
        let bytes = std::fs::read(path)?;
        self.publish_bytes(&filename, &bytes)
    }

    pub fn publish_bytes(&self, filename: &str, bytes: &[u8]) -> Result<DebPackage, ClientError> {
        let token = self
            .token
            .as_deref()
            .ok_or_else(|| ClientError::Message("publish requires an auth token".into()))?;
        let url = format!("{}/v1/packages", self.base_url);
        let mut resp = self
            .agent
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("X-Package-Filename", filename)
            .header("Content-Type", "application/octet-stream")
            .send(bytes)
            .map_err(|e| ClientError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(ClientError::Status { status, body });
        }
        serde_json::from_str(&body).map_err(|e| ClientError::Json(e.to_string()))
    }

    pub fn delete_package(&self, filename: &str) -> Result<(), ClientError> {
        let token = self
            .token
            .as_deref()
            .ok_or_else(|| ClientError::Message("delete requires an auth token".into()))?;
        let url = format!("{}/v1/packages/{filename}", self.base_url);
        let mut resp = self
            .agent
            .delete(&url)
            .header("Authorization", format!("Bearer {token}"))
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        Err(ClientError::Status {
            status,
            body: resp.body_mut().read_to_string().unwrap_or_default(),
        })
    }
}

/// Minimal query-string encoding for the `q` parameter.
fn urlencoding_lite(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
