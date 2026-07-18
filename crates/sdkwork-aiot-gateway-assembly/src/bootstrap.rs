//! Gateway bootstrap for sdkwork-aiot.

use std::sync::Arc;

use axum::Router;

pub struct ApplicationAssembly {
    pub router: Router,
}

/// Assemble the aiot application router from environment variables.
///
/// This function bootstraps the aiot device database from environment variables,
/// creates the app and admin API servers, and builds wrapped routers for both
/// the app-api and backend-api surfaces.
pub async fn assemble_application_router() -> Result<ApplicationAssembly, String> {
    sdkwork_iot_platform_service::assert_production_environment_safety();
    let device_db_path = sdkwork_iot_platform_service::configured_device_db_path(
        "SDKWORK_AIOT_APPLICATION_GATEWAY_DEVICE_DB_PATH",
    );

    let app_stores = sdkwork_iot_platform_service::open_app_service_stores(
        device_db_path.as_deref(),
        "sdkwork-aiot-gateway-assembly",
    )?;
    let admin_stores = sdkwork_iot_platform_service::open_admin_service_stores(
        device_db_path.as_deref(),
        "sdkwork-aiot-gateway-assembly",
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
    let router = Router::new().merge(app_router).merge(backend_router);

    Ok(ApplicationAssembly { router })
}
