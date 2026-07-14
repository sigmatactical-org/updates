//! [`ChannelsResponse`].

#[allow(unused_imports)]
use super::*;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ChannelsResponse {
    pub(crate) channels: Vec<String>,
}
