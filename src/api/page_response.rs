//! [`PageResponse`].

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

use crate::listing::Page;

/// JSON body for a paginated catalog listing.
///
/// The items live under a per-catalog key (`packages`, `files`, …), so the
/// body is serialized by hand rather than derived — one type covers every
/// catalog while keeping each endpoint's wire format unchanged.
pub(crate) struct PageResponse<T> {
    items_field: &'static str,
    page: Page<T>,
}

impl<T> PageResponse<T> {
    /// Wrap `page`, naming the item array `items_field`.
    pub(crate) fn new(items_field: &'static str, page: Page<T>) -> Self {
        Self { items_field, page }
    }
}

impl<T: Serialize> Serialize for PageResponse<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let query = !self.page.query.is_empty();
        let mut map = serializer.serialize_map(Some(5 + usize::from(query)))?;
        map.serialize_entry(self.items_field, &self.page.items)?;
        map.serialize_entry("total", &self.page.total)?;
        map.serialize_entry("page", &self.page.page)?;
        map.serialize_entry("per_page", &self.page.per_page)?;
        map.serialize_entry("total_pages", &self.page.total_pages)?;
        if query {
            map.serialize_entry("query", &self.page.query)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(query: &str) -> Page<&'static str> {
        Page {
            items: vec!["a"],
            total: 1,
            page: 1,
            per_page: 50,
            total_pages: 1,
            query: query.to_owned(),
        }
    }

    #[test]
    fn names_the_item_array_per_catalog() {
        let json = serde_json::to_string(&PageResponse::new("packages", page(""))).unwrap();
        assert!(json.starts_with(r#"{"packages":["a"],"total":1"#), "{json}");
        assert!(!json.contains("query"));
    }

    #[test]
    fn includes_a_non_empty_query() {
        let json = serde_json::to_string(&PageResponse::new("files", page("abc"))).unwrap();
        assert!(json.contains(r#""files":["a"]"#), "{json}");
        assert!(json.contains(r#""query":"abc""#), "{json}");
    }
}
