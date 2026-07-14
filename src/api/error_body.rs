//! [`ErrorBody`].

#[allow(unused_imports)]
use super::*;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ErrorBody {
    pub(crate) error: String,
}
