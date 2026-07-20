//! Shared `ureq` agent construction.
//!
//! Building an agent sets up a connection pool and a TLS provider, so it is
//! done once per process and the agent is cloned/borrowed by every caller
//! (`ureq::Agent` shares its pool across clones).

use std::sync::OnceLock;
use std::time::Duration;

static SHARED: OnceLock<ureq::Agent> = OnceLock::new();

/// Build an agent with the Sigma defaults (native TLS, HTTP statuses returned
/// rather than raised) and the given timeouts.
#[must_use]
pub fn build_agent(connect: Duration, recv_response: Duration, recv_body: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(connect))
        .timeout_recv_response(Some(recv_response))
        .timeout_recv_body(Some(recv_body))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .build(),
        )
        .build()
        .new_agent()
}

/// Process-wide agent (connection pooling, TLS session reuse). Built on first
/// use with 10s connect / 120s response / 120s body timeouts — large `.deb`
/// uploads and downloads share it.
#[must_use]
pub fn shared_agent() -> &'static ureq::Agent {
    SHARED.get_or_init(|| {
        build_agent(
            Duration::from_secs(10),
            Duration::from_secs(120),
            Duration::from_secs(120),
        )
    })
}
