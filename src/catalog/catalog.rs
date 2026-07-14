//! [`Catalog`].

#[allow(unused_imports)]
use super::*;
use crate::config::public_base_url_trimmed;
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct Catalog {
    pub(crate) releases: Vec<ChannelRelease>,
}
impl Catalog {
    /// Built-in **dev** channel pointing at the local kind ingress URL.
    pub fn with_dev_defaults() -> Self {
        let base = public_base_url_trimmed();
        let version = std::env::var("UPDATES_DEV_VERSION").unwrap_or_else(|_| "0.1.0-dev.1".into());
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

    /// Channel names present in the catalog.
    pub fn channels(&self) -> Vec<String> {
        let mut names: Vec<_> = self.releases.iter().map(|r| r.channel.clone()).collect();
        names.sort();
        names.dedup();
        names
    }

    /// Latest release on `channel`, if any.
    pub fn latest(&self, channel: &str) -> Option<&ChannelRelease> {
        self.releases.iter().find(|r| r.channel == channel)
    }
}
