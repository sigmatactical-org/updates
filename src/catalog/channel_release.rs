//! [`ChannelRelease`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One release published on an OTA channel.
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
