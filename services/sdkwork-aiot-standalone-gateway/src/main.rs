use std::sync::Arc;

use axum::Router;

const PUBLIC_INGRESS_BIND_ENV: &str = "SDKWORK_AIOT_APPLICATION_PUBLIC_INGRESS_BIND";
const LEGACY_APP_HTTP_BIND_ENV: &str = "SDKWORK_AIOT_APPLICATION_APP_HTTP_BIND";
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:18082";

#[tokio::main]
async fn main() {
    sdkwork_iot_platform_service::assert_production_environment_safety();

    let device_db_path = sdkwork_iot_platform_service::configured_device_db_path(
        "SDKWORK_AIOT_APPLICATION_GATEWAY_DEVICE_DB_PATH",
    );
    let app_stores = sdkwork_iot_platform_service::open_app_service_stores(
        device_db_path.as_deref(),
        "sdkwork-aiot-standalone-gateway",
    )
    .expect("open app service stores");
    let admin_stores = sdkwork_iot_platform_service::open_admin_service_stores(
        device_db_path.as_deref(),
        "sdkwork-aiot-standalone-gateway",
    )
    .expect("open admin service stores");

    let app_server = Arc::new(
        sdkwork_iot_platform_service::standard_app_api_server()
            .expect("app api server")
            .with_device_repository(app_stores.device_repository.clone())
            .with_command_repository(app_stores.device_repository.clone())
            .with_event_repository(app_stores.device_repository.clone())
            .with_twin_repository(app_stores.device_repository)
            .with_credential_repository(app_stores.credential_repository)
            .with_catalog_repository(app_stores.catalog_repository),
    );
    let admin_server = Arc::new(
        sdkwork_iot_platform_service::standard_admin_api_server()
            .expect("admin api server")
            .with_device_repository(admin_stores.device_repository.clone())
            .with_command_repository(admin_stores.device_repository.clone())
            .with_event_repository(admin_stores.device_repository.clone())
            .with_twin_repository(admin_stores.device_repository)
            .with_credential_repository(admin_stores.credential_repository)
            .with_catalog_repository(admin_stores.catalog_repository)
            .with_firmware_repository(admin_stores.firmware_repository),
    );

    let app_router = sdkwork_routes_iot_app_api::build_wrapped_app_api_router(app_server).await;
    let admin_router =
        sdkwork_routes_iot_backend_api::build_wrapped_backend_api_router(admin_server).await;
    let router = Router::new().merge(app_router).merge(admin_router);

    if std::env::var("SDKWORK_AIOT_APPLICATION_GATEWAY_NO_LISTEN").as_deref() == Ok("1") {
        return;
    }

    let bind_addr = std::env::var(PUBLIC_INGRESS_BIND_ENV)
        .or_else(|_| std::env::var(LEGACY_APP_HTTP_BIND_ENV))
        .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());
    println!(
        "sdkwork-aiot-standalone-gateway listening on {bind_addr} (app-api + admin-api embedded)"
    );
    if let Err(error) = sdkwork_routes_iot_app_api::serve_app_api_router(&bind_addr, router).await {
        eprintln!("sdkwork-aiot-standalone-gateway failed: {error}");
        std::process::exit(1);
    }
}
