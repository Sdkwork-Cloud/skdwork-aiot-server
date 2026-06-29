use sdkwork_aiot_transport::HttpRequest;
use sdkwork_utils_rust::{PageInfo, PageMode};

/// Standard list query parameters per `API_SPEC.md` section 16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageQuery {
    pub page: u32,
    pub page_size: u32,
}

impl PageQuery {
    pub const DEFAULT_PAGE: u32 = 1;
    pub const DEFAULT_PAGE_SIZE: u32 = 20;
    pub const MAX_PAGE_SIZE: u32 = 200;

    pub fn from_request(request: &HttpRequest) -> Self {
        let page = request
            .query_param("page")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(Self::DEFAULT_PAGE)
            .max(1);
        let page_size = request
            .query_param("page_size")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(Self::DEFAULT_PAGE_SIZE)
            .clamp(1, Self::MAX_PAGE_SIZE);
        Self { page, page_size }
    }

    pub fn slice_bounds(self, total: usize) -> (usize, usize) {
        if total == 0 {
            return (0, 0);
        }
        let start = ((self.page - 1) as usize).saturating_mul(self.page_size as usize);
        if start >= total {
            return (total, total);
        }
        let end = (start + self.page_size as usize).min(total);
        (start, end)
    }
}

pub fn paginated_page_info(page_query: PageQuery, total: usize) -> PageInfo {
    let has_more = (page_query.page as usize).saturating_mul(page_query.page_size as usize) < total;
    let total_pages = if page_query.page_size == 0 {
        0
    } else {
        total.div_ceil(page_query.page_size as usize) as i32
    };

    PageInfo {
        mode: PageMode::Offset,
        page: Some(page_query.page as i32),
        page_size: Some(page_query.page_size as i32),
        total_items: Some(total.to_string()),
        total_pages: Some(total_pages),
        next_cursor: None,
        has_more: Some(has_more),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_query_defaults_and_clamps_page_size() {
        let request = HttpRequest::new("GET", "/items");
        let query = PageQuery::from_request(&request);
        assert_eq!(query.page, 1);
        assert_eq!(query.page_size, 20);
    }

    #[test]
    fn slice_bounds_respects_page_window() {
        let query = PageQuery {
            page: 2,
            page_size: 10,
        };
        assert_eq!(query.slice_bounds(25), (10, 20));
        assert_eq!(query.slice_bounds(0), (0, 0));
    }

    #[test]
    fn page_info_uses_offset_mode() {
        let page_info = paginated_page_info(
            PageQuery {
                page: 1,
                page_size: 20,
            },
            0,
        );
        assert_eq!(page_info.mode, PageMode::Offset);
        assert_eq!(page_info.page, Some(1));
        assert_eq!(page_info.page_size, Some(20));
        assert_eq!(page_info.total_items.as_deref(), Some("0"));
    }
}
