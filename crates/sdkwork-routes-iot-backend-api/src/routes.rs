use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Extension, State},
    http::Request,
    response::Response,
    routing::any,
    Router,
};
use sdkwork_iot_platform_service::AiotApiServer;
use sdkwork_web_core::WebRequestContext;

use crate::http_adapter::dispatch_with_web_context;

pub fn build_sdkwork_iot_backend_api_router(server: Arc<AiotApiServer>) -> Router {
    Router::new().fallback(any(dispatch)).with_state(server)
}

async fn dispatch(
    State(server): State<Arc<AiotApiServer>>,
    Extension(web_context): Extension<WebRequestContext>,
    request: Request<Body>,
) -> Response<Body> {
    dispatch_with_web_context(server.as_ref(), web_context, request).await
}
