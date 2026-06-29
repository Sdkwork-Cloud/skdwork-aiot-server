use sdkwork_aiot_transport::{HttpRequest, HttpResponse, HttpStatus};
use sdkwork_utils_rust::{
    SdkWorkApiResponse, SdkWorkCommandData, SdkWorkPageData, SdkWorkResourceData,
    SDKWORK_TRACE_ID_HEADER,
};
use sdkwork_web_core::trace::{resolve_problem_trace_id, trace_id_from_traceparent};

use crate::pagination::{paginated_page_info, PageQuery};

pub fn resolve_trace_id(request: &HttpRequest) -> String {
    let traceparent = request
        .header("traceparent")
        .or_else(|| request.header("Traceparent"));
    let trace_from_header = traceparent.and_then(trace_id_from_traceparent);
    let request_id = request
        .header("x-request-id")
        .or_else(|| request.header("X-Request-Id"))
        .unwrap_or("aiot-request");
    resolve_problem_trace_id(request_id, trace_from_header)
}

pub fn success_json_response(status: HttpStatus, body: String, trace_id: &str) -> HttpResponse {
    HttpResponse::new(status)
        .with_header("content-type", "application/json")
        .with_header(SDKWORK_TRACE_ID_HEADER, trace_id)
        .with_body(body)
}

pub fn success_list_body(
    items_json: &str,
    page_query: PageQuery,
    total: usize,
    trace_id: &str,
) -> String {
    let page_info = paginated_page_info(page_query, total);
    let payload = SdkWorkPageData::<serde_json::Value> {
        items: parse_json_array(items_json),
        page_info,
    };
    serde_json::to_string(&SdkWorkApiResponse::success(payload, trace_id))
        .unwrap_or_else(|_| legacy_fallback_list(items_json, page_query, total, trace_id))
}

pub fn success_resource_body(data_json: &str, trace_id: &str) -> String {
    let item =
        serde_json::from_str::<serde_json::Value>(data_json).unwrap_or(serde_json::Value::Null);
    let payload = SdkWorkResourceData { item };
    serde_json::to_string(&SdkWorkApiResponse::success(payload, trace_id)).unwrap_or_else(|_| {
        format!(r#"{{"code":0,"data":{{"item":{data_json}}},"traceId":"{trace_id}"}}"#)
    })
}

pub fn success_command_body(resource_id: &str, status: Option<&str>, trace_id: &str) -> String {
    let payload = SdkWorkCommandData {
        accepted: true,
        resource_id: Some(resource_id.to_string()),
        status: status.map(str::to_string),
    };
    serde_json::to_string(&SdkWorkApiResponse::success(payload, trace_id))
        .expect("command acceptance envelope must serialize")
}

pub fn json_collection_response(
    request: &HttpRequest,
    items_joined: &str,
    page_query: PageQuery,
    total: usize,
) -> HttpResponse {
    let trace_id = resolve_trace_id(request);
    success_json_response(
        HttpStatus::Ok,
        success_list_body(items_joined, page_query, total, &trace_id),
        &trace_id,
    )
}

pub fn standard_command_acceptance_response(
    request: &HttpRequest,
    status: HttpStatus,
    resource_id: &str,
    command_status: Option<&str>,
) -> HttpResponse {
    let trace_id = resolve_trace_id(request);
    success_json_response(
        status,
        success_command_body(resource_id, command_status, &trace_id),
        &trace_id,
    )
}

pub fn standard_resource_response(
    request: &HttpRequest,
    status: HttpStatus,
    data_json: String,
) -> HttpResponse {
    let trace_id = resolve_trace_id(request);
    success_json_response(
        status,
        success_resource_body(&data_json, &trace_id),
        &trace_id,
    )
}

fn parse_json_array(items_json: &str) -> Vec<serde_json::Value> {
    if items_json.trim().is_empty() {
        return Vec::new();
    }
    format!("[{items_json}]")
        .parse::<serde_json::Value>()
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn legacy_fallback_list(
    items_json: &str,
    page_query: PageQuery,
    total: usize,
    trace_id: &str,
) -> String {
    let page_info = paginated_page_info(page_query, total);
    format!(
        r#"{{"code":0,"data":{{"items":[{items_json}],"pageInfo":{}}},"traceId":"{trace_id}"}}"#,
        serde_json::to_string(&page_info).unwrap_or_else(|_| "{}".to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_aiot_transport::HttpRequest;

    #[test]
    fn list_envelope_uses_sdkwork_v3_shape() {
        let request = HttpRequest::new("GET", "/app/v3/api/iot/devices");
        let body = success_list_body(
            "",
            PageQuery {
                page: 1,
                page_size: 20,
            },
            0,
            "trace-1",
        );
        let payload: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(payload["code"].as_i64(), Some(0));
        assert_eq!(payload["traceId"].as_str(), Some("trace-1"));
        assert!(payload["data"]["items"].is_array());
        assert_eq!(payload["data"]["pageInfo"]["mode"].as_str(), Some("offset"));
        let _ = request;
    }

    #[test]
    fn resource_envelope_wraps_item_payload() {
        let body = success_resource_body(r#"{"deviceId":"dev-1"}"#, "trace-2");
        let payload: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(payload["data"]["item"]["deviceId"].as_str(), Some("dev-1"));
    }
}
