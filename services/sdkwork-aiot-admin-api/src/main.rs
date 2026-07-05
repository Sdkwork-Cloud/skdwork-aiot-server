use std::sync::Arc;

#[tokio::main]
async fn main() {
    sdkwork_iot_platform_service::assert_production_environment_safety();
    let device_db_path = sdkwork_iot_platform_service::configured_device_db_path(
        "SDKWORK_AIOT_ADMIN_API_DEVICE_DB_PATH",
    );
    let stores = sdkwork_iot_platform_service::open_admin_service_stores(
        device_db_path.as_deref(),
        "sdkwork-aiot-admin-api",
    )
    .expect("open admin service stores");
    let server = Arc::new(
        sdkwork_iot_platform_service::standard_admin_api_server()
            .expect("admin api server")
            .with_device_repository(stores.device_repository.clone())
            .with_command_repository(stores.device_repository.clone())
            .with_event_repository(stores.device_repository.clone())
            .with_twin_repository(stores.device_repository.clone())
            .with_device_session_repository(stores.device_repository)
            .with_credential_repository(stores.credential_repository)
            .with_catalog_repository(stores.catalog_repository)
            .with_firmware_repository(stores.firmware_repository),
    );
    let plan = sdkwork_aiot_service_host::RuntimeServicePlan::standard();

    println!(
        "sdkwork-aiot-admin-api mode={:?} backend_routes={} components={}",
        server.runtime().mode(),
        plan.backend_routes.len(),
        server.runtime().component_names().len()
    );

    if std::env::var("SDKWORK_AIOT_ADMIN_API_NO_LISTEN").as_deref() == Ok("1") {
        return;
    }

    let bind_addr = std::env::var("SDKWORK_AIOT_APPLICATION_ADMIN_HTTP_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18081".to_string());
    let router = sdkwork_routes_iot_backend_api::build_wrapped_backend_api_router(server).await;
    if let Err(error) =
        sdkwork_routes_iot_backend_api::serve_backend_api_router(&bind_addr, router).await
    {
        eprintln!("sdkwork-aiot-admin-api serve_error={error}");
        std::process::exit(1);
    }
}
