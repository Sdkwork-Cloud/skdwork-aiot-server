use sdkwork_aiot_transport::HttpRequest;
use sdkwork_utils_rust::OffsetListPageParams;

/// Standard offset list parameters (`SdkWorkListQuery` / API_SPEC section 16).
pub type PageQuery = OffsetListPageParams;

pub fn page_params_from_request(request: &HttpRequest) -> OffsetListPageParams {
    let page = request
        .query_param("page")
        .or_else(|| request.query_param("pageNo"))
        .or_else(|| request.query_param("page_no"))
        .and_then(|value| value.parse::<i64>().ok());
    let page_size = request
        .query_param("page_size")
        .or_else(|| request.query_param("pageSize"))
        .and_then(|value| value.parse::<i64>().ok());
    OffsetListPageParams::parse(page, page_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_utils_rust::{offset_list_page_info, PageMode};

    #[test]
    fn page_params_default_and_clamp_page_size() {
        let request = HttpRequest::new("GET", "/items");
        let params = page_params_from_request(&request);
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 20);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn page_info_uses_offset_mode() {
        let params = OffsetListPageParams::parse(Some(1), Some(20));
        let page_info = offset_list_page_info(0, params);
        assert_eq!(page_info.mode, PageMode::Offset);
        assert_eq!(page_info.page, Some(1));
        assert_eq!(page_info.page_size, Some(20));
        assert_eq!(page_info.total_items.as_deref(), Some("0"));
    }
}
