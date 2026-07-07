use sdkwork_aiot_transport::{HttpRequest, HttpResponse, HttpStatus};
use sdkwork_utils_rust::{
    legacy_wire_result_code, offset_list_page_info, OffsetListPageParams, SdkWorkApiResponse,
    SdkWorkCommandData, SdkWorkPageData, SdkWorkProblemDetail, SdkWorkResourceData,
    SdkWorkResultCode, SDKWORK_TRACE_ID_HEADER,
};
use sdkwork_web_core::trace::{resolve_problem_trace_id, trace_id_from_traceparent};

fn apply_security_headers(response: HttpResponse) -> HttpResponse {
    response
        .with_header("x-content-type-options", "nosniff")
        .with_header("x-frame-options", "DENY")
        .with_header("referrer-policy", "no-referrer")
        .with_header("cache-control", "no-store")
        .with_header(
            "content-security-policy",
            "default-src 'none'; frame-ancestors 'none'",
        )
}

/// Maps legacy AIoT wire codes to platform numeric `SdkWorkResultCode` values.
pub fn aiot_wire_code_to_result(wire_code: &str) -> SdkWorkResultCode {
    match wire_code {
        "api.auth.missing_dual_token" | "api.auth.invalid_bearer" => {
            SdkWorkResultCode::AuthenticationRequired
        }
        "api.auth.rate_limited" => SdkWorkResultCode::RateLimitExceeded,
        "api.permission.denied" => SdkWorkResultCode::PermissionRequired,
        "api.context.missing" => SdkWorkResultCode::PermissionRequired,
        "api.context.invalid_tenant_id"
        | "api.context.invalid_organization_id"
        | "api.context.invalid_user_id"
        | "api.context.invalid_data_scope"
        | "api.request.invalid_json"
        | "api.request.invalid_field"
        | "api.request.body_read_failed"
        | "api.device.invalid_product_id"
        | "api.firmware.rollout.invalid_target_policy"
        | "api.firmware.artifact.invalid_reference" => SdkWorkResultCode::ValidationError,
        "api.request.body.required" => SdkWorkResultCode::MissingRequiredField,
        "api.route.unsupported" => SdkWorkResultCode::NotFound,
        code if code.ends_with(".not_found") => SdkWorkResultCode::NotFound,
        code if code.contains("duplicate") => SdkWorkResultCode::Conflict,
        "api.storage.write_failed"
        | "api.storage.read_failed"
        | "api.storage.read_write_failed" => SdkWorkResultCode::InternalError,
        "api.intelligence.kernel_misconfigured"
        | "api.intelligence.kernel_unavailable"
        | "api.intelligence.kernel_session_failed"
        | "api.intelligence.kernel_message_failed" => SdkWorkResultCode::ServiceUnavailable,
        _ => legacy_wire_result_code(wire_code),
    }
}

fn http_status_from_result_code(result_code: SdkWorkResultCode) -> HttpStatus {
    match result_code.http_status_code() {
        400 => HttpStatus::BadRequest,
        401 => HttpStatus::Unauthorized,
        403 => HttpStatus::Forbidden,
        404 => HttpStatus::NotFound,
        409 => HttpStatus::Conflict,
        429 => HttpStatus::TooManyRequests,
        503 => HttpStatus::ServiceUnavailable,
        _ => HttpStatus::InternalServerError,
    }
}

pub fn problem_detail_response(
    trace_id: &str,
    result_code: SdkWorkResultCode,
    detail: impl Into<String>,
) -> HttpResponse {
    let problem = SdkWorkProblemDetail::platform(result_code, detail, trace_id);
    let status = http_status_from_result_code(result_code);
    let body = serde_json::to_string(&problem).unwrap_or_else(|_| {
        format!(
            r#"{{"type":"about:blank","title":"{}","status":{},"code":{},"traceId":"{}"}}"#,
            result_code.title(),
            result_code.http_status_code(),
            result_code.as_i32(),
            trace_id
        )
    });
    apply_security_headers(
        HttpResponse::new(status)
            .with_header("content-type", "application/problem+json")
            .with_header(SDKWORK_TRACE_ID_HEADER, trace_id)
            .with_body(body),
    )
}

pub fn problem_detail_from_request(
    request: &HttpRequest,
    wire_code: &str,
    detail: impl Into<String>,
) -> HttpResponse {
    let trace_id = resolve_trace_id(request);
    problem_detail_response(&trace_id, aiot_wire_code_to_result(wire_code), detail)
}

pub fn problem_detail_from_wire_code(
    trace_id: Option<&str>,
    wire_code: &str,
    detail: impl Into<String>,
) -> HttpResponse {
    let trace_id = trace_id
        .map(str::to_string)
        .unwrap_or_else(|| resolve_problem_trace_id("aiot-request", None));
    problem_detail_response(&trace_id, aiot_wire_code_to_result(wire_code), detail)
}

pub fn domain_not_found_response(
    resource_label: &str,
    resource_id: &str,
    trace_id: Option<&str>,
) -> HttpResponse {
    let trace_id = trace_id
        .map(str::to_string)
        .unwrap_or_else(|| resolve_problem_trace_id("aiot-not-found", None));
    problem_detail_response(
        &trace_id,
        SdkWorkResultCode::NotFound,
        format!("{resource_label} '{resource_id}' was not found"),
    )
}

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
    apply_security_headers(
        HttpResponse::new(status)
            .with_header("content-type", "application/json")
            .with_header(SDKWORK_TRACE_ID_HEADER, trace_id)
            .with_body(body),
    )
}

pub fn success_list_body(
    items_json: &str,
    page_params: OffsetListPageParams,
    total: i64,
    trace_id: &str,
) -> String {
    let page_info = offset_list_page_info(total, page_params);
    let payload = SdkWorkPageData::<serde_json::Value> {
        items: parse_json_array(items_json),
        page_info,
    };
    serde_json::to_string(&SdkWorkApiResponse::success(payload, trace_id))
        .unwrap_or_else(|_| legacy_fallback_list(items_json, page_params, total, trace_id))
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
    page_params: OffsetListPageParams,
    total: i64,
) -> HttpResponse {
    let trace_id = resolve_trace_id(request);
    success_json_response(
        HttpStatus::Ok,
        success_list_body(items_joined, page_params, total, &trace_id),
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
    page_params: OffsetListPageParams,
    total: i64,
    trace_id: &str,
) -> String {
    let page_info = offset_list_page_info(total, page_params);
    format!(
        r#"{{"code":0,"data":{{"items":[{items_json}],"pageInfo":{}}},"traceId":"{trace_id}"}}"#,
        serde_json::to_string(&page_info).unwrap_or_else(|_| "{}".to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_aiot_transport::HttpRequest;
    use sdkwork_utils_rust::OffsetListPageParams;

    #[test]
    fn list_envelope_uses_sdkwork_v3_shape() {
        let request = HttpRequest::new("GET", "/app/v3/api/iot/devices");
        let body = success_list_body(
            "",
            OffsetListPageParams::parse(Some(1), Some(20)),
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
