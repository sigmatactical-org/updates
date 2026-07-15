//! [`SchemaRow`].

#[allow(unused_imports)]
use super::*;

#[derive(Debug)]
pub struct SchemaRow {
    pub name: String,
    pub filename: String,
    pub size_label: String,
    pub download_path: String,
}
