//! [`PageQuery`].

use serde::Deserialize;

use crate::listing::DEFAULT_PER_PAGE;

/// `?page=&per_page=&q=` — shared by the HTML index and every JSON listing.
#[derive(Debug, Deserialize)]
pub(crate) struct PageQuery {
    page: Option<u32>,
    per_page: Option<u32>,
    q: Option<String>,
}

impl PageQuery {
    /// 1-based page number (default 1).
    pub(crate) fn page(&self) -> u32 {
        self.page.unwrap_or(1)
    }

    /// Requested page size (default [`DEFAULT_PER_PAGE`]; clamped downstream).
    pub(crate) fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(DEFAULT_PER_PAGE)
    }

    /// Search needle (empty when absent).
    pub(crate) fn query(&self) -> &str {
        self.q.as_deref().unwrap_or("")
    }
}
