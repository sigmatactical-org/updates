//! [`Health`].

#[allow(unused_imports)]
use super::*;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Health {
    pub(crate) status: &'static str,
    pub(crate) service: &'static str,
}
