//! Gateway bootstrap for sdkwork-aiot.
//!
//! The assembly exports the indivisible `ApiAssemblyContribution` contract
//! (API_ASSEMBLY_SPEC.md section 4); the platform cloud gateway composes the
//! contribution with its process-shared PostgreSQL pool.

use std::sync::Arc;

use axum::Router;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_web_bootstrap::{
    ApiAssemblyContribution, DatabasePoolReadinessCheck, ReadinessCheck, WebModule,
};
use sdkwork_web_core::HttpRouteManifest;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract).
pub type ApiAssembly = ApiAssemblyContribution;

fn combined_route_manifest() -> HttpRouteManifest {
    let manifests = [
        sdkwork_routes_iot_app_api::gateway_route_manifest(),
        sdkwork_routes_iot_backend_api::gateway_route_manifest(),
    ];
    HttpRouteManifest::from_owned_routes(
        manifests
            .into_iter()
            .flat_map(|manifest| manifest.routes().to_vec())
            .collect(),
    )
}

fn contribution_from(
    router: Router,
    readiness_check: Arc<dyn ReadinessCheck>,
) -> Result<ApiAssembly, String> {
    ApiAssemblyContribution::from_manifest(
        "sdkwork-aiot",
        "SDKWork AIoT API",
        router,
        combined_route_manifest(),
        vec![
            Arc::new(sdkwork_routes_iot_app_api::AiotAppContextInjector),
            Arc::new(sdkwork_routes_iot_backend_api::AiotBackendContextInjector),
        ],
        readiness_check,
    )
}

/// Assemble the aiot application router from environment variables.
///
/// This function opens lifecycle-prepared AIoT persistence, creates the app and
/// admin API servers, and builds wrapped routers for both the app-api and
/// backend-api surfaces.
pub async fn assemble_api_router() -> Result<ApiAssembly, String> {
    let (app_server, admin_server) = bootstrap_aiot_servers()?;

    let app_router = sdkwork_routes_iot_app_api::build_wrapped_app_api_router(app_server).await;
    let backend_router =
        sdkwork_routes_iot_backend_api::build_wrapped_backend_api_router(admin_server).await;
    let router = compose_application_router(app_router, backend_router);

    contribution_from(router, Arc::new(sdkwork_web_bootstrap::AlwaysReady))
}

/// Assemble the AIoT contribution from the canonical environment profile with
/// lifecycle-prepared AIoT database-backed readiness for the standalone gateway.
pub async fn assemble_api_router_with_database_host() -> Result<ApiAssembly, String> {
    let database_host = sdkwork_aiot_database_host::bootstrap_aiot_database_from_env()
        .await
        .map_err(|error| format!("bootstrap AIoT database host: {error}"))?;
    let (app_server, admin_server) = bootstrap_aiot_servers()?;

    let app_router = sdkwork_routes_iot_app_api::build_wrapped_app_api_router(app_server).await;
    let backend_router =
        sdkwork_routes_iot_backend_api::build_wrapped_backend_api_router(admin_server).await;
    let router = compose_application_router(app_router, backend_router);

    contribution_from(
        router,
        Arc::new(crate::readiness::AiotDatabaseReadinessCheck::new(
            database_host.pool().clone(),
        )),
    )
}

/// Assemble the AIoT contribution against a caller-provided database pool so the
/// platform cloud gateway can share its process-wide PostgreSQL pool.
pub async fn assemble_api_router_with_pool(pool: DatabasePool) -> Result<ApiAssembly, String> {
    let database_host = sdkwork_aiot_database_host::bootstrap_aiot_database(pool)
        .await
        .map_err(|error| format!("bootstrap AIoT database host: {error}"))?;
    let (app_server, admin_server) = bootstrap_aiot_servers()?;

    let app_router = sdkwork_routes_iot_app_api::gateway_mount(app_server);
    let backend_router = sdkwork_routes_iot_backend_api::gateway_mount(admin_server);
    let router = compose_application_router(app_router, backend_router);

    contribution_from(
        router,
        Arc::new(DatabasePoolReadinessCheck::new(
            database_host.pool().clone(),
        )),
    )
}

fn bootstrap_aiot_servers() -> Result<
    (
        Arc<sdkwork_iot_platform_service::AiotApiServer>,
        Arc<sdkwork_iot_platform_service::AiotApiServer>,
    ),
    String,
> {
    sdkwork_iot_platform_service::assert_production_environment_safety();
    let app_stores =
        sdkwork_iot_platform_service::open_app_service_stores("sdkwork-api-aiot-assembly")?;
    let admin_stores =
        sdkwork_iot_platform_service::open_admin_service_stores("sdkwork-api-aiot-assembly")?;

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
    Ok((app_server, admin_server))
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

/// Canonical Web Module definition for this application
/// (API_ASSEMBLY_SPEC §4.1.1): the complete HTTP surface — every route,
/// manifest, and OpenAPI document of this owner — as one installable module.
pub async fn web_module() -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(assemble_api_router().await?))
}

/// Same as [`web_module`] but composed on a process-shared database pool
/// (platform gateways, API_ASSEMBLY_SPEC §4.1.1).
pub async fn web_module_with_pool(pool: DatabasePool) -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(
        assemble_api_router_with_pool(pool).await?,
    ))
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
