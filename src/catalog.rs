//! In-memory / env-backed channel catalog.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::public_base_url_trimmed;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelRelease {
    pub channel: String,
    pub version: String,
    /// RAUC `compatible` glob / machine family.
    pub compatible: String,
    pub bundle_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub released_at: DateTime<Utc>,
    pub notes: String,
    /// UX hint for clients: always reboot after install for A/B.
    pub install: String,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    releases: Vec<ChannelRelease>,
}

impl Catalog {
    /// Built-in **dev** channel pointing at the local kind ingress URL.
    pub fn with_dev_defaults() -> Self {
        let base = public_base_url_trimmed();
        let version = std::env::var("UPDATES_DEV_VERSION")
            .unwrap_or_else(|_| "0.1.0-dev.1".into());
        Self {
            releases: vec![ChannelRelease {
                channel: "dev".into(),
                version: version.clone(),
                compatible: "sigma-racer-wingman-*".into(),
                bundle_url: format!("{base}/v1/channel/dev/bundle/{version}.raucb"),
                sha256: None,
                released_at: Utc::now(),
                notes: "Dev channel sample — install updates and reboot when ready.".into(),
                install: "reboot".into(),
            }],
        }
    }

    pub fn channels(&self) -> Vec<String> {
        let mut names: Vec<_> = self.releases.iter().map(|r| r.channel.clone()).collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn latest(&self, channel: &str) -> Option<&ChannelRelease> {
        self.releases.iter().find(|r| r.channel == channel)
    }
}
