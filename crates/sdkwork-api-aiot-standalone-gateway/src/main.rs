use sdkwork_api_aiot_assembly as api_assembly;
use sdkwork_web_bootstrap::{service_router, ServiceRouterConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sdkwork_web_bootstrap::init_tracing_from_env();
    let bind_address = std::env::var("SDKWORK_AIOT_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let assembly = api_assembly::assemble_api_router()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let app = service_router(
        assembly.router,
        ServiceRouterConfig::default().with_always_ready(),
    );
    let bind_address = bind_address.parse()?;
    println!("sdkwork-api-aiot-standalone-gateway listening on http://{bind_address}");
    sdkwork_web_bootstrap::serve(app, bind_address).await?;
    Ok(())
}
