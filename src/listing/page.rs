//! One page of a filtered catalog listing.

/// A pagination window over a filtered listing.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// Items on this page.
    pub items: Vec<T>,
    /// Total items after filtering.
    pub total: usize,
    /// 1-based page number (clamped into range).
    pub page: u32,
    /// Page size used.
    pub per_page: u32,
    /// Total pages (at least 1).
    pub total_pages: u32,
    /// The trimmed filter query.
    pub query: String,
}

impl<T> Page<T> {
    /// Whether a previous page exists.
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    /// Whether a further page exists.
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }
}
