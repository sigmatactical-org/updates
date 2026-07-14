//! OIDC helpers for machine (client-credentials) auth.

use crate::ClientError;

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Exchange client id/secret for an access token (OIDC client-credentials grant).
pub fn client_credentials_token(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String, ClientError> {
    let agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .timeout_recv_response(Some(std::time::Duration::from_secs(30)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(30)))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .new_agent();
    let mut resp = agent
        .post(token_url.trim())
        .send_form([
            ("grant_type", "client_credentials"),
            ("client_id", client_id.trim()),
            ("client_secret", client_secret.trim()),
        ])
        .map_err(|e| ClientError::Http(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(ClientError::Status { status, body });
    }
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|e| ClientError::Json(e.to_string()))?;
    if parsed.access_token.trim().is_empty() {
        return Err(ClientError::Message(
            "token response missing access_token".into(),
        ));
    }
    Ok(parsed.access_token)
}

/// Build a Keycloak token endpoint URL from an issuer base
/// (`https://…/realms/multcorp` → `…/protocol/openid-connect/token`).
#[must_use]
pub fn token_url_from_issuer(issuer: &str) -> String {
    let base = issuer.trim().trim_end_matches('/');
    format!("{base}/protocol/openid-connect/token")
}

#[cfg(test)]
mod tests {
    use super::token_url_from_issuer;

    #[test]
    fn token_url_appends_path() {
        assert_eq!(
            token_url_from_issuer("https://keycloak.example/realms/multcorp/"),
            "https://keycloak.example/realms/multcorp/protocol/openid-connect/token"
        );
    }
}
