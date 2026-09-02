use sdkwork_api_aiot_assembly::assemble_api_router_with_database_host;
use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_env,
};
use sdkwork_web_bootstrap::{infra_public_path_prefixes, ApiModuleRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sdkwork_web_bootstrap::init_tracing_from_env();
    sdkwork_database_sqlx::enable_process_shared_database_pool();
    let bind_address = std::env::var("SDKWORK_AIOT_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let assembly = assemble_api_router_with_database_host()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let framework = build_web_framework_builder(
        iam_web_request_context_resolver_from_env().await,
        assembly.route_manifest.clone(),
        infra_public_path_prefixes(),
    );
    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_modules(vec![assembly]);
    let app = module_registry
        .try_compose("SDKWork AIoT API")
        .map_err(std::io::Error::other)?
        .into_hosted(framework)
        .router;
    let bind_address = bind_address.parse()?;
    println!("sdkwork-api-aiot-standalone-gateway listening on http://{bind_address}");
    sdkwork_web_bootstrap::serve(app, bind_address).await?;
    Ok(())
}
