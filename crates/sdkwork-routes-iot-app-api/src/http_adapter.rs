use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
};
use http_body_util::BodyExt;
use sdkwork_aiot_transport::{HttpRequest, HttpResponse, HttpStatus};
use sdkwork_iot_platform_service::{
    handle_resolved_api_request, resolve_api_request_from_web_context,
};
use sdkwork_web_core::WebRequestContext;

pub async fn axum_to_http_request(request: Request<Body>) -> Result<HttpRequest, HttpResponse> {
    let (parts, body) = request.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map_err(|_| internal_problem("api.request.body_read_failed", "Failed to read body"))?
        .to_bytes();

    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let (raw_path, path, query_params) = split_path_and_query(path_and_query);

    let mut http_request = HttpRequest::new(parts.method.as_str(), path);
    http_request.raw_path = raw_path;
    for (name, value) in parts.headers.iter() {
        if let Ok(value) = value.to_str() {
            http_request = http_request.with_header(name.as_str(), value);
        }
    }
    for (name, value) in query_params {
        http_request = http_request.with_query_param(name, value);
    }

    http_request.body = body_bytes.to_vec();
    Ok(http_request)
}

fn split_path_and_query(path_and_query: &str) -> (String, String, Vec<(String, String)>) {
    let mut parts = path_and_query.splitn(2, '?');
    let path = parts.next().unwrap_or("/").to_owned();
    let raw_path = path_and_query.to_owned();
    let mut query_params = Vec::new();
    if let Some(query) = parts.next() {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut segments = pair.splitn(2, '=');
            let name = segments.next().unwrap_or_default().to_owned();
            let value = segments.next().unwrap_or_default().to_owned();
            if !name.is_empty() {
                query_params.push((name, value));
            }
        }
    }
    (raw_path, path, query_params)
}

pub fn http_to_axum_response(response: HttpResponse) -> Response<Body> {
    let status =
        StatusCode::from_u16(response.status.code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    if let Some(headers) = builder.headers_mut() {
        for (name, value) in response.headers() {
            if let (Ok(name), Ok(value)) = (
                axum::http::HeaderName::from_bytes(name.as_bytes()),
                axum::http::HeaderValue::from_str(value),
            ) {
                headers.insert(name, value);
            }
        }
    }
    builder
        .body(Body::from(response.body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

pub async fn dispatch_with_web_context(
    server: &sdkwork_iot_platform_service::AiotApiServer,
    web_context: WebRequestContext,
    request: Request<Body>,
) -> Response<Body> {
    let http_request = match axum_to_http_request(request).await {
        Ok(request) => request,
        Err(problem) => return http_to_axum_response(problem),
    };
    let resolved = match resolve_api_request_from_web_context(&http_request, &web_context) {
        Ok(resolved) => resolved,
        Err(problem) => return http_to_axum_response(problem),
    };
    let response = handle_resolved_api_request(server, &resolved);
    http_to_axum_response(response)
}

fn internal_problem(code: &str, title: &str) -> HttpResponse {
    HttpResponse::new(HttpStatus::InternalServerError)
        .with_header("content-type", "application/problem+json")
        .with_body(format!(
            r#"{{"type":"about:blank","title":"{title}","status":500,"code":"{code}"}}"#
        ))
}
