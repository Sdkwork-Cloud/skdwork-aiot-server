use std::sync::Arc;

use sdkwork_api_aiot_assembly as api_assembly;
use sdkwork_web_bootstrap::{service_router, ServiceRouterConfig};

mod readiness;

use readiness::AiotDatabaseReadinessCheck;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sdkwork_web_bootstrap::init_tracing_from_env();
    sdkwork_database_sqlx::enable_process_shared_database_pool();
    let bind_address = std::env::var("SDKWORK_AIOT_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let database_host = sdkwork_aiot_database_host::bootstrap_aiot_database_from_env()
        .await
        .map_err(std::io::Error::other)?;
    let assembly = api_assembly::assemble_api_router()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let app = service_router(
        assembly.router,
        ServiceRouterConfig::default().with_readiness_check(Arc::new(
            AiotDatabaseReadinessCheck::new(database_host.pool().clone()),
        )),
    );
    let bind_address = bind_address.parse()?;
    println!("sdkwork-api-aiot-standalone-gateway listening on http://{bind_address}");
    sdkwork_web_bootstrap::serve(app, bind_address).await?;
    Ok(())
}
