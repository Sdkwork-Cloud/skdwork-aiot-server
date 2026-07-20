const PUBLIC_INGRESS_BIND_ENV: &str = "SDKWORK_AIOT_APPLICATION_PUBLIC_INGRESS_BIND";
const LEGACY_APP_HTTP_BIND_ENV: &str = "SDKWORK_AIOT_APPLICATION_APP_HTTP_BIND";
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:18082";

#[tokio::main]
async fn main() {
    let assembly = sdkwork_api_aiot_assembly::assemble_api_router()
        .await
        .expect("assemble aiot application gateway");

    if std::env::var("SDKWORK_AIOT_APPLICATION_GATEWAY_NO_LISTEN").as_deref() == Ok("1") {
        return;
    }

    let bind_addr = std::env::var(PUBLIC_INGRESS_BIND_ENV)
        .or_else(|_| std::env::var(LEGACY_APP_HTTP_BIND_ENV))
        .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());
    println!(
        "sdkwork-aiot-standalone-gateway listening on {bind_addr} (app-api + admin-api embedded)"
    );
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind aiot standalone gateway");
    if let Err(error) = axum::serve(listener, assembly.router).await {
        eprintln!("sdkwork-aiot-standalone-gateway failed: {error}");
        std::process::exit(1);
    }
}
