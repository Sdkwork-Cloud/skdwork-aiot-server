//! Store-level offset pagination types aligned with `PAGINATION_SPEC.md` and `API_SPEC.md` §16.

pub use sdkwork_utils_rust::{
    offset_list_page_info, OffsetListPageParams, DEFAULT_LIST_PAGE_SIZE, MAX_LIST_PAGE_SIZE,
};

/// Paginated list result from an authoritative store (SQL or maintained index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotOffsetListResult<T> {
    pub items: Vec<T>,
    pub total: i64,
}

impl<T> AiotOffsetListResult<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: 0,
        }
    }

    pub fn single_page(items: Vec<T>) -> Self {
        let total = items.len() as i64;
        Self { items, total }
    }
}

/// Window extraction for bounded in-memory test repositories only.
pub fn paginate_vec<T>(items: Vec<T>, params: OffsetListPageParams) -> AiotOffsetListResult<T> {
    let total = items.len() as i64;
    let offset = params.offset.max(0) as usize;
    let limit = params.page_size.max(1) as usize;
    let page_items = items.into_iter().skip(offset).take(limit).collect();
    AiotOffsetListResult {
        items: page_items,
        total,
    }
}
