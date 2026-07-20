//! Gateway bootstrap for sdkwork-aiot.

use std::sync::Arc;

use axum::Router;

pub struct ApiAssembly {
    pub router: Router,
}

/// Assemble the aiot application router from environment variables.
///
/// This function bootstraps the aiot device database from environment variables,
/// creates the app and admin API servers, and builds wrapped routers for both
/// the app-api and backend-api surfaces.
pub async fn assemble_api_router() -> Result<ApiAssembly, String> {
    sdkwork_iot_platform_service::assert_production_environment_safety();
    let device_db_path = sdkwork_iot_platform_service::configured_device_db_path(
        "SDKWORK_AIOT_APPLICATION_GATEWAY_DEVICE_DB_PATH",
    );

    let app_stores = sdkwork_iot_platform_service::open_app_service_stores(
        device_db_path.as_deref(),
        "sdkwork-api-aiot-assembly",
    )?;
    let admin_stores = sdkwork_iot_platform_service::open_admin_service_stores(
        device_db_path.as_deref(),
        "sdkwork-api-aiot-assembly",
    )?;

    let app_server = Arc::new(
        sdkwork_iot_platform_service::standard_app_api_server()
            .map_err(|e| format!("failed to build aiot app api server: {e}"))?
            .with_device_repository(app_stores.device_repository.clone())
            .with_command_repository(app_stores.device_repository.clone())
            .with_event_repository(app_stores.device_repository.clone())
            .with_twin_repository(app_stores.device_repository)
            .with_credential_repository(app_stores.credential_repository)
            .with_catalog_repository(app_stores.catalog_repository),
    );
    let admin_server = Arc::new(
        sdkwork_iot_platform_service::standard_admin_api_server()
            .map_err(|e| format!("failed to build aiot admin api server: {e}"))?
            .with_device_repository(admin_stores.device_repository.clone())
            .with_command_repository(admin_stores.device_repository.clone())
            .with_event_repository(admin_stores.device_repository.clone())
            .with_twin_repository(admin_stores.device_repository)
            .with_credential_repository(admin_stores.credential_repository)
            .with_catalog_repository(admin_stores.catalog_repository)
            .with_firmware_repository(admin_stores.firmware_repository),
    );

    let app_router = sdkwork_routes_iot_app_api::build_wrapped_app_api_router(app_server).await;
    let backend_router =
        sdkwork_routes_iot_backend_api::build_wrapped_backend_api_router(admin_server).await;
    let router = compose_application_router(app_router, backend_router);

    Ok(ApiAssembly { router })
}

fn compose_application_router(app_router: Router, backend_router: Router) -> Router {
    let app_service = app_router.into_service();
    let backend_service = backend_router.into_service();

    Router::new()
        .route_service("/app/v3/api/iot", app_service.clone())
        .route_service("/app/v3/api/iot/{*path}", app_service)
        .route_service("/backend/v3/api/iot", backend_service.clone())
        .route_service("/backend/v3/api/iot/{*path}", backend_service)
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::compose_application_router;

    #[tokio::test]
    async fn composition_preserves_both_fallback_dispatchers() {
        let router = compose_application_router(
            Router::new().fallback(|| async { "app" }),
            Router::new().fallback(|| async { "backend" }),
        );

        assert_eq!(
            response_body(&router, "/app/v3/api/iot/devices").await,
            "app"
        );
        assert_eq!(
            response_body(&router, "/backend/v3/api/iot/devices").await,
            "backend",
        );
    }

    async fn response_body(router: &Router, uri: &str) -> String {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
