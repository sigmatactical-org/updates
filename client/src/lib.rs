//! HTTP client for sigma-updates package index.

mod push;
mod types;

pub use push::{
    MissingDependency, PushPlan, PushReport, check_packages, collect_deb_paths, load_local_packages,
    plan_push, push_packages,
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
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(120))
            .build();
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
        let body = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| ClientError::Http(e.to_string()))?
            .into_string()
            .map_err(|e| ClientError::Http(e.to_string()))?;
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
        let response = self
            .agent
            .post(&url)
            .set("Authorization", &format!("Bearer {token}"))
            .set("X-Package-Filename", filename)
            .set("Content-Type", "application/octet-stream")
            .send_bytes(bytes);
        match response {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.into_string().unwrap_or_default();
                if !(200..300).contains(&status) {
                    return Err(ClientError::Status { status, body });
                }
                serde_json::from_str(&body).map_err(|e| ClientError::Json(e.to_string()))
            }
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(ClientError::Status { status, body })
            }
            Err(e) => Err(ClientError::Http(e.to_string())),
        }
    }

    pub fn delete_package(&self, filename: &str) -> Result<(), ClientError> {
        let token = self
            .token
            .as_deref()
            .ok_or_else(|| ClientError::Message("delete requires an auth token".into()))?;
        let url = format!("{}/v1/packages/{filename}", self.base_url);
        let response = self
            .agent
            .delete(&url)
            .set("Authorization", &format!("Bearer {token}"))
            .call();
        match response {
            Ok(resp) if (200..300).contains(&resp.status()) || resp.status() == 204 => Ok(()),
            Ok(resp) => Err(ClientError::Status {
                status: resp.status(),
                body: resp.into_string().unwrap_or_default(),
            }),
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(ClientError::Status { status, body })
            }
            Err(e) => Err(ClientError::Http(e.to_string())),
        }
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
