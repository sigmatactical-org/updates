//! [`Cli`].

use clap::Parser;
use sigma_updates_client::{client_credentials_token, token_url_from_issuer};

use crate::command::Command;

/// Global options plus the selected subcommand.
#[derive(Parser, Debug)]
#[command(name = "sigma-updates-cli")]
#[command(about = "Publish and inspect packages on sigma-updates")]
pub struct Cli {
    /// Base URL of the updates service or Identity `/api` proxy
    /// (e.g. http://updates.sigma.localtest.me:30080 or https://identity…/api)
    #[arg(long, env = "SIGMA_UPDATES_URL", global = true)]
    pub url: Option<String>,

    /// Shared secret for direct updates publish/delete (`SIGMA_INTERNAL_TOKEN`)
    #[arg(long, env = "SIGMA_INTERNAL_TOKEN", global = true)]
    pub token: Option<String>,

    /// OIDC token endpoint (client-credentials). Overrides issuer derivation.
    #[arg(long, env = "SIGMA_OIDC_TOKEN_URL", global = true)]
    pub oidc_token_url: Option<String>,

    /// OIDC issuer URL used to derive the token endpoint when `--oidc-token-url` is unset
    #[arg(long, env = "SIGMA_OIDC_ISSUER", global = true)]
    pub oidc_issuer: Option<String>,

    /// OIDC confidential client id (service account)
    #[arg(long, env = "SIGMA_OIDC_CLIENT_ID", global = true)]
    pub oidc_client_id: Option<String>,

    /// OIDC confidential client secret
    #[arg(long, env = "SIGMA_OIDC_CLIENT_SECRET", global = true)]
    pub oidc_client_secret: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// Trimmed value, or `None` when unset/blank.
fn present(value: Option<&String>) -> Option<&str> {
    value.map(|s| s.trim()).filter(|s| !s.is_empty())
}

impl Cli {
    /// Base URL of the service (default: local dev server).
    pub fn base_url(&self) -> &str {
        present(self.url.as_ref()).unwrap_or("http://127.0.0.1:8080")
    }

    /// Resolve the auth token: an OIDC client-credentials exchange when any
    /// OIDC option is set, otherwise the shared internal token.
    pub fn auth_token(&self) -> Result<Option<String>, String> {
        let oidc = [
            &self.oidc_client_id,
            &self.oidc_client_secret,
            &self.oidc_token_url,
            &self.oidc_issuer,
        ]
        .into_iter()
        .any(|opt| present(opt.as_ref()).is_some());

        if !oidc {
            return Ok(present(self.token.as_ref()).map(str::to_owned));
        }

        let client_id = present(self.oidc_client_id.as_ref())
            .ok_or("OIDC auth requires --oidc-client-id / SIGMA_OIDC_CLIENT_ID")?;
        let client_secret = present(self.oidc_client_secret.as_ref())
            .ok_or("OIDC auth requires --oidc-client-secret / SIGMA_OIDC_CLIENT_SECRET")?;
        let token_url = match present(self.oidc_token_url.as_ref()) {
            Some(url) => url.to_owned(),
            None => token_url_from_issuer(present(self.oidc_issuer.as_ref()).ok_or(
                "OIDC auth requires --oidc-token-url / SIGMA_OIDC_TOKEN_URL or --oidc-issuer / SIGMA_OIDC_ISSUER",
            )?),
        };
        client_credentials_token(&token_url, client_id, client_secret)
            .map(Some)
            .map_err(|e| e.to_string())
    }
}
