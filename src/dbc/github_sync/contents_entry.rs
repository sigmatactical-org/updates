//! [`ContentsEntry`].

use serde::Deserialize;

/// One entry of a GitHub contents-API directory listing.
#[derive(Debug, Deserialize)]
pub(super) struct ContentsEntry {
    pub(super) name: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) download_url: Option<String>,
}
