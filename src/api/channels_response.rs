//! [`ChannelsResponse`].

use serde::Serialize;

/// JSON body for `GET /v1/channels`.
#[derive(Serialize)]
pub(crate) struct ChannelsResponse {
    pub(crate) channels: Vec<String>,
}
