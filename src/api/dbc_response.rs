//! [`DbcResponse`].

#[allow(unused_imports)]
use super::*;
use crate::dbc::{self};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct DbcResponse {
    pub(crate) files: Vec<dbc::DbcFile>,
    pub(crate) total: usize,
    pub(crate) page: u32,
    pub(crate) per_page: u32,
    pub(crate) total_pages: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) query: String,
}
