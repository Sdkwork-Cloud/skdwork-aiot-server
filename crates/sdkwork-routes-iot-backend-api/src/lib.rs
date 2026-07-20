use std::sync::Arc;

use axum::Router;
use sdkwork_iam_web_adapter::{build_web_framework_layer, IamWebRequestContextResolver};
use sdkwork_web_axum::with_web_request_context;
use sdkwork_web_core::{DomainContextInjector, HttpRouteManifest, WebRequestContext};

include!(concat!(env!("OUT_DIR"), "/iot_backend_http_routes.rs"));

pub mod http_adapter;
pub mod routes;

pub fn iot_public_path_prefixes() -> Vec<String> {
    vec!["/healthz".to_owned(), "/readyz".to_owned()]
}

#[derive(Clone, Default)]
struct AiotBackendContextInjector;

impl DomainContextInjector for AiotBackendContextInjector {
    fn inject(&self, request: &mut axum::extract::Request, context: &WebRequestContext) {
        if let Some(aiot_context) = sdkwork_aiot_app_context::aiot_context_from_web_request(context)
        {
            request.extensions_mut().insert(aiot_context);
        }
    }
}

fn wrap_router_with_resolver(resolver: IamWebRequestContextResolver, router: Router) -> Router {
    let layer = build_web_framework_layer(
        resolver,
        HttpRouteManifest::new(IOT_BACKEND_HTTP_ROUTES),
        iot_public_path_prefixes(),
    )
    .with_domain_injector(Arc::new(AiotBackendContextInjector));
    with_web_request_context(router, layer)
}

pub fn wrap_router_with_web_framework(
    resolver: IamWebRequestContextResolver,
    router: Router,
) -> Router {
    wrap_router_with_resolver(resolver, router)
}

pub async fn wrap_router_with_web_framework_from_env(router: Router) -> Router {
    let resolver = sdkwork_iam_web_adapter::iam_web_request_context_resolver_from_env().await;
    wrap_router_with_resolver(resolver, router)
}

pub async fn build_wrapped_backend_api_router(
    server: Arc<sdkwork_iot_platform_service::AiotApiServer>,
) -> Router {
    let router = routes::build_sdkwork_iot_backend_api_router(server);
    wrap_router_with_web_framework_from_env(router).await
}

pub async fn serve_backend_api_router(bind_addr: &str, router: Router) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    println!("sdkwork-aiot-admin-api listening on http://{bind_addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

pub fn gateway_mount(
    server: Arc<sdkwork_iot_platform_service::AiotApiServer>,
) -> axum::Router {
    routes::build_sdkwork_iot_backend_api_router(server)
}
