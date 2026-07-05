use sdkwork_aiot_storage::{
    AiotDeviceEventCreateCommand, AiotDeviceTwinRepository, AiotEventRepository,
    AiotStorageAssociation, AiotTwinPropertyUpsertCommand,
};
use sdkwork_aiot_transport::{HttpRequest, HttpStatus};
use sdkwork_iot_platform_service::{
    handle_resolved_api_request, resolve_api_request, route_contract_for_request,
    standard_admin_api_server, standard_api_route_contracts, standard_app_api_server,
    AiotApiRequestContext, AiotApiSurface, AiotResolvedApiRequest,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

mod test_env {
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub fn setup() {
        INIT.call_once(|| {
            std::env::set_var("SDKWORK_AIOT_DEV_MODE", "1");
            std::env::set_var("SDKWORK_AIOT_TRUST_PROXY_HEADERS", "1");
            std::env::set_var(
                "SDKWORK_AIOT_DEV_PERMISSIONS",
                "iot.devices.read,iot.twins.read,iot.commands.execute,iot.protocolAdapters.read,iot.runtime.read,iot.profiles.read,iot.devices.write,iot.sessions.read,iot.sessions.disconnect,iot.commands.read,iot.commands.cancel",
            );
        });
    }
}

fn handle_api_request_bytes(
    server: &sdkwork_iot_platform_service::AiotApiServer,
    bytes: &[u8],
) -> Result<String, sdkwork_iot_platform_service::AiotApiError> {
    test_env::setup();
    sdkwork_iot_platform_service::handle_api_request_bytes(server, bytes)
}

fn resolve_api_request_for_test(
    request: &HttpRequest,
) -> Result<AiotResolvedApiRequest<'_>, sdkwork_aiot_transport::HttpResponse> {
    test_env::setup();
    resolve_api_request(request)
}

#[test]
fn admin_api_server_exposes_runtime_backed_protocol_catalog() {
    let server = standard_admin_api_server().expect("admin api server");

    assert_eq!(server.surface(), AiotApiSurface::Admin);
    assert!(server.runtime().supports_protocol("xiaozhi.websocket"));

    let response = handle_api_request_bytes(
        &server,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.protocolAdapters.read\r\n\r\n",
    )
    .expect("protocol adapter catalog");

    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains(r#""code":0"#));
    assert!(response.contains(r#""data":{"items":["#));
    assert!(response.contains(r#""protocolId":"xiaozhi.websocket""#));
    assert!(response.contains(r#""pluginId":"xiaozhi""#));
    assert!(response.contains(r#""scope":"CompatibilityPlugin""#));
    assert!(response.contains(r#""codecs":["JsonText","JsonRpc","BinaryMedia"]"#));
    assert!(response.contains(r#""sessionPolicies":["StatefulDeviceSession"]"#));
    assert!(response.contains(r#""securityModes":["bearer_token","hmac"]"#));
    assert!(response.contains(r#""hardwareFamilies":["esp32","esp32_s3"]"#));
    assert!(response.contains(r#""transports":["WebSocket","Http","Mqtt","Udp"]"#));
}

#[test]
fn app_api_server_exposes_safe_device_collection_boundary() {
    let server = standard_app_api_server().expect("app api server");

    assert_eq!(server.surface(), AiotApiSurface::App);

    let response = handle_api_request_bytes(
        &server,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("device list");

    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains(r#""code":0"#));
    assert!(response.contains(r#""data":{"items":[]"#));
}

#[test]
fn admin_api_server_exposes_runtime_capacity_policy_from_standard_bundle() {
    let server = standard_admin_api_server().expect("admin api server");

    let response = handle_api_request_bytes(
        &server,
        b"GET /backend/v3/api/iot/runtime/capacity HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.runtime.read\r\n\r\n",
    )
    .expect("runtime capacity");

    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains(r#""code":0"#));
    assert!(response.contains(r#""nodeId":"local""#));
    assert!(response.contains(r#""maxConnectionsPerNode":"100000""#));
    assert!(response.contains(r#""maxSessionsPerTenant":"1000000""#));
    assert!(response.contains(r#""maxInflightPerDevice":64"#));
    assert!(response.contains(r#""sessionLeaseTtlSeconds":90"#));
    assert!(response.contains(r#""warnLag":"100000""#));
    assert!(response.contains(r#""rejectLag":"500000""#));
    assert!(response.contains(r#""deadLetterLag":"1000000""#));
    assert!(response.contains(r#""orderedDeviceCommands":true"#));
    assert!(response.contains(r#""idempotentIngest":true"#));
}

#[test]
fn app_and_admin_api_servers_share_health_and_ready_contracts() {
    for server in [
        standard_admin_api_server().unwrap(),
        standard_app_api_server().unwrap(),
    ] {
        let health =
            handle_api_request_bytes(&server, b"GET /healthz HTTP/1.1\r\nHost: local\r\n\r\n")
                .expect("health");
        assert!(health.starts_with("HTTP/1.1 200"));
        assert!(health.contains(r#""ready":true"#));

        let ready =
            handle_api_request_bytes(&server, b"GET /readyz HTTP/1.1\r\nHost: local\r\n\r\n")
                .expect("ready");
        assert!(ready.starts_with("HTTP/1.1 200"));
        assert!(ready.contains(r#""ready":true"#));
    }
}

#[test]
fn protected_api_routes_require_sdkwork_dual_token_and_resolved_appbase_context() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_tokens = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("missing tokens problem");
    assert!(missing_tokens.starts_with("HTTP/1.1 401"));
    assert!(missing_tokens.contains("application/problem+json"));
    assert!(missing_tokens.contains("api.auth.missing_dual_token"));

    let missing_context = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer missing-token\r\nAccess-Token: missing-token\r\n\r\n",
    )
    .expect("missing context problem");
    assert!(
        missing_context.starts_with("HTTP/1.1 401") || missing_context.starts_with("HTTP/1.1 403")
    );

    let invalid_context = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: tenant-a\r\nX-Sdkwork-Organization-Id: 0\r\n\r\n",
    )
    .expect("invalid context problem");
    assert!(invalid_context.starts_with("HTTP/1.1 400"));
    assert!(invalid_context.contains("api.context.invalid_tenant_id"));
}

#[test]
fn protected_api_request_resolution_exposes_appbase_context_to_downstream_handlers() {
    let admin = standard_admin_api_server().expect("admin api server");
    let request = HttpRequest::new("GET", "/backend/v3/api/iot/runtime/capacity")
        .with_header("Authorization", "Bearer app-token")
        .with_header("Access-Token", "user-token");

    let resolved = resolve_api_request_for_test(&request).expect("resolved api request");

    assert_eq!(
        resolved.request().path,
        "/backend/v3/api/iot/runtime/capacity"
    );
    match resolved.context() {
        AiotApiRequestContext::Protected(ctx) => {
            assert_eq!(ctx.tenant_id, "100001");
            assert_eq!(ctx.organization_id, "0");
            assert_eq!(ctx.user_id.as_deref(), Some("30001"));
            assert!(ctx.data_scope.is_empty());
        }
        AiotApiRequestContext::Public => panic!("protected API route must carry context"),
    }

    let response = handle_resolved_api_request(&admin, &resolved);
    assert_eq!(response.status, HttpStatus::Ok);
    assert!(response.body.contains(r#""code":0"#));
}

#[test]
fn protected_api_handler_rejects_unresolved_public_context_before_dispatch() {
    let admin = standard_admin_api_server().expect("admin api server");
    let request = HttpRequest::new("GET", "/backend/v3/api/iot/runtime/capacity");
    let unresolved = AiotResolvedApiRequest::public(&request);

    let response = handle_resolved_api_request(&admin, &unresolved);

    assert_eq!(response.status, HttpStatus::Forbidden);
    assert!(response.body.contains("api.context.missing"));
}

#[test]
fn standard_api_route_contracts_declare_surface_operation_and_permission_boundaries() {
    let contracts = standard_api_route_contracts();

    let app_devices = contracts
        .iter()
        .find(|route| route.path == "/app/v3/api/iot/devices")
        .expect("app devices route contract");
    assert_eq!(app_devices.surface, AiotApiSurface::App);
    assert_eq!(app_devices.method, "GET");
    assert_eq!(app_devices.operation_id, "devices.list");
    assert_eq!(app_devices.required_permission, "iot.devices.read");

    let backend_protocols = contracts
        .iter()
        .find(|route| route.path == "/backend/v3/api/iot/protocol_adapters")
        .expect("backend protocol adapter route contract");
    assert_eq!(backend_protocols.surface, AiotApiSurface::Admin);
    assert_eq!(backend_protocols.operation_id, "protocolAdapters.list");
    assert_eq!(
        backend_protocols.required_permission,
        "iot.protocolAdapters.read"
    );

    assert!(contracts
        .iter()
        .all(|route| route.operation_id.contains('.')));
    assert!(contracts.iter().any(|route| {
        route.operation_id == "runtime.capacity.retrieve"
            && route.required_permission == "iot.runtime.read"
    }));
    assert!(contracts.iter().any(|route| {
        route.path == "/backend/v3/api/iot/devices"
            && route.method == "POST"
            && route.operation_id == "devices.create"
            && route.required_permission == "iot.devices.write"
    }));
    assert!(contracts.iter().any(|route| {
        route.path == "/backend/v3/api/iot/devices/{deviceId}"
            && route.method == "PUT"
            && route.operation_id == "devices.update"
            && route.required_permission == "iot.devices.write"
    }));
    assert!(contracts.iter().any(|route| {
        route.path == "/backend/v3/api/iot/devices/{deviceId}"
            && route.method == "DELETE"
            && route.operation_id == "devices.delete"
            && route.required_permission == "iot.devices.delete"
    }));
}

#[test]
fn protected_api_routes_require_resolved_permission_scope_from_appbase_context() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_permission = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("missing permission problem");
    assert!(missing_permission.starts_with("HTTP/1.1 403"));
    assert!(missing_permission.contains("api.permission.denied"));
    assert!(missing_permission.contains("iot.protocolAdapters.read"));

    let allowed = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.protocolAdapters.read\r\n\r\n",
    )
    .expect("allowed protocol adapters");
    assert!(allowed.starts_with("HTTP/1.1 200"));
}

#[test]
fn templated_api_route_contracts_match_concrete_paths_before_dispatch() {
    let app = standard_app_api_server().expect("app api server");
    let request = HttpRequest::new("POST", "/app/v3/api/iot/devices/device-001/commands");
    let contract = route_contract_for_request(AiotApiSurface::App, &request)
        .expect("templated command route contract");

    assert_eq!(contract.path, "/app/v3/api/iot/devices/{deviceId}/commands");
    assert_eq!(contract.operation_id, "devices.commands.create");
    assert_eq!(contract.required_permission, "iot.commands.execute");

    let missing_permission = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("templated route permission problem");

    assert!(missing_permission.starts_with("HTTP/1.1 403"));
    assert!(missing_permission.contains("api.permission.denied"));
    assert!(missing_permission.contains("iot.commands.execute"));
}

#[test]
fn declared_backend_collection_routes_return_structured_catalog_payloads() {
    let admin = standard_admin_api_server().expect("admin api server");

    for (path, permission) in [
        ("/backend/v3/api/iot/products", "iot.products.read"),
        ("/backend/v3/api/iot/hardware_profiles", "iot.profiles.read"),
        ("/backend/v3/api/iot/protocol_profiles", "iot.profiles.read"),
        ("/backend/v3/api/iot/devices", "iot.devices.read"),
    ] {
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: {permission}\r\n\r\n"
        );

        let response = handle_api_request_bytes(&admin, request.as_bytes())
            .unwrap_or_else(|error| panic!("{path} failed with {}", error.code));

        assert!(
            response.starts_with("HTTP/1.1 200"),
            "{path} should be mounted, got {response}"
        );
        assert!(response.contains(r#""code":0"#), "{path} missing code");
        assert!(
            !response.contains("api.route.unsupported"),
            "{path} must not fall through to unsupported route"
        );

        let body = response_body_json(&response);
        let data = list_data_items(&body);
        if path == "/backend/v3/api/iot/devices" {
            assert!(
                data.is_empty(),
                "{path} should stay empty without seeded devices"
            );
            continue;
        }

        assert!(
            !data.is_empty(),
            "{path} should return standard seeded catalog entries"
        );

        match path {
            "/backend/v3/api/iot/products" => {
                assert!(data.iter().any(|entry| {
                    entry.get("productId").and_then(serde_json::Value::as_str) == Some("9001")
                        && entry
                            .get("defaultCapabilityModelId")
                            .and_then(serde_json::Value::as_str)
                            == Some("capmodel-xiaozhi-core")
                }));
            }
            "/backend/v3/api/iot/hardware_profiles" => {
                assert!(data.iter().any(|entry| {
                    entry
                        .get("hardwareProfileId")
                        .and_then(serde_json::Value::as_str)
                        == Some("hw-esp32-s3")
                        && entry.get("chipFamily").and_then(serde_json::Value::as_str)
                            == Some("esp32_s3")
                }));
            }
            "/backend/v3/api/iot/protocol_profiles" => {
                assert!(data.iter().any(|entry| {
                    entry
                        .get("protocolProfileId")
                        .and_then(serde_json::Value::as_str)
                        == Some("proto-xiaozhi")
                        && entry
                            .get("defaultProtocolId")
                            .and_then(serde_json::Value::as_str)
                            == Some("xiaozhi.websocket")
                }));
            }
            _ => {}
        }
    }
}

#[test]
fn capability_model_retrieve_route_returns_standard_model_payload() {
    let admin = standard_admin_api_server().expect("admin api server");

    let response = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/capability_models/capmodel-xiaozhi-core HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.profiles.read\r\n\r\n",
    )
    .expect("capability model retrieve");
    assert!(response.starts_with("HTTP/1.1 200"));
    let body = response_body_json(&response);
    assert_success_envelope(&body);
    assert_eq!(
        resource_data_pointer(&body, "/capabilityModelId").and_then(serde_json::Value::as_str),
        Some("capmodel-xiaozhi-core")
    );
    assert_eq!(resource_data_str(&body, "version"), Some("1.0.0"));
    assert!(
        resource_data_pointer(&body, "/capabilities")
            .and_then(serde_json::Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false),
        "capability model should include non-empty capabilities"
    );

    let not_found_response = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/capability_models/not-exists HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.profiles.read\r\n\r\n",
    )
    .expect("capability model not found");
    assert!(not_found_response.starts_with("HTTP/1.1 404"));
    let not_found_body =
        assert_problem_json_fields(&not_found_response, 404, "api.capability_model.not_found");
    assert_eq!(
        not_found_body
            .get("title")
            .and_then(serde_json::Value::as_str),
        Some("Capability model not found")
    );
}

#[test]
fn backend_device_sessions_and_capabilities_routes_are_device_scoped_and_typed() {
    let admin = standard_admin_api_server().expect("admin api server");

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"session-capability-001\",\"displayName\":\"Session Capability Device\",\"productId\":\"9001\",\"chipFamily\":\"esp32_s3\"}",
    )
    .expect("backend devices.create session-capability");
    assert!(create.starts_with("HTTP/1.1 201"));

    let sessions = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-capability-001/sessions HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.sessions.read\r\n\r\n",
    )
    .expect("backend devices.sessions.list");
    assert!(sessions.starts_with("HTTP/1.1 200"));
    let sessions_json = response_body_json(&sessions);
    let sessions_data = list_data_items(&sessions_json);
    assert_eq!(sessions_data.len(), 1, "sessions data array");
    assert_eq!(
        sessions_data[0]
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("session-capability-001")
    );
    assert_eq!(
        sessions_data[0]
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("connected")
    );
    assert_eq!(
        sessions_data[0]
            .get("transport")
            .and_then(serde_json::Value::as_str),
        Some("websocket")
    );
    assert!(sessions_data[0]
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .is_some());

    let capabilities = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-capability-001/capabilities HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.capabilities.list");
    assert!(capabilities.starts_with("HTTP/1.1 200"));
    let capabilities_json = response_body_json(&capabilities);
    let capabilities_data = list_data_items(&capabilities_json);
    assert!(capabilities_data.len() >= 3, "capabilities data array");
    assert!(capabilities_data.iter().any(|item| {
        item.get("capabilityName")
            .and_then(serde_json::Value::as_str)
            == Some("audio.capture")
    }));
    assert!(capabilities_data
        .iter()
        .all(|item| { item.get("status").and_then(serde_json::Value::as_str) == Some("enabled") }));

    let sessions_wrong_scope = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-capability-001/sessions HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.sessions.read\r\n\r\n",
    )
    .expect("backend devices.sessions.list wrong scope");
    assert!(sessions_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(sessions_wrong_scope.contains("api.device.not_found"));

    let capabilities_wrong_scope = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-capability-001/capabilities HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.capabilities.list wrong scope");
    assert!(capabilities_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(capabilities_wrong_scope.contains("api.device.not_found"));
}

#[test]
fn backend_device_session_disconnect_and_command_cancel_routes_are_scoped_and_effective() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo);

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"session-cancel-001\",\"displayName\":\"Session Cancel Device\",\"productId\":\"9501\"}",
    )
    .expect("create session-cancel device");
    assert!(create.starts_with("HTTP/1.1 201"));

    let sessions_before = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-cancel-001/sessions HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.sessions.read\r\n\r\n",
    )
    .expect("list sessions before disconnect");
    assert!(sessions_before.starts_with("HTTP/1.1 200"));
    let sessions_before_json = response_body_json(&sessions_before);
    let sessions_before_data = list_data_items(&sessions_before_json);
    assert_eq!(sessions_before_data.len(), 1, "sessions before data array");
    let session_id = sessions_before_data[0]
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .expect("session id")
        .to_string();

    let disconnect_wrong_permission = handle_api_request_bytes(
        &admin,
        format!(
            "DELETE /backend/v3/api/iot/devices/session-cancel-001/sessions/{session_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.sessions.read\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("disconnect wrong permission");
    assert!(disconnect_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(disconnect_wrong_permission.contains("api.permission.denied"));

    let disconnect = handle_api_request_bytes(
        &admin,
        format!(
            "DELETE /backend/v3/api/iot/devices/session-cancel-001/sessions/{session_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.sessions.disconnect\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("disconnect session");
    assert!(disconnect.starts_with("HTTP/1.1 204"));

    let sessions_after = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-cancel-001/sessions HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.sessions.read\r\n\r\n",
    )
    .expect("list sessions after disconnect");
    assert!(sessions_after.starts_with("HTTP/1.1 200"));
    assert_eq!(
        list_data_items(&response_body_json(&sessions_after)).len(),
        0
    );

    let disconnect_again = handle_api_request_bytes(
        &admin,
        format!(
            "DELETE /backend/v3/api/iot/devices/session-cancel-001/sessions/{session_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.sessions.disconnect\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("disconnect session again");
    assert!(disconnect_again.starts_with("HTTP/1.1 404"));
    assert!(disconnect_again.contains("api.device.session.not_found"));

    let disconnect_wrong_scope = handle_api_request_bytes(
        &admin,
        format!(
            "DELETE /backend/v3/api/iot/devices/session-cancel-001/sessions/{session_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23999\r\nX-Sdkwork-Permission-Scope: iot.sessions.disconnect\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("disconnect session wrong scope");
    assert!(disconnect_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(disconnect_wrong_scope.contains("api.device.not_found"));

    let create_command = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/session-cancel-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play\",\"payload\":{\"text\":\"cancel-me\"}}",
    )
    .expect("create command for cancel");
    assert!(create_command.starts_with("HTTP/1.1 202"));
    let create_command_json = response_body_json(&create_command);
    let command_id = command_acceptance_resource_id(&create_command_json)
        .expect("command id")
        .to_string();

    let cancel_wrong_permission = handle_api_request_bytes(
        &admin,
        format!(
            "POST /backend/v3/api/iot/devices/session-cancel-001/commands/{command_id}/cancel HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("cancel wrong permission");
    assert!(cancel_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(cancel_wrong_permission.contains("api.permission.denied"));

    let cancel = handle_api_request_bytes(
        &admin,
        format!(
            "POST /backend/v3/api/iot/devices/session-cancel-001/commands/{command_id}/cancel HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.commands.cancel\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("cancel command");
    assert!(cancel.starts_with("HTTP/1.1 200"));
    let cancel_json = response_body_json(&cancel);
    assert_command_acceptance(&cancel_json);
    assert_eq!(
        command_acceptance_resource_id(&cancel_json),
        Some(command_id.as_str())
    );
    assert_eq!(command_acceptance_status(&cancel_json), Some("cancelled"));

    let list_after_cancel = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/session-cancel-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("list commands after cancel");
    assert!(list_after_cancel.starts_with("HTTP/1.1 200"));
    let list_after_cancel_json = response_body_json(&list_after_cancel);
    let list_after_cancel_data = list_data_items(&list_after_cancel_json);
    let cancelled_command = list_after_cancel_data
        .iter()
        .find(|item| {
            item.get("commandId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == command_id)
        })
        .expect("cancelled command in list");
    assert_eq!(
        cancelled_command
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("cancelled")
    );

    let cancel_missing_command = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/session-cancel-001/commands/cmd-missing-001/cancel HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23001\r\nX-Sdkwork-Permission-Scope: iot.commands.cancel\r\n\r\n",
    )
    .expect("cancel missing command");
    assert!(cancel_missing_command.starts_with("HTTP/1.1 404"));
    assert!(cancel_missing_command.contains("api.command.not_found"));

    let cancel_wrong_scope = handle_api_request_bytes(
        &admin,
        format!(
            "POST /backend/v3/api/iot/devices/session-cancel-001/commands/{command_id}/cancel HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 23999\r\nX-Sdkwork-Permission-Scope: iot.commands.cancel\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("cancel command wrong scope");
    assert!(cancel_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(cancel_wrong_scope.contains("api.device.not_found"));
}

#[test]
fn events_routes_return_typed_event_collections_with_media_resource_identity() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    shared_repo
        .record_event(
            AiotDeviceEventCreateCommand::new(
                association.clone(),
                "device-777",
                "iot.device.media_frame.ingested",
            )
            .with_event_id("evt-device-777-0001")
            .with_event_version("1")
            .with_protocol("xiaozhi.websocket", "xiaozhi")
            .with_message_routing("mediaFrame", "audio", "websocket", "device_to_cloud")
            .with_message_id("msg-001")
            .with_correlation_id("corr-001")
            .with_trace_id("trace-001")
            .with_payload_hash(
                "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb7625e5c7c5f5a4c5d6",
            )
            .with_media(
                Some("media-res-001".to_string()),
                Some("obj-blob-001".to_string()),
                Some(
                    r#"{"id":"media-res-001","kind":"audio","source":"object_storage","objectBlobId":"obj-blob-001","mimeType":"audio/opus","sizeBytes":"4096"}"#
                        .to_string(),
                ),
            )
            .with_payload_json(r#"{"codec":"opus","sampleRate":16000}"#)
            .with_occurred_at("2026-06-01T00:00:00Z"),
        )
        .expect("record event");
    shared_repo
        .record_event(AiotDeviceEventCreateCommand::new(
            association,
            "device-999",
            "iot.device.media_frame.ingested",
        ))
        .expect("record second event");

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_event_repository(shared_repo.clone());
    let admin_response = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.telemetry.read\r\n\r\n",
    )
    .expect("backend events.list");

    assert!(admin_response.starts_with("HTTP/1.1 200"));
    assert!(admin_response.contains(r#""eventType":"iot.device.media_frame.ingested""#));
    assert!(admin_response.contains(r#""eventVersion":"1""#));
    assert!(admin_response.contains(r#""payloadHash":"#));
    assert!(admin_response.contains(r#""id":"media-res-001""#));
    assert!(admin_response.contains(r#""kind":"audio""#));
    assert!(admin_response.contains(r#""source":"object_storage""#));
    assert!(admin_response.contains(r#""payload":{"codec":"opus","sampleRate":16000}"#));
    assert!(admin_response.contains(r#""deviceId":"device-777""#));
    assert!(admin_response.contains(r#""deviceId":"device-999""#));

    let app = standard_app_api_server()
        .expect("app api server")
        .with_event_repository(shared_repo);
    let app_response = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/device-777/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app devices.events.list");

    assert!(app_response.starts_with("HTTP/1.1 200"));
    assert!(app_response.contains(r#""deviceId":"device-777""#));
    assert!(app_response.contains(r#""eventVersion":"1""#));
    assert!(!app_response.contains(r#""deviceId":"device-999""#));
}

#[test]
fn command_routes_return_typed_command_payloads_with_media_resource_fields() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());

    let app = standard_app_api_server()
        .expect("app api server")
        .with_command_repository(shared_repo.clone());
    let create_response = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-888/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\",\"payload\":{\"text\":\"xiaozhi-ready\",\"lang\":\"zh-CN\"},\"requestMediaResourceId\":\"media-res-xyz\",\"requestObjectBlobId\":\"obj-blob-xyz\",\"requestMedia\":{\"id\":\"media-res-xyz\",\"kind\":\"audio\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-xyz\",\"mimeType\":\"audio/wav\",\"sizeBytes\":\"8192\"}}",
    )
    .expect("app devices.commands.create");

    assert!(create_response.starts_with("HTTP/1.1 202"));
    let create_json = response_body_json(&create_response);
    assert_command_acceptance(&create_json);
    assert_eq!(
        command_acceptance_resource_id(&create_json),
        Some("cmd-device-888-0001")
    );
    assert_eq!(command_acceptance_status(&create_json), Some("accepted"));

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_command_repository(shared_repo);
    let list_response = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/device-888/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("backend devices.commands.list");

    assert!(list_response.starts_with("HTTP/1.1 200"));
    assert!(list_response.contains(r#""deviceId":"device-888""#));
    assert!(list_response.contains(r#""commandId":"cmd-device-888-0001""#));
    assert!(list_response.contains(r#""capabilityName":"player""#));
    assert!(list_response.contains(r#""commandName":"speak""#));
    assert!(list_response.contains(r#""requestPayload":{"lang":"zh-CN","text":"xiaozhi-ready"}"#));
    assert!(list_response.contains(r#""requestMediaResourceId":"media-res-xyz""#));
    assert!(list_response.contains(r#""requestObjectBlobId":"obj-blob-xyz""#));
    assert!(list_response.contains(
        r#""requestMedia":{"id":"media-res-xyz","kind":"audio","mimeType":"audio/wav","objectBlobId":"obj-blob-xyz","sizeBytes":"8192","source":"object_storage"}"#
    ));
    assert!(!list_response.contains(r#""requestAudioUrl":"#));
    assert!(list_response.contains(r#""status":"accepted""#));
    assert!(list_response.contains(r#""result":null"#));
}

#[test]
fn command_create_rejects_invalid_json_body() {
    let app = standard_app_api_server().expect("app api server");
    let response = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-888/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":",
    )
    .expect("invalid json response");

    assert!(response.starts_with("HTTP/1.1 400"));
    assert!(response.contains("application/problem+json"));
    assert!(response.contains("api.request.invalid_json"));
}

#[test]
fn command_create_requires_non_empty_body_and_required_fields() {
    let app = standard_app_api_server().expect("app api server");
    let missing_body = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-888/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n",
    )
    .expect("missing command body response");

    assert!(missing_body.starts_with("HTTP/1.1 400"));
    assert!(missing_body.contains("api.request.body.required"));

    let missing_payload = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-888/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\"}",
    )
    .expect("missing payload response");
    assert!(missing_payload.starts_with("HTTP/1.1 400"));
    assert!(missing_payload.contains("api.request.invalid_field"));
    assert!(missing_payload.contains("Field payload is required"));
}

#[test]
fn command_create_uses_idempotency_key_header_for_deduplication() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_command_repository(shared_repo);

    let first = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-889/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: same-command-key-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\",\"payload\":{\"text\":\"once\"}}",
    )
    .expect("first command create response");
    assert!(first.starts_with("HTTP/1.1 202"));
    let first_json = response_body_json(&first);
    assert_command_acceptance(&first_json);
    assert_eq!(
        command_acceptance_resource_id(&first_json),
        Some("cmd-device-889-0001")
    );

    let second = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-889/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: same-command-key-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\",\"payload\":{\"text\":\"once\"}}",
    )
    .expect("second command create response");
    assert!(second.starts_with("HTTP/1.1 202"));
    let second_json = response_body_json(&second);
    assert_command_acceptance(&second_json);
    assert_eq!(
        command_acceptance_resource_id(&second_json),
        Some("cmd-device-889-0001")
    );
    assert_ne!(
        command_acceptance_resource_id(&second_json),
        Some("cmd-device-889-0002")
    );
}

#[test]
fn command_create_idempotency_is_scoped_by_tenant_and_organization() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_command_repository(shared_repo);

    let org_a = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-890/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: same-tenant-different-org-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak-org-a\",\"payload\":{\"text\":\"a\"}}",
    )
    .expect("tenant=100001 org=0 command create");
    assert!(org_a.starts_with("HTTP/1.1 202"));
    let org_a_json = response_body_json(&org_a);
    let org_a_command_id = command_acceptance_resource_id(&org_a_json)
        .expect("org A command id")
        .to_string();

    let org_b = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-890/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: same-tenant-different-org-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 1\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak-org-b\",\"payload\":{\"text\":\"b\"}}",
    )
    .expect("tenant=100001 org=1 command create");
    assert!(org_b.starts_with("HTTP/1.1 202"));
    let org_b_json = response_body_json(&org_b);
    let org_b_command_id = command_acceptance_resource_id(&org_b_json)
        .expect("org B command id")
        .to_string();
    assert_ne!(
        org_a_command_id, org_b_command_id,
        "idempotency must not deduplicate across organizations under same tenant"
    );
}

#[test]
fn twin_routes_return_repository_backed_desired_and_reported_snapshots() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let association = AiotStorageAssociation::tenant_org(100001, 0);
    shared_repo
        .upsert_twin_property(
            AiotTwinPropertyUpsertCommand::new(association, "device-001", "volume")
                .with_desired_value_json("80")
                .with_reported_value_json("72")
                .with_desired_updated_at("2026-06-01T00:00:01Z")
                .with_reported_updated_at("2026-06-01T00:00:02Z"),
        )
        .expect("upsert twin property");

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_twin_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_twin_repository(shared_repo);

    let admin_response = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("backend devices.twin.retrieve");
    assert!(admin_response.starts_with("HTTP/1.1 200"));
    assert!(admin_response.contains(r#""deviceId":"device-001""#));
    assert!(admin_response.contains(r#""desired":{"volume":80}"#));
    assert!(admin_response.contains(r#""reported":{"volume":72}"#));

    let app_response = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/device-404/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("app devices.twin.retrieve");
    assert!(app_response.starts_with("HTTP/1.1 200"));
    assert!(app_response.contains(r#""deviceId":"device-404""#));
    assert!(app_response.contains(r#""desired":{}"#));
    assert!(app_response.contains(r#""reported":{}"#));
}

#[test]
fn admin_and_app_end_to_end_flow_matches_sdk_response_shapes() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());

    let create_device = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"e2e-device-001\",\"displayName\":\"E2E Device\",\"productId\":\"9001\",\"clientId\":\"e2e-client-001\",\"chipFamily\":\"esp32_s3\"}",
    )
    .expect("backend devices.create e2e");
    assert!(create_device.starts_with("HTTP/1.1 201"));
    let create_device_json = response_body_json(&create_device);
    assert_success_envelope(&create_device_json);
    assert_json_string_at(&create_device_json, "/data/id");
    assert_json_string_at(&create_device_json, "/data/tenantId");
    assert_json_string_at(&create_device_json, "/data/organizationId");
    assert_eq!(
        resource_data_str(&create_device_json, "deviceId"),
        Some("e2e-device-001")
    );
    assert_json_string_at(&create_device_json, "/data/displayName");
    assert_json_string_at(&create_device_json, "/data/productId");
    assert_json_string_at(&create_device_json, "/data/status");

    let app_retrieve = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app devices.retrieve e2e");
    assert!(app_retrieve.starts_with("HTTP/1.1 200"));
    let app_retrieve_json = response_body_json(&app_retrieve);
    assert_success_envelope(&app_retrieve_json);
    assert_eq!(
        resource_data_str(&app_retrieve_json, "deviceId"),
        Some("e2e-device-001")
    );

    let app_list = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app devices.list e2e");
    assert!(app_list.starts_with("HTTP/1.1 200"));
    let app_list_json = response_body_json(&app_list);
    let app_devices = list_data_items(&app_list_json);
    let app_e2e_device = app_devices
        .iter()
        .find(|device| {
            device
                .get("deviceId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == "e2e-device-001")
        })
        .expect("app e2e device");
    assert!(app_e2e_device
        .get("status")
        .and_then(serde_json::Value::as_str)
        .is_some());

    let app_command_create = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/e2e-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: e2e-command-key-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play\",\"payload\":{\"text\":\"hello-e2e\",\"lang\":\"zh-CN\"},\"requestMedia\":{\"id\":\"media-e2e-001\",\"kind\":\"audio\",\"source\":\"object_storage\",\"objectBlobId\":\"blob-e2e-001\",\"mimeType\":\"audio/opus\",\"sizeBytes\":\"2048\"}}",
    )
    .expect("app devices.commands.create e2e");
    assert!(app_command_create.starts_with("HTTP/1.1 202"));
    let app_command_create_json = response_body_json(&app_command_create);
    assert_command_acceptance(&app_command_create_json);
    assert_eq!(
        command_acceptance_status(&app_command_create_json),
        Some("accepted")
    );

    let command_id = command_acceptance_resource_id(&app_command_create_json).expect("command id");

    let admin_command_list = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/e2e-device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("backend devices.commands.list e2e");
    assert!(admin_command_list.starts_with("HTTP/1.1 200"));
    let admin_command_list_json = response_body_json(&admin_command_list);
    let admin_commands = list_data_items(&admin_command_list_json);
    let listed_command = admin_commands
        .iter()
        .find(|command| {
            command
                .get("commandId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == command_id)
        })
        .expect("listed e2e command");
    assert_eq!(
        listed_command
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("e2e-device-001")
    );
    assert!(listed_command.get("requestPayload").is_some());

    shared_repo
        .record_event(
            AiotDeviceEventCreateCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "e2e-device-001",
                "iot.device.media_frame.ingested",
            )
            .with_event_id("evt-e2e-0001")
            .with_event_version("1")
            .with_protocol("xiaozhi.websocket", "xiaozhi")
            .with_message_routing("mediaFrame", "audio", "websocket", "device_to_cloud")
            .with_trace_id("trace-e2e-0001")
            .with_payload_hash(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .with_media(
                Some("media-e2e-001".to_string()),
                Some("blob-e2e-001".to_string()),
                Some(
                    r#"{"id":"media-e2e-001","kind":"audio","source":"object_storage","objectBlobId":"blob-e2e-001","mimeType":"audio/opus","sizeBytes":"2048"}"#
                        .to_string(),
                ),
            )
            .with_payload_json(r#"{"codec":"opus","sampleRate":16000}"#)
            .with_occurred_at("2026-06-01T12:00:00Z"),
        )
        .expect("record e2e event");

    shared_repo
        .upsert_twin_property(
            AiotTwinPropertyUpsertCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "e2e-device-001",
                "volume",
            )
            .with_desired_value_json("85")
            .with_reported_value_json("82")
            .with_desired_updated_at("2026-06-01T12:00:01Z")
            .with_reported_updated_at("2026-06-01T12:00:02Z"),
        )
        .expect("upsert e2e twin");

    let app_events = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-device-001/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app devices.events.list e2e");
    assert!(app_events.starts_with("HTTP/1.1 200"));
    let app_events_json = response_body_json(&app_events);
    let app_events_data = list_data_items(&app_events_json);
    let e2e_event = app_events_data
        .iter()
        .find(|event| {
            event
                .get("eventId")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == "evt-e2e-0001")
        })
        .expect("e2e event");
    assert_json_string_value(e2e_event, "eventType");
    assert_json_string_value(e2e_event, "eventVersion");
    assert_eq!(
        e2e_event
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("e2e-device-001")
    );
    assert_json_string_value(e2e_event, "protocolId");
    assert_json_string_value(e2e_event, "adapterId");
    assert_json_string_value(e2e_event, "messageClass");
    assert_json_string_value(e2e_event, "semanticType");
    assert_json_string_value(e2e_event, "transport");
    assert_json_string_value(e2e_event, "direction");
    assert!(e2e_event.get("payload").is_some());
    assert_json_string_value(e2e_event, "occurredAt");

    let app_twin = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("app devices.twin.retrieve e2e");
    assert!(app_twin.starts_with("HTTP/1.1 200"));
    let app_twin_json = response_body_json(&app_twin);
    assert_success_envelope(&app_twin_json);
    assert_eq!(
        resource_data_str(&app_twin_json, "deviceId"),
        Some("e2e-device-001")
    );
    assert_eq!(
        resource_data_pointer(&app_twin_json, "/desired/volume")
            .and_then(serde_json::Value::as_i64),
        Some(85)
    );
    assert_eq!(
        resource_data_pointer(&app_twin_json, "/reported/volume")
            .and_then(serde_json::Value::as_i64),
        Some(82)
    );
    assert_json_string_at(&app_twin_json, "/data/desiredVersion");
    assert_json_string_at(&app_twin_json, "/data/reportedVersion");
    assert_json_string_at(&app_twin_json, "/data/updatedAt");
}

#[test]
fn admin_and_app_end_to_end_flow_enforces_minimum_permissions_per_step() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());

    let create_denied = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n{\"deviceId\":\"e2e-auth-device-001\",\"displayName\":\"E2E Auth Device\",\"productId\":\"9101\"}",
    )
    .expect("devices.create denied");
    assert!(create_denied.starts_with("HTTP/1.1 403"));
    assert!(create_denied.contains("api.permission.denied"));
    assert!(create_denied.contains("iot.devices.write"));

    let create_allowed = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"e2e-auth-device-001\",\"displayName\":\"E2E Auth Device\",\"productId\":\"9101\"}",
    )
    .expect("devices.create allowed");
    assert!(create_allowed.starts_with("HTTP/1.1 201"));

    let retrieve_denied = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n",
    )
    .expect("devices.retrieve denied");
    assert!(retrieve_denied.starts_with("HTTP/1.1 403"));
    assert!(retrieve_denied.contains("api.permission.denied"));
    assert!(retrieve_denied.contains("iot.devices.read"));

    let retrieve_allowed = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("devices.retrieve allowed");
    assert!(retrieve_allowed.starts_with("HTTP/1.1 200"));

    let command_create_denied = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/e2e-auth-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play\",\"payload\":{\"text\":\"hello\"}}",
    )
    .expect("devices.commands.create denied");
    assert!(command_create_denied.starts_with("HTTP/1.1 403"));
    assert!(command_create_denied.contains("api.permission.denied"));
    assert!(command_create_denied.contains("iot.commands.execute"));

    let command_create_allowed = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/e2e-auth-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play\",\"payload\":{\"text\":\"hello\"}}",
    )
    .expect("devices.commands.create allowed");
    assert!(command_create_allowed.starts_with("HTTP/1.1 202"));

    let command_list_denied = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/e2e-auth-device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("devices.commands.list denied");
    assert!(command_list_denied.starts_with("HTTP/1.1 403"));
    assert!(command_list_denied.contains("api.permission.denied"));
    assert!(command_list_denied.contains("iot.commands.read"));

    let command_list_allowed = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/e2e-auth-device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("devices.commands.list allowed");
    assert!(command_list_allowed.starts_with("HTTP/1.1 200"));

    shared_repo
        .record_event(
            AiotDeviceEventCreateCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "e2e-auth-device-001",
                "iot.device.media_frame.ingested",
            )
            .with_event_id("evt-e2e-auth-0001")
            .with_payload_json(r#"{"codec":"opus"}"#),
        )
        .expect("record auth e2e event");

    let app_events_denied = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("devices.events.list denied");
    assert!(app_events_denied.starts_with("HTTP/1.1 403"));
    assert!(app_events_denied.contains("api.permission.denied"));
    assert!(app_events_denied.contains("iot.devices.read"));

    let app_events_allowed = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("devices.events.list allowed");
    assert!(app_events_allowed.starts_with("HTTP/1.1 200"));

    shared_repo
        .upsert_twin_property(
            AiotTwinPropertyUpsertCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "e2e-auth-device-001",
                "volume",
            )
            .with_desired_value_json("66")
            .with_reported_value_json("65"),
        )
        .expect("upsert auth e2e twin");

    let app_twin_denied = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("devices.twin.retrieve denied");
    assert!(app_twin_denied.starts_with("HTTP/1.1 403"));
    assert!(app_twin_denied.contains("api.permission.denied"));
    assert!(app_twin_denied.contains("iot.twins.read"));

    let app_twin_allowed = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/e2e-auth-device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("devices.twin.retrieve allowed");
    assert!(app_twin_allowed.starts_with("HTTP/1.1 200"));
}

#[test]
fn app_and_admin_routes_isolate_state_between_tenants_and_organizations() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo.clone());

    let create_tenant_a = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"scope-device-001\",\"displayName\":\"Scope Device A\",\"productId\":\"9201\"}",
    )
    .expect("create tenant A device");
    assert!(create_tenant_a.starts_with("HTTP/1.1 201"));

    let create_tenant_b = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"scope-device-001\",\"displayName\":\"Scope Device B\",\"productId\":\"9201\"}",
    )
    .expect("create tenant B device");
    assert!(create_tenant_b.starts_with("HTTP/1.1 201"));

    let app_retrieve_tenant_a = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app retrieve tenant A");
    assert!(app_retrieve_tenant_a.starts_with("HTTP/1.1 200"));
    let app_retrieve_tenant_a_json = response_body_json(&app_retrieve_tenant_a);
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_a_json, "displayName"),
        Some("Scope Device A")
    );
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_a_json, "tenantId"),
        Some("100001")
    );
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_a_json, "organizationId"),
        Some("0")
    );

    let app_retrieve_tenant_b = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app retrieve tenant B");
    assert!(app_retrieve_tenant_b.starts_with("HTTP/1.1 200"));
    let app_retrieve_tenant_b_json = response_body_json(&app_retrieve_tenant_b);
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_b_json, "displayName"),
        Some("Scope Device B")
    );
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_b_json, "tenantId"),
        Some("10002")
    );
    assert_eq!(
        resource_data_str(&app_retrieve_tenant_b_json, "organizationId"),
        Some("20002")
    );

    let app_list_tenant_a = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app list tenant A");
    assert!(app_list_tenant_a.starts_with("HTTP/1.1 200"));
    let app_list_tenant_a_json = response_body_json(&app_list_tenant_a);
    let app_list_tenant_a_data = list_data_items(&app_list_tenant_a_json);
    assert_eq!(app_list_tenant_a_data.len(), 1);
    assert_eq!(
        app_list_tenant_a_data[0]
            .get("displayName")
            .and_then(serde_json::Value::as_str),
        Some("Scope Device A")
    );

    let app_list_tenant_b = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app list tenant B");
    assert!(app_list_tenant_b.starts_with("HTTP/1.1 200"));
    let app_list_tenant_b_json = response_body_json(&app_list_tenant_b);
    let app_list_tenant_b_data = list_data_items(&app_list_tenant_b_json);
    assert_eq!(app_list_tenant_b_data.len(), 1);
    assert_eq!(
        app_list_tenant_b_data[0]
            .get("displayName")
            .and_then(serde_json::Value::as_str),
        Some("Scope Device B")
    );

    let create_command_tenant_a = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/scope-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: scope-cmd-a-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play-a\",\"payload\":{\"text\":\"a\"}}",
    )
    .expect("create command tenant A");
    assert!(create_command_tenant_a.starts_with("HTTP/1.1 202"));

    let create_command_tenant_b = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/scope-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nIdempotency-Key: scope-cmd-b-001\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"speaker\",\"commandName\":\"play-b\",\"payload\":{\"text\":\"b\"}}",
    )
    .expect("create command tenant B");
    assert!(create_command_tenant_b.starts_with("HTTP/1.1 202"));

    let admin_commands_tenant_a = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/scope-device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("admin commands tenant A");
    assert!(admin_commands_tenant_a.starts_with("HTTP/1.1 200"));
    let admin_commands_tenant_a_json = response_body_json(&admin_commands_tenant_a);
    let admin_commands_tenant_a_data = list_data_items(&admin_commands_tenant_a_json);
    assert_eq!(admin_commands_tenant_a_data.len(), 1);
    assert_eq!(
        admin_commands_tenant_a_data[0]
            .get("commandName")
            .and_then(serde_json::Value::as_str),
        Some("play-a")
    );

    let admin_commands_tenant_b = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/scope-device-001/commands HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.commands.read\r\n\r\n",
    )
    .expect("admin commands tenant B");
    assert!(admin_commands_tenant_b.starts_with("HTTP/1.1 200"));
    let admin_commands_tenant_b_json = response_body_json(&admin_commands_tenant_b);
    let admin_commands_tenant_b_data = list_data_items(&admin_commands_tenant_b_json);
    assert_eq!(admin_commands_tenant_b_data.len(), 1);
    assert_eq!(
        admin_commands_tenant_b_data[0]
            .get("commandName")
            .and_then(serde_json::Value::as_str),
        Some("play-b")
    );

    shared_repo
        .record_event(
            AiotDeviceEventCreateCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "scope-device-001",
                "iot.device.media_frame.ingested",
            )
            .with_event_id("evt-scope-a-001")
            .with_payload_json(r#"{"codec":"opus","tenant":"a"}"#),
        )
        .expect("record scoped event A");
    shared_repo
        .record_event(
            AiotDeviceEventCreateCommand::new(
                AiotStorageAssociation::tenant_org(10002, 20002),
                "scope-device-001",
                "iot.device.media_frame.ingested",
            )
            .with_event_id("evt-scope-b-001")
            .with_payload_json(r#"{"codec":"opus","tenant":"b"}"#),
        )
        .expect("record scoped event B");

    let app_events_tenant_a = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app events tenant A");
    assert!(app_events_tenant_a.starts_with("HTTP/1.1 200"));
    let app_events_tenant_a_json = response_body_json(&app_events_tenant_a);
    let app_events_tenant_a_data = list_data_items(&app_events_tenant_a_json);
    assert_eq!(app_events_tenant_a_data.len(), 1);
    assert_eq!(
        app_events_tenant_a_data[0]
            .get("eventId")
            .and_then(serde_json::Value::as_str),
        Some("evt-scope-a-001")
    );

    let app_events_tenant_b = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001/events HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app events tenant B");
    assert!(app_events_tenant_b.starts_with("HTTP/1.1 200"));
    let app_events_tenant_b_json = response_body_json(&app_events_tenant_b);
    let app_events_tenant_b_data = list_data_items(&app_events_tenant_b_json);
    assert_eq!(app_events_tenant_b_data.len(), 1);
    assert_eq!(
        app_events_tenant_b_data[0]
            .get("eventId")
            .and_then(serde_json::Value::as_str),
        Some("evt-scope-b-001")
    );

    shared_repo
        .upsert_twin_property(
            AiotTwinPropertyUpsertCommand::new(
                AiotStorageAssociation::tenant_org(100001, 0),
                "scope-device-001",
                "volume",
            )
            .with_desired_value_json("31")
            .with_reported_value_json("30"),
        )
        .expect("upsert scoped twin A");
    shared_repo
        .upsert_twin_property(
            AiotTwinPropertyUpsertCommand::new(
                AiotStorageAssociation::tenant_org(10002, 20002),
                "scope-device-001",
                "volume",
            )
            .with_desired_value_json("91")
            .with_reported_value_json("90"),
        )
        .expect("upsert scoped twin B");

    let app_twin_tenant_a = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("app twin tenant A");
    assert!(app_twin_tenant_a.starts_with("HTTP/1.1 200"));
    let app_twin_tenant_a_json = response_body_json(&app_twin_tenant_a);
    assert_eq!(
        resource_data_pointer(&app_twin_tenant_a_json, "/desired/volume")
            .and_then(serde_json::Value::as_i64),
        Some(31)
    );
    assert_eq!(
        resource_data_pointer(&app_twin_tenant_a_json, "/reported/volume")
            .and_then(serde_json::Value::as_i64),
        Some(30)
    );

    let app_twin_tenant_b = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/scope-device-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 10002\r\nX-Sdkwork-Organization-Id: 20002\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("app twin tenant B");
    assert!(app_twin_tenant_b.starts_with("HTTP/1.1 200"));
    let app_twin_tenant_b_json = response_body_json(&app_twin_tenant_b);
    assert_eq!(
        resource_data_pointer(&app_twin_tenant_b_json, "/desired/volume")
            .and_then(serde_json::Value::as_i64),
        Some(91)
    );
    assert_eq!(
        resource_data_pointer(&app_twin_tenant_b_json, "/reported/volume")
            .and_then(serde_json::Value::as_i64),
        Some(90)
    );
}

#[test]
fn backend_device_crud_and_credentials_are_scoped_by_tenant_and_organization() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo);

    let create_org_a = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"crud-scope-001\",\"displayName\":\"CRUD Scope A\",\"productId\":\"9301\"}",
    )
    .expect("create device org A");
    assert!(create_org_a.starts_with("HTTP/1.1 201"));

    let create_org_b = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21002\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"crud-scope-001\",\"displayName\":\"CRUD Scope B\",\"productId\":\"9301\"}",
    )
    .expect("create device org B");
    assert!(create_org_b.starts_with("HTTP/1.1 201"));

    let update_org_a = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":\"CRUD Scope A Updated\"}",
    )
    .expect("update device org A");
    assert!(update_org_a.starts_with("HTTP/1.1 200"));

    let retrieve_org_a = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("retrieve device org A");
    assert!(retrieve_org_a.starts_with("HTTP/1.1 200"));
    let retrieve_org_a_json = response_body_json(&retrieve_org_a);
    assert_eq!(
        resource_data_str(&retrieve_org_a_json, "displayName"),
        Some("CRUD Scope A Updated")
    );

    let retrieve_org_b = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21002\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("retrieve device org B");
    assert!(retrieve_org_b.starts_with("HTTP/1.1 200"));
    let retrieve_org_b_json = response_body_json(&retrieve_org_b);
    assert_eq!(
        resource_data_str(&retrieve_org_b_json, "displayName"),
        Some("CRUD Scope B")
    );

    let credential_org_a = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/crud-scope-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("create credential org A");
    assert!(credential_org_a.starts_with("HTTP/1.1 201"));
    assert!(credential_org_a.contains(r#""deviceId":"crud-scope-001""#));

    let credential_list_org_a = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001/credentials HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("list credentials org A");
    assert!(credential_list_org_a.starts_with("HTTP/1.1 200"));
    let credential_list_org_a_json = response_body_json(&credential_list_org_a);
    let credential_list_org_a_data = list_data_items(&credential_list_org_a_json);
    assert_eq!(credential_list_org_a_data.len(), 1);
    let credential_id = credential_list_org_a_data[0]
        .get("credentialId")
        .and_then(serde_json::Value::as_str)
        .expect("credential id")
        .to_string();
    assert_eq!(
        credential_list_org_a_data[0]
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("crud-scope-001")
    );
    assert_eq!(
        credential_list_org_a_data[0]
            .get("credentialType")
            .and_then(serde_json::Value::as_str),
        Some("hmac")
    );
    assert_eq!(
        credential_list_org_a_data[0]
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("active")
    );

    let credential_wrong_scope = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/crud-scope-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21999\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("create credential wrong scope");
    assert!(credential_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(credential_wrong_scope.contains("api.device.not_found"));

    let credential_list_wrong_scope = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001/credentials HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21999\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("list credential wrong scope");
    assert!(credential_list_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(credential_list_wrong_scope.contains("api.device.not_found"));

    let credential_delete_org_a = handle_api_request_bytes(
        &admin,
        format!(
            "DELETE /backend/v3/api/iot/devices/crud-scope-001/credentials/{credential_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("delete credential org A");
    assert!(credential_delete_org_a.starts_with("HTTP/1.1 204"));

    let credential_retrieve_after_delete = handle_api_request_bytes(
        &admin,
        format!(
            "GET /backend/v3/api/iot/devices/crud-scope-001/credentials/{credential_id} HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("retrieve credential after delete");
    assert!(credential_retrieve_after_delete.starts_with("HTTP/1.1 200"));
    let credential_retrieve_after_delete_json =
        response_body_json(&credential_retrieve_after_delete);
    assert_eq!(
        resource_data_str(&credential_retrieve_after_delete_json, "credentialId"),
        Some(credential_id.as_str())
    );
    assert_eq!(
        resource_data_str(&credential_retrieve_after_delete_json, "status"),
        Some("revoked")
    );

    let credential_list_after_delete = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001/credentials HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("list credentials after delete");
    assert!(credential_list_after_delete.starts_with("HTTP/1.1 200"));
    let credential_list_after_delete_json = response_body_json(&credential_list_after_delete);
    let credential_list_after_delete_data = list_data_items(&credential_list_after_delete_json);
    assert_eq!(credential_list_after_delete_data.len(), 1);
    assert_eq!(
        credential_list_after_delete_data[0]
            .get("credentialId")
            .and_then(serde_json::Value::as_str),
        Some(credential_id.as_str())
    );
    assert_eq!(
        credential_list_after_delete_data[0]
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("revoked")
    );
    assert!(credential_list_after_delete_data[0]
        .get("revokedAt")
        .and_then(serde_json::Value::as_str)
        .is_some());

    let missing_credential_delete = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/devices/crud-scope-001/credentials/credential-missing-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n",
    )
    .expect("delete missing credential");
    assert!(missing_credential_delete.starts_with("HTTP/1.1 404"));
    assert!(missing_credential_delete.contains("api.device.credential.not_found"));

    let missing_credential_retrieve = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001/credentials/credential-missing-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("retrieve missing credential");
    assert!(missing_credential_retrieve.starts_with("HTTP/1.1 404"));
    assert!(missing_credential_retrieve.contains("api.device.credential.not_found"));

    let delete_org_a = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.delete\r\n\r\n",
    )
    .expect("delete device org A");
    assert!(delete_org_a.starts_with("HTTP/1.1 204"));

    let retrieve_deleted_org_a = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21001\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("retrieve deleted device org A");
    assert!(retrieve_deleted_org_a.starts_with("HTTP/1.1 404"));
    assert!(retrieve_deleted_org_a.contains("api.device.not_found"));

    let retrieve_org_b_after_a_delete = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/crud-scope-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 21002\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("retrieve device org B after org A delete");
    assert!(retrieve_org_b_after_a_delete.starts_with("HTTP/1.1 200"));
    let retrieve_org_b_after_delete_json = response_body_json(&retrieve_org_b_after_a_delete);
    assert_eq!(
        resource_data_str(&retrieve_org_b_after_delete_json, "displayName"),
        Some("CRUD Scope B")
    );
}

#[test]
fn backend_device_twin_update_is_scoped_and_validated() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo);

    let create_org_a = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"twin-update-001\",\"displayName\":\"Twin Scope A\",\"productId\":\"9401\"}",
    )
    .expect("create org A twin device");
    assert!(create_org_a.starts_with("HTTP/1.1 201"));

    let create_org_b = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22002\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"twin-update-001\",\"displayName\":\"Twin Scope B\",\"productId\":\"9401\"}",
    )
    .expect("create org B twin device");
    assert!(create_org_b.starts_with("HTTP/1.1 201"));

    let update_twin_org_a = handle_api_request_bytes(
        &admin,
        b"PATCH /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.twins.write\r\n\r\n{\"desired\":{\"volume\":42,\"lang\":\"zh-CN\"},\"reported\":{\"volume\":40,\"ready\":true}}",
    )
    .expect("update twin org A");
    assert!(update_twin_org_a.starts_with("HTTP/1.1 200"));
    let update_twin_org_a_json = response_body_json(&update_twin_org_a);
    assert_eq!(
        resource_data_pointer(&update_twin_org_a_json, "/desired/volume")
            .and_then(serde_json::Value::as_i64),
        Some(42)
    );
    assert_eq!(
        resource_data_pointer(&update_twin_org_a_json, "/reported/ready")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );

    let update_twin_wrong_permission = handle_api_request_bytes(
        &admin,
        b"PATCH /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n{\"desired\":{\"volume\":41}}",
    )
    .expect("update twin with read permission");
    assert!(update_twin_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(update_twin_wrong_permission.contains("api.permission.denied"));

    let update_twin_invalid_json = handle_api_request_bytes(
        &admin,
        b"PATCH /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.twins.write\r\n\r\n{\"desired\":",
    )
    .expect("update twin invalid json");
    assert!(update_twin_invalid_json.starts_with("HTTP/1.1 400"));
    assert!(update_twin_invalid_json.contains("api.request.invalid_json"));

    let update_twin_invalid_field = handle_api_request_bytes(
        &admin,
        b"PATCH /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.twins.write\r\n\r\n{\"desired\":[1,2,3]}",
    )
    .expect("update twin invalid desired shape");
    assert!(update_twin_invalid_field.starts_with("HTTP/1.1 400"));
    assert!(update_twin_invalid_field.contains("api.request.invalid_field"));

    let retrieve_twin_org_a = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22001\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("retrieve twin org A");
    assert!(retrieve_twin_org_a.starts_with("HTTP/1.1 200"));
    assert_eq!(
        resource_data_pointer(&response_body_json(&retrieve_twin_org_a), "/desired/volume")
            .and_then(serde_json::Value::as_i64),
        Some(42)
    );

    let retrieve_twin_org_b = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22002\r\nX-Sdkwork-Permission-Scope: iot.twins.read\r\n\r\n",
    )
    .expect("retrieve twin org B");
    assert!(retrieve_twin_org_b.starts_with("HTTP/1.1 200"));
    assert_eq!(
        resource_data_pointer(&response_body_json(&retrieve_twin_org_b), "/desired")
            .and_then(serde_json::Value::as_object)
            .map(serde_json::Map::len),
        Some(0)
    );

    let update_twin_wrong_scope = handle_api_request_bytes(
        &admin,
        b"PATCH /backend/v3/api/iot/devices/twin-update-001/twin HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 22999\r\nX-Sdkwork-Permission-Scope: iot.twins.write\r\n\r\n{\"desired\":{\"volume\":11}}",
    )
    .expect("update twin wrong scope");
    assert!(update_twin_wrong_scope.starts_with("HTTP/1.1 404"));
    assert!(update_twin_wrong_scope.contains("api.device.not_found"));
}

#[test]
fn declared_backend_device_detail_routes_are_mounted_with_expected_status_codes() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_before_create = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.retrieve before create");
    assert!(missing_before_create.starts_with("HTTP/1.1 404"));
    assert!(missing_before_create.contains("api.device.not_found"));

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"device-001\",\"displayName\":\"Front Door\",\"productId\":\"1001\",\"clientId\":\"client-1\",\"chipFamily\":\"esp32_s3\"}",
    )
    .expect("backend devices.create");
    assert!(create.starts_with("HTTP/1.1 201"));
    assert!(create.contains(r#""deviceId":"device-001""#));
    assert!(create.contains(r#""displayName":"Front Door""#));
    assert!(create.contains(r#""productId":"1001""#));
    assert!(create.contains(r#""clientId":"client-1""#));
    assert!(create.contains(r#""chipFamily":"esp32_s3""#));
    assert!(create.contains(r#""status":"active""#));

    let retrieve = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.retrieve");
    assert!(retrieve.starts_with("HTTP/1.1 200"));
    assert!(retrieve.contains(r#""deviceId":"device-001""#));
    assert!(retrieve.contains(r#""displayName":"Front Door""#));

    let list = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.list");
    assert!(list.starts_with("HTTP/1.1 200"));
    assert!(list.contains(r#""deviceId":"device-001""#));

    let update = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/devices/device-001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":\"Front Door Updated\",\"status\":\"inactive\",\"metadata\":{\"firmware\":\"1.0.1\"}}",
    )
    .expect("backend devices.update");
    assert!(update.starts_with("HTTP/1.1 200"));
    assert!(update.contains(r#""displayName":"Front Door Updated""#));
    assert!(update.contains(r#""status":"inactive""#));
    assert!(update.contains(r#""metadata":{"firmware":"1.0.1"}"#));

    let delete = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/devices/device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.delete\r\n\r\n",
    )
    .expect("backend devices.delete");
    assert!(delete.starts_with("HTTP/1.1 204"));

    let missing_after_delete = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/device-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.retrieve after delete");
    assert!(missing_after_delete.starts_with("HTTP/1.1 404"));
    assert!(missing_after_delete.contains("api.device.not_found"));
}

#[test]
fn backend_device_create_validates_required_fields_and_duplicate_ids() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_required = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":\"Only Name\"}",
    )
    .expect("backend devices.create missing required");
    assert!(missing_required.starts_with("HTTP/1.1 400"));
    assert!(missing_required.contains("api.request.invalid_field"));

    let first_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"dup-device\",\"displayName\":\"Dup\",\"productId\":\"1001\"}",
    )
    .expect("backend devices.create first");
    assert!(first_create.starts_with("HTTP/1.1 201"));

    let duplicate_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"dup-device\",\"displayName\":\"Dup2\",\"productId\":\"1002\"}",
    )
    .expect("backend devices.create duplicate");
    assert!(duplicate_create.starts_with("HTTP/1.1 409"));
    assert!(duplicate_create.contains("api.device.duplicate_device_id"));
}

#[test]
fn backend_device_create_rejects_non_numeric_product_id() {
    let admin = standard_admin_api_server().expect("admin api server");

    let invalid_product = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"bad-product-id\",\"displayName\":\"Bad Product\",\"productId\":\"product-alpha\"}",
    )
    .expect("backend devices.create invalid product id");

    assert!(invalid_product.starts_with("HTTP/1.1 400"));
    assert!(invalid_product.contains("api.request.invalid_field"));
    assert!(invalid_product.contains("Field productId must be an int64 string"));
}

#[test]
fn backend_device_mutation_routes_enforce_standard_error_code_semantics() {
    let shared_repo = Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone())
        .with_command_repository(shared_repo.clone())
        .with_event_repository(shared_repo.clone())
        .with_twin_repository(shared_repo);

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"mutation-semantics-001\",\"displayName\":\"Mutation Semantics Device\",\"productId\":\"9401\"}",
    )
    .expect("create baseline device");
    assert!(create.starts_with("HTTP/1.1 201"));

    let duplicate_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"mutation-semantics-001\",\"displayName\":\"Mutation Semantics Device Duplicate\",\"productId\":\"9401\"}",
    )
    .expect("duplicate create semantics");
    assert!(duplicate_create.starts_with("HTTP/1.1 409"));
    assert!(duplicate_create.contains("api.device.duplicate_device_id"));

    let update_invalid_json = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/devices/mutation-semantics-001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":",
    )
    .expect("update invalid json semantics");
    assert!(update_invalid_json.starts_with("HTTP/1.1 400"));
    assert!(update_invalid_json.contains("api.request.invalid_json"));

    let update_wrong_permission = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/devices/mutation-semantics-001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n{\"displayName\":\"Mutation Semantics Updated\"}",
    )
    .expect("update wrong permission semantics");
    assert!(update_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(update_wrong_permission.contains("api.permission.denied"));
    assert!(update_wrong_permission.contains("iot.devices.write"));

    let update_missing_scope = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/devices/mutation-semantics-001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":\"Mutation Semantics Wrong Scope\"}",
    )
    .expect("update missing scope semantics");
    assert!(update_missing_scope.starts_with("HTTP/1.1 404"));
    assert!(update_missing_scope.contains("api.device.not_found"));

    let delete_wrong_permission = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/devices/mutation-semantics-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n",
    )
    .expect("delete wrong permission semantics");
    assert!(delete_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(delete_wrong_permission.contains("api.permission.denied"));
    assert!(delete_wrong_permission.contains("iot.devices.delete"));

    let delete_missing_scope = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/devices/mutation-semantics-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.devices.delete\r\n\r\n",
    )
    .expect("delete missing scope semantics");
    assert!(delete_missing_scope.starts_with("HTTP/1.1 404"));
    assert!(delete_missing_scope.contains("api.device.not_found"));

    let credential_invalid_json = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/mutation-semantics-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":",
    )
    .expect("credentials invalid json semantics");
    assert!(credential_invalid_json.starts_with("HTTP/1.1 400"));
    assert!(credential_invalid_json.contains("api.request.invalid_json"));

    let credential_wrong_permission = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/mutation-semantics-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("credentials wrong permission semantics");
    assert!(credential_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(credential_wrong_permission.contains("api.permission.denied"));
    assert!(credential_wrong_permission.contains("iot.devices.write"));

    let credential_missing_scope = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/mutation-semantics-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("credentials missing scope semantics");
    assert!(credential_missing_scope.starts_with("HTTP/1.1 404"));
    assert!(credential_missing_scope.contains("api.device.not_found"));
}

#[test]
fn problem_json_errors_expose_standard_fields_across_core_failure_paths() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_auth = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("missing auth problem");
    assert!(missing_auth.starts_with("HTTP/1.1 401"));
    let missing_auth_problem =
        assert_problem_json_fields(&missing_auth, 401, "api.auth.missing_dual_token");
    assert_eq!(
        missing_auth_problem
            .get("title")
            .and_then(serde_json::Value::as_str),
        Some("SDKWork dual token is required")
    );

    let invalid_context = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: tenant-a\r\nX-Sdkwork-Organization-Id: 0\r\n\r\n",
    )
    .expect("invalid context problem");
    assert!(invalid_context.starts_with("HTTP/1.1 400"));
    assert_problem_json_fields(&invalid_context, 400, "api.context.invalid_tenant_id");

    let permission_denied = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("permission denied problem");
    assert!(permission_denied.starts_with("HTTP/1.1 403"));
    let permission_denied_problem =
        assert_problem_json_fields(&permission_denied, 403, "api.permission.denied");
    assert_eq!(
        permission_denied_problem
            .get("requiredPermission")
            .and_then(serde_json::Value::as_str),
        Some("iot.protocolAdapters.read")
    );

    let device_not_found = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/problem-json-missing HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("device not found problem");
    assert!(device_not_found.starts_with("HTTP/1.1 404"));
    let device_not_found_problem =
        assert_problem_json_fields(&device_not_found, 404, "api.device.not_found");
    assert_eq!(
        device_not_found_problem
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("problem-json-missing")
    );

    let create_first = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"problem-json-dup-001\",\"displayName\":\"Problem Json Device\",\"productId\":\"9501\"}",
    )
    .expect("create first for duplicate");
    assert!(create_first.starts_with("HTTP/1.1 201"));

    let duplicate_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"problem-json-dup-001\",\"displayName\":\"Problem Json Device Duplicate\",\"productId\":\"9501\"}",
    )
    .expect("duplicate create problem");
    assert!(duplicate_create.starts_with("HTTP/1.1 409"));
    assert_problem_json_fields(&duplicate_create, 409, "api.device.duplicate_device_id");
}

#[test]
fn app_api_problem_json_errors_expose_standard_fields_across_core_failure_paths() {
    let app = standard_app_api_server().expect("app api server");

    let missing_auth = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("app missing auth problem");
    assert!(missing_auth.starts_with("HTTP/1.1 401"));
    let missing_auth_problem =
        assert_problem_json_fields(&missing_auth, 401, "api.auth.missing_dual_token");
    assert_eq!(
        missing_auth_problem
            .get("title")
            .and_then(serde_json::Value::as_str),
        Some("SDKWork dual token is required")
    );

    let missing_context = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token-missing-context\r\nAccess-Token: user-token-missing-context\r\n\r\n",
    )
    .expect("app missing context problem");
    assert!(missing_context.starts_with("HTTP/1.1 403"));
    assert_problem_json_fields(&missing_context, 403, "api.context.missing");

    let permission_denied = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n",
    )
    .expect("app permission denied problem");
    assert!(permission_denied.starts_with("HTTP/1.1 403"));
    let permission_denied_problem =
        assert_problem_json_fields(&permission_denied, 403, "api.permission.denied");
    assert_eq!(
        permission_denied_problem
            .get("requiredPermission")
            .and_then(serde_json::Value::as_str),
        Some("iot.devices.read")
    );

    let invalid_json = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-problem-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":",
    )
    .expect("app invalid json problem");
    assert!(invalid_json.starts_with("HTTP/1.1 400"));
    assert_problem_json_fields(&invalid_json, 400, "api.request.invalid_json");

    let missing_field = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/device-problem-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\"}",
    )
    .expect("app invalid field problem");
    assert!(missing_field.starts_with("HTTP/1.1 400"));
    assert_problem_json_fields(&missing_field, 400, "api.request.invalid_field");

    let not_found = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/app-problem-missing-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app not found problem");
    assert!(not_found.starts_with("HTTP/1.1 404"));
    let not_found_problem = assert_problem_json_fields(&not_found, 404, "api.device.not_found");
    assert_eq!(
        not_found_problem
            .get("deviceId")
            .and_then(serde_json::Value::as_str),
        Some("app-problem-missing-001")
    );
}

#[test]
fn backend_device_credentials_create_validates_request_and_requires_existing_device() {
    let admin = standard_admin_api_server().expect("admin api server");

    let missing_device = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/device-credentials-missing/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("backend devices.credentials.create missing device");
    assert!(missing_device.starts_with("HTTP/1.1 404"));
    assert!(missing_device.contains("api.device.not_found"));

    let create_device = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"device-cred-001\",\"displayName\":\"Credential Device\",\"productId\":\"1005\"}",
    )
    .expect("backend devices.create credential target");
    assert!(create_device.starts_with("HTTP/1.1 201"));

    let missing_body = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/device-cred-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n",
    )
    .expect("backend devices.credentials.create missing body");
    assert!(missing_body.starts_with("HTTP/1.1 400"));
    assert!(missing_body.contains("api.request.body.required"));

    let invalid_type = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/device-cred-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"api_key\"}",
    )
    .expect("backend devices.credentials.create invalid type");
    assert!(invalid_type.starts_with("HTTP/1.1 400"));
    assert!(invalid_type.contains("api.request.invalid_field"));
    assert!(invalid_type.contains("Field credentialType must be one of"));

    let created = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/device-cred-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\",\"expiresAt\":\"2026-06-30T00:00:00Z\"}",
    )
    .expect("backend devices.credentials.create success");
    assert!(created.starts_with("HTTP/1.1 201"));
    assert!(created.contains(r#""deviceId":"device-cred-001""#));
    assert!(created.contains(r#""credentialType":"hmac""#));
    assert!(created.contains(r#""status":"active""#));
    assert!(created.contains(r#""expiresAt":"2026-06-30T00:00:00Z""#));
}

#[test]
fn admin_and_app_servers_can_share_external_device_repository_state() {
    let shared_repo: Arc<dyn sdkwork_aiot_storage::AiotDeviceRepository> =
        Arc::new(sdkwork_aiot_storage_sqlx::InMemorySqlxDeviceRepository::new());

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(shared_repo.clone());
    let app = standard_app_api_server()
        .expect("app api server")
        .with_device_repository(shared_repo);

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"shared-001\",\"displayName\":\"Shared Device\",\"productId\":\"1003\"}",
    )
    .expect("backend devices.create");
    assert!(create.starts_with("HTTP/1.1 201"));

    let app_retrieve = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/shared-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app devices.retrieve shared");
    assert!(app_retrieve.starts_with("HTTP/1.1 200"));
    assert!(app_retrieve.contains(r#""deviceId":"shared-001""#));
    assert!(app_retrieve.contains(r#""displayName":"Shared Device""#));
}

#[test]
fn backend_device_create_maps_storage_failure_to_500_problem() {
    struct FailingDeviceRepository;

    impl sdkwork_aiot_storage::AiotDeviceRepository for FailingDeviceRepository {
        fn create_device(
            &self,
            _command: sdkwork_aiot_storage::AiotDeviceCreateCommand,
        ) -> Result<
            sdkwork_aiot_storage::AiotDeviceRecord,
            sdkwork_aiot_storage::AiotDeviceRepositoryError,
        > {
            Err(sdkwork_aiot_storage::AiotDeviceRepositoryError::PersistenceFailure)
        }

        fn get_device(
            &self,
            _association: &sdkwork_aiot_storage::AiotStorageAssociation,
            _device_id: &str,
        ) -> Option<sdkwork_aiot_storage::AiotDeviceRecord> {
            None
        }

        fn list_devices(
            &self,
            _association: &sdkwork_aiot_storage::AiotStorageAssociation,
            _params: sdkwork_aiot_storage::OffsetListPageParams,
        ) -> Result<
            sdkwork_aiot_storage::AiotOffsetListResult<sdkwork_aiot_storage::AiotDeviceRecord>,
            sdkwork_aiot_storage::AiotDeviceRepositoryError,
        > {
            Ok(sdkwork_aiot_storage::AiotOffsetListResult::empty())
        }

        fn list_device_ids_for_rollout(
            &self,
            _association: &sdkwork_aiot_storage::AiotStorageAssociation,
            _product_id: Option<&str>,
            _limit: Option<i64>,
        ) -> Result<Vec<String>, sdkwork_aiot_storage::AiotDeviceRepositoryError> {
            Ok(Vec::new())
        }

        fn update_device(
            &self,
            _command: sdkwork_aiot_storage::AiotDeviceUpdateCommand,
        ) -> Result<
            sdkwork_aiot_storage::AiotDeviceRecord,
            sdkwork_aiot_storage::AiotDeviceRepositoryError,
        > {
            Err(sdkwork_aiot_storage::AiotDeviceRepositoryError::PersistenceFailure)
        }

        fn delete_device(
            &self,
            _association: &sdkwork_aiot_storage::AiotStorageAssociation,
            _device_id: &str,
        ) -> Result<(), sdkwork_aiot_storage::AiotDeviceRepositoryError> {
            Err(sdkwork_aiot_storage::AiotDeviceRepositoryError::PersistenceFailure)
        }
    }

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_device_repository(Arc::new(FailingDeviceRepository));

    let response = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"storage-fail-001\",\"displayName\":\"Storage Fail\",\"productId\":\"1009\"}",
    )
    .expect("backend devices.create storage failure");

    assert!(response.starts_with("HTTP/1.1 500"));
    assert!(response.contains("application/problem+json"));
    assert!(response.contains("api.storage.write_failed"));
}

#[test]
fn backend_firmware_artifact_create_response_uses_media_resource_shape() {
    let admin = standard_admin_api_server().expect("admin api server");

    let response = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-main-shape\",\"version\":\"1.0.0\",\"resource\":{\"id\":\"media-res-001\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-001\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1048576\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create");

    assert!(response.starts_with("HTTP/1.1 201"));
    assert!(response.contains(r#""artifactId":"firmware-artifact-0001""#));
    assert!(response.contains(r#""mediaResourceId":"media-res-001""#));
    let body = response_body_json(&response);
    assert_eq!(
        resource_data_pointer(&body, "/resource/id").and_then(serde_json::Value::as_str),
        Some("media-res-001")
    );
    assert_eq!(
        resource_data_pointer(&body, "/resource/kind").and_then(serde_json::Value::as_str),
        Some("document")
    );
    assert_eq!(
        resource_data_pointer(&body, "/resource/source").and_then(serde_json::Value::as_str),
        Some("object_storage")
    );
    assert!(!response.contains(r#""storageUri":"#));
}

#[test]
fn backend_firmware_artifact_crud_routes_are_complete_and_scoped() {
    let admin = standard_admin_api_server().expect("admin api server");

    let create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-main\",\"version\":\"1.0.0\",\"resource\":{\"id\":\"media-res-201\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-201\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1048576\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create");
    assert!(create.starts_with("HTTP/1.1 201"));
    assert!(create.contains(r#""artifactId":"firmware-artifact-0001""#));
    assert!(create.contains(r#""artifactKey":"fw-main""#));

    let list = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.list");
    assert!(list.starts_with("HTTP/1.1 200"));
    assert!(list.contains(r#""artifactId":"firmware-artifact-0001""#));

    let retrieve = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.retrieve");
    assert!(retrieve.starts_with("HTTP/1.1 200"));
    assert!(retrieve.contains(r#""artifactId":"firmware-artifact-0001""#));

    let update = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"version\":\"1.0.1\",\"status\":\"deprecated\"}",
    )
    .expect("backend firmwareArtifacts.update");
    assert!(update.starts_with("HTTP/1.1 200"));
    assert!(update.contains(r#""version":"1.0.1""#));
    assert!(update.contains(r#""status":"deprecated""#));

    let cross_scope = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.retrieve cross scope");
    assert!(cross_scope.starts_with("HTTP/1.1 404"));
    assert!(cross_scope.contains("api.firmware.artifact.not_found"));

    let delete = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.delete");
    assert!(delete.starts_with("HTTP/1.1 204"));

    let retrieve_deleted = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.retrieve deleted");
    assert!(retrieve_deleted.starts_with("HTTP/1.1 404"));
}

#[test]
fn backend_firmware_rollout_crud_routes_are_complete_and_scoped() {
    let admin = standard_admin_api_server().expect("admin api server");

    let create_artifact = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-rollout\",\"version\":\"2.0.0\",\"resource\":{\"id\":\"media-res-301\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-301\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"2048\"},\"sha256\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"}",
    )
    .expect("backend firmwareArtifacts.create rollout anchor");
    assert!(create_artifact.starts_with("HTTP/1.1 201"));

    let create_rollout = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n{\"artifactId\":\"firmware-artifact-0001\",\"targetPolicy\":{\"scope\":\"all\",\"batch\":10}}",
    )
    .expect("backend firmwareRollouts.create");
    assert!(create_rollout.starts_with("HTTP/1.1 202"));
    assert!(create_rollout.contains(r#""rolloutId":"firmware-rollout-0001""#));
    assert!(create_rollout.contains(r#""status":"accepted""#));

    let list = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareRollouts.list");
    assert!(list.starts_with("HTTP/1.1 200"));
    assert!(list.contains(r#""rolloutId":"firmware-rollout-0001""#));

    let retrieve = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareRollouts.retrieve");
    assert!(retrieve.starts_with("HTTP/1.1 200"));
    assert!(retrieve.contains(r#""rolloutId":"firmware-rollout-0001""#));

    let update = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n{\"status\":\"paused\",\"targetPolicy\":{\"scope\":\"all\",\"batch\":5}}",
    )
    .expect("backend firmwareRollouts.update");
    assert!(update.starts_with("HTTP/1.1 200"));
    assert!(update.contains(r#""status":"paused""#));

    let cross_scope = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 29999\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareRollouts.retrieve cross scope");
    assert!(cross_scope.starts_with("HTTP/1.1 404"));
    assert!(cross_scope.contains("api.firmware.rollout.not_found"));

    let delete = handle_api_request_bytes(
        &admin,
        b"DELETE /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n",
    )
    .expect("backend firmwareRollouts.delete");
    assert!(delete.starts_with("HTTP/1.1 204"));

    let retrieve_deleted = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareRollouts.retrieve deleted");
    assert!(retrieve_deleted.starts_with("HTTP/1.1 404"));
}

#[test]
fn backend_firmware_mutation_error_semantics_are_stable() {
    let admin = standard_admin_api_server().expect("admin api server");

    let create_artifact = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-err\",\"version\":\"1.0.0\",\"resource\":{\"id\":\"media-res-err-001\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-err-001\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1024\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create error anchor");
    assert!(create_artifact.starts_with("HTTP/1.1 201"));

    let artifact_wrong_permission = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n{\"artifactKey\":\"fw-err-denied\",\"version\":\"1.0.1\",\"resource\":{\"id\":\"media-res-err-002\",\"kind\":\"document\",\"source\":\"object_storage\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1024\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create wrong permission");
    assert!(artifact_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(artifact_wrong_permission.contains("application/problem+json"));
    assert!(artifact_wrong_permission.contains("api.permission.denied"));

    let artifact_invalid_json = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":",
    )
    .expect("backend firmwareArtifacts.create invalid json");
    assert!(artifact_invalid_json.starts_with("HTTP/1.1 400"));
    assert!(artifact_invalid_json.contains("application/problem+json"));
    assert!(artifact_invalid_json.contains("api.request.invalid_json"));

    let artifact_invalid_field = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-err-invalid\",\"version\":\"1.0.2\",\"resource\":\"not-an-object\",\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create invalid field");
    assert!(artifact_invalid_field.starts_with("HTTP/1.1 400"));
    assert!(artifact_invalid_field.contains("application/problem+json"));
    assert!(artifact_invalid_field.contains("api.request.invalid_field"));

    let rollout_invalid_reference = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n{\"artifactId\":\"firmware-artifact-does-not-exist\",\"targetPolicy\":{\"scope\":\"all\"}}",
    )
    .expect("backend firmwareRollouts.create invalid reference");
    assert!(rollout_invalid_reference.starts_with("HTTP/1.1 400"));
    assert!(rollout_invalid_reference.contains("application/problem+json"));
    assert!(rollout_invalid_reference.contains("api.firmware.artifact.invalid_reference"));

    let rollout_wrong_permission = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n{\"artifactId\":\"firmware-artifact-0001\",\"targetPolicy\":{\"scope\":\"all\"}}",
    )
    .expect("backend firmwareRollouts.create wrong permission");
    assert!(rollout_wrong_permission.starts_with("HTTP/1.1 403"));
    assert!(rollout_wrong_permission.contains("application/problem+json"));
    assert!(rollout_wrong_permission.contains("api.permission.denied"));

    let rollout_invalid_json = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n{\"artifactId\":",
    )
    .expect("backend firmwareRollouts.create invalid json");
    assert!(rollout_invalid_json.starts_with("HTTP/1.1 400"));
    assert!(rollout_invalid_json.contains("application/problem+json"));
    assert!(rollout_invalid_json.contains("api.request.invalid_json"));
}

#[test]
fn backend_firmware_update_accepts_empty_body_as_noop() {
    let admin = standard_admin_api_server().expect("admin api server");

    let create_artifact = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-empty-update\",\"version\":\"1.0.0\",\"resource\":{\"id\":\"media-res-empty-update\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-empty-update\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1024\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create for empty update");
    assert!(create_artifact.starts_with("HTTP/1.1 201"));

    let artifact_empty_update = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/firmware_artifacts/firmware-artifact-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.update empty body");
    assert!(artifact_empty_update.starts_with("HTTP/1.1 200"));
    assert!(artifact_empty_update.contains(r#""artifactId":"firmware-artifact-0001""#));

    let create_rollout = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_rollouts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n{\"artifactId\":\"firmware-artifact-0001\",\"targetPolicy\":{\"scope\":\"all\",\"batch\":8}}",
    )
    .expect("backend firmwareRollouts.create for empty update");
    assert!(create_rollout.starts_with("HTTP/1.1 202"));

    let rollout_empty_update = handle_api_request_bytes(
        &admin,
        b"PUT /backend/v3/api/iot/firmware_rollouts/firmware-rollout-0001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.rollout\r\n\r\n",
    )
    .expect("backend firmwareRollouts.update empty body");
    assert!(rollout_empty_update.starts_with("HTTP/1.1 200"));
    assert!(rollout_empty_update.contains(r#""rolloutId":"firmware-rollout-0001""#));
}

#[test]
fn api_server_rejects_cross_surface_routes_with_problem_json() {
    let app = standard_app_api_server().expect("app api server");

    let response = handle_api_request_bytes(
        &app,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.protocolAdapters.read\r\n\r\n",
    )
    .expect("problem json");

    assert!(response.starts_with("HTTP/1.1 404"));
    assert!(response.contains("application/problem+json"));
    assert!(response.contains("api.route.unsupported"));
}

#[test]
fn backend_openapi_operation_ids_align_with_route_contracts() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read backend openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse backend openapi json");

    let mut openapi_operation_ids = std::collections::BTreeSet::new();
    if let Some(paths) = openapi_json
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for item in paths.values() {
            if let Some(path_item) = item.as_object() {
                for method in path_item.values() {
                    if let Some(operation_id) = method
                        .get("operationId")
                        .and_then(serde_json::Value::as_str)
                    {
                        openapi_operation_ids.insert(operation_id.to_string());
                    }
                }
            }
        }
    }

    let contract_operation_ids = standard_api_route_contracts()
        .into_iter()
        .filter(|route| route.surface == AiotApiSurface::Admin)
        .map(|route| route.operation_id.to_string())
        .collect::<std::collections::BTreeSet<_>>();

    for operation_id in openapi_operation_ids {
        assert!(
            contract_operation_ids.contains(&operation_id),
            "missing backend route contract for OpenAPI operationId: {operation_id}"
        );
    }
}

#[test]
fn app_openapi_operation_ids_align_with_route_contracts() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/app-api/iot/sdkwork-aiot-app-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read app openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse app openapi json");

    let mut openapi_operation_ids = std::collections::BTreeSet::new();
    if let Some(paths) = openapi_json
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for item in paths.values() {
            if let Some(path_item) = item.as_object() {
                for method in path_item.values() {
                    if let Some(operation_id) = method
                        .get("operationId")
                        .and_then(serde_json::Value::as_str)
                    {
                        openapi_operation_ids.insert(operation_id.to_string());
                    }
                }
            }
        }
    }

    let contract_operation_ids = standard_api_route_contracts()
        .into_iter()
        .filter(|route| route.surface == AiotApiSurface::App)
        .map(|route| route.operation_id.to_string())
        .collect::<std::collections::BTreeSet<_>>();

    for operation_id in openapi_operation_ids {
        assert!(
            contract_operation_ids.contains(&operation_id),
            "missing app route contract for OpenAPI operationId: {operation_id}"
        );
    }
}

#[test]
fn backend_openapi_permissions_align_with_route_contracts() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read backend openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse backend openapi json");

    let mut openapi_permissions = std::collections::BTreeMap::new();
    if let Some(paths) = openapi_json
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for item in paths.values() {
            if let Some(path_item) = item.as_object() {
                for method in path_item.values() {
                    let Some(operation_id) = method
                        .get("operationId")
                        .and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let required_permission = method
                        .get("x-sdkwork-required-permission")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    openapi_permissions
                        .insert(operation_id.to_string(), required_permission.to_string());
                }
            }
        }
    }

    let contract_permissions = standard_api_route_contracts()
        .into_iter()
        .filter(|route| route.surface == AiotApiSurface::Admin)
        .map(|route| {
            (
                route.operation_id.to_string(),
                route.required_permission.to_string(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for (operation_id, required_permission) in openapi_permissions {
        let Some(contract_permission) = contract_permissions.get(&operation_id) else {
            continue;
        };
        assert_eq!(
            required_permission, *contract_permission,
            "backend permission drift for operationId {operation_id}"
        );
    }
}

#[test]
fn app_openapi_permissions_align_with_route_contracts() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/app-api/iot/sdkwork-aiot-app-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read app openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse app openapi json");

    let mut openapi_permissions = std::collections::BTreeMap::new();
    if let Some(paths) = openapi_json
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for item in paths.values() {
            if let Some(path_item) = item.as_object() {
                for method in path_item.values() {
                    let Some(operation_id) = method
                        .get("operationId")
                        .and_then(serde_json::Value::as_str)
                    else {
                        continue;
                    };
                    let required_permission = method
                        .get("x-sdkwork-required-permission")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    openapi_permissions
                        .insert(operation_id.to_string(), required_permission.to_string());
                }
            }
        }
    }

    let contract_permissions = standard_api_route_contracts()
        .into_iter()
        .filter(|route| route.surface == AiotApiSurface::App)
        .map(|route| {
            (
                route.operation_id.to_string(),
                route.required_permission.to_string(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for (operation_id, required_permission) in openapi_permissions {
        let Some(contract_permission) = contract_permissions.get(&operation_id) else {
            continue;
        };
        assert_eq!(
            required_permission, *contract_permission,
            "app permission drift for operationId {operation_id}"
        );
    }
}

#[test]
fn backend_openapi_problem_declaration_covers_observed_runtime_error_statuses() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read backend openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse backend openapi json");
    assert_problem_component_declares_problem_json_media_type(&openapi_json);
    let operation_responses = openapi_operation_responses(&openapi_json);

    let admin = standard_admin_api_server().expect("admin api server");

    let missing_auth = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("backend missing auth");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "protocolAdapters.list",
        &missing_auth,
        &operation_responses,
    );

    let invalid_context = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: tenant-a\r\nX-Sdkwork-Organization-Id: 0\r\n\r\n",
    )
    .expect("backend invalid context");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "protocolAdapters.list",
        &invalid_context,
        &operation_responses,
    );

    let permission_denied = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend permission denied");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "protocolAdapters.list",
        &permission_denied,
        &operation_responses,
    );

    let first_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"openapi-problem-dup-001\",\"displayName\":\"OpenAPI Problem Device\",\"productId\":\"9601\"}",
    )
    .expect("backend create for duplicate");
    assert!(first_create.starts_with("HTTP/1.1 201"));

    let duplicate_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"openapi-problem-dup-001\",\"displayName\":\"OpenAPI Problem Device Duplicate\",\"productId\":\"9601\"}",
    )
    .expect("backend duplicate create");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.create",
        &duplicate_create,
        &operation_responses,
    );

    let device_not_found = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/openapi-problem-missing-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend device not found");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.retrieve",
        &device_not_found,
        &operation_responses,
    );
}

#[test]
fn app_openapi_problem_declaration_covers_observed_runtime_error_statuses() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/app-api/iot/sdkwork-aiot-app-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read app openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse app openapi json");
    assert_problem_component_declares_problem_json_media_type(&openapi_json);
    let operation_responses = openapi_operation_responses(&openapi_json);

    let app = standard_app_api_server().expect("app api server");

    let missing_auth = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("app missing auth");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.list",
        &missing_auth,
        &operation_responses,
    );

    let permission_denied = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n",
    )
    .expect("app permission denied");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.list",
        &permission_denied,
        &operation_responses,
    );

    let invalid_json = handle_api_request_bytes(
        &app,
        b"POST /app/v3/api/iot/devices/openapi-problem-device-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":",
    )
    .expect("app invalid json");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.commands.create",
        &invalid_json,
        &operation_responses,
    );

    let device_not_found = handle_api_request_bytes(
        &app,
        b"GET /app/v3/api/iot/devices/openapi-problem-missing-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("app device not found");
    assert_runtime_error_status_is_declared_via_openapi_problem_default(
        "devices.retrieve",
        &device_not_found,
        &operation_responses,
    );
}

#[test]
fn app_typescript_sdk_problem_details_contract_aligns_with_openapi_schema() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/app-api/iot/sdkwork-aiot-app-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read app openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse app openapi json");
    let schema = openapi_problem_details_schema(&openapi_json);
    let required = schema_required_fields(schema);
    assert!(required.contains("type"));
    assert!(required.contains("title"));
    assert!(required.contains("status"));

    let ts_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript/generated/server-openapi/src/types/problem-detail.ts",
    );
    let ts = std::fs::read_to_string(ts_path).expect("read app generated problem-detail");

    assert!(
        ts.contains("export interface ProblemDetail"),
        "app generated TS SDK must export ProblemDetail interface"
    );
    assert!(ts.contains("type: string;"));
    assert!(ts.contains("title: string;"));
    assert!(ts.contains("status: number;"));
    assert!(ts.contains("detail?: string;"));
    assert!(ts.contains("traceId: string;"));
    assert!(ts.contains("code: SdkWorkPlatformErrorCode;"));
}

#[test]
fn backend_typescript_sdk_problem_details_contract_aligns_with_openapi_schema() {
    let openapi_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json");
    let openapi_text = std::fs::read_to_string(openapi_path).expect("read backend openapi");
    let openapi_json: serde_json::Value =
        serde_json::from_str(&openapi_text).expect("parse backend openapi json");
    let schema = openapi_problem_details_schema(&openapi_json);
    let required = schema_required_fields(schema);
    assert!(required.contains("type"));
    assert!(required.contains("title"));
    assert!(required.contains("status"));

    let ts_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../sdks/sdkwork-aiot-backend-sdk/sdkwork-aiot-backend-sdk-typescript/generated/server-openapi/src/types/problem-detail.ts",
    );
    let ts = std::fs::read_to_string(ts_path).expect("read backend generated problem-detail");

    assert!(
        ts.contains("export interface ProblemDetail"),
        "backend generated TS SDK must export ProblemDetail interface"
    );
    assert!(ts.contains("type: string;"));
    assert!(ts.contains("title: string;"));
    assert!(ts.contains("status: number;"));
    assert!(ts.contains("detail?: string;"));
    assert!(ts.contains("traceId: string;"));
    assert!(ts.contains("code: SdkWorkPlatformErrorCode;"));
}

#[test]
fn backend_typescript_sdk_exposes_firmware_crud_surface() {
    let generated_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../sdks/sdkwork-aiot-backend-sdk/sdkwork-aiot-backend-sdk-typescript/generated/server-openapi/src",
    );
    let artifact_update = std::fs::read_to_string(
        generated_root.join("types/aiot-firmware-artifact-update-request.ts"),
    )
    .expect("read backend generated firmware artifact update request");
    let rollout_update = std::fs::read_to_string(
        generated_root.join("types/aiot-firmware-rollout-update-request.ts"),
    )
    .expect("read backend generated firmware rollout update request");
    let rollout_response =
        std::fs::read_to_string(generated_root.join("types/aiot-firmware-rollout-response.ts"))
            .expect("read backend generated firmware rollout response");
    let artifact = std::fs::read_to_string(generated_root.join("types/aiot-firmware-artifact.ts"))
        .expect("read backend generated firmware artifact");
    let iot_api = std::fs::read_to_string(generated_root.join("api/iot.ts"))
        .expect("read backend generated iot api");

    assert!(artifact_update.contains("export interface AiotFirmwareArtifactUpdateRequest"));
    assert!(rollout_update.contains("export interface AiotFirmwareRolloutUpdateRequest"));
    assert!(rollout_response.contains("export interface AiotFirmwareRolloutResponse"));
    assert!(artifact.contains("export interface AiotFirmwareArtifact"));

    assert!(iot_api.contains("class IotFirmwareArtifactsApi"));
    assert!(iot_api.contains("Promise<SdkWorkPageData>"));
    assert!(iot_api.contains("async retrieve(artifactId: string"));
    assert!(iot_api.contains("body?: AiotFirmwareArtifactUpdateRequest"));
    assert!(iot_api.contains("async delete(artifactId: string"));

    assert!(iot_api.contains("class IotFirmwareRolloutsApi"));
    assert!(iot_api.contains("async create(body: AiotFirmwareRolloutCreateRequest"));
    assert!(iot_api.contains("async retrieve(rolloutId: string"));
    assert!(iot_api.contains("body?: AiotFirmwareRolloutUpdateRequest"));

    assert!(iot_api.contains("class IotDevicesSessionsApi"));
    assert!(iot_api.contains("async list(deviceId: string"));
    assert!(iot_api.contains("async disconnect(deviceId: string, sessionId: string"));
    assert!(iot_api.contains("class IotDevicesCapabilitiesApi"));
    assert!(iot_api.contains("async cancel(deviceId: string, commandId: string"));
    assert!(iot_api.contains("class IotDevicesCredentialsApi"));
    assert!(iot_api.contains("async retrieve(deviceId: string, credentialId: string"));
    assert!(iot_api.contains("class IotDevicesTwinApi"));
    assert!(iot_api.contains("async update(deviceId: string, body: AiotTwinUpdateRequest"));
}

#[test]
fn typescript_problem_code_catalogs_cover_observed_runtime_problem_codes() {
    let admin = standard_admin_api_server().expect("admin api server");
    let app = standard_app_api_server().expect("app api server");

    let mut observed_backend = std::collections::BTreeSet::new();
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\n\r\n",
        )
        .expect("backend missing auth response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token-missing-context\r\nAccess-Token: user-token-missing-context\r\n\r\n",
        )
        .expect("backend missing context response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: tenant-a\r\nX-Sdkwork-Organization-Id: 0\r\n\r\n",
        )
        .expect("backend invalid tenant response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"GET /backend/v3/api/iot/protocol_adapters HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
        )
        .expect("backend permission denied response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":",
        )
        .expect("backend invalid json response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"displayName\":\"Missing DeviceId\",\"productId\":\"9001\"}",
        )
        .expect("backend missing field response"),
    ));
    let backend_create = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"runtime-problem-catalog-001\",\"displayName\":\"Catalog Device\",\"productId\":\"9001\"}",
    )
    .expect("backend create response");
    assert!(backend_create.starts_with("HTTP/1.1 201"));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"runtime-problem-catalog-001\",\"displayName\":\"Catalog Device Duplicate\",\"productId\":\"9001\"}",
        )
        .expect("backend duplicate response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"GET /backend/v3/api/iot/devices/runtime-problem-catalog-missing HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
        )
        .expect("backend not found response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"DELETE /backend/v3/api/iot/devices/runtime-problem-catalog-001/sessions/session-runtime-problem-catalog-001-unknown HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.sessions.disconnect\r\n\r\n",
        )
        .expect("backend session not found response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"POST /backend/v3/api/iot/devices/runtime-problem-catalog-001/commands/runtime-problem-catalog-command-001/cancel HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.cancel\r\n\r\n",
        )
        .expect("backend command not found response"),
    ));
    observed_backend.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &admin,
            b"DELETE /backend/v3/api/iot/devices/runtime-problem-catalog-missing/sessions/session-runtime-problem-catalog-missing-primary HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.sessions.disconnect\r\n\r\n",
        )
        .expect("backend session device missing response"),
    ));

    let mut observed_app = std::collections::BTreeSet::new();
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\n\r\n",
        )
        .expect("app missing auth response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token-missing-context\r\nAccess-Token: user-token-missing-context\r\n\r\n",
        )
        .expect("app missing context response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: tenant-a\r\nX-Sdkwork-Organization-Id: 0\r\n\r\n",
        )
        .expect("app invalid tenant response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"GET /app/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n",
        )
        .expect("app permission denied response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"POST /app/v3/api/iot/devices/runtime-problem-catalog-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":",
        )
        .expect("app invalid json response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"POST /app/v3/api/iot/devices/runtime-problem-catalog-001/commands HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.commands.execute\r\n\r\n{\"capabilityName\":\"player\",\"commandName\":\"speak\"}",
        )
        .expect("app missing field response"),
    ));
    observed_app.insert(problem_code_from_response(
        &handle_api_request_bytes(
            &app,
            b"GET /app/v3/api/iot/devices/runtime-problem-catalog-missing HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
        )
        .expect("app not found response"),
    ));

    let app_ts_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../sdks/sdkwork-aiot-app-sdk/sdkwork-aiot-app-sdk-typescript/generated/server-openapi/src/types/problem-details.ts",
    );
    let app_ts = std::fs::read_to_string(app_ts_path).expect("read app generated problem-details");
    assert!(
        app_ts.contains("export interface ProblemDetails"),
        "app generated TS SDK must export ProblemDetails interface"
    );
    assert!(
        app_ts.contains("code?: string;"),
        "app generated TS SDK must model ProblemDetails.code for runtime problem codes"
    );
    let backend_ts_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../sdks/sdkwork-aiot-backend-sdk/sdkwork-aiot-backend-sdk-typescript/generated/server-openapi/src/types/problem-details.ts",
    );
    let backend_ts =
        std::fs::read_to_string(backend_ts_path).expect("read backend generated problem-details");
    assert!(
        backend_ts.contains("export interface ProblemDetails"),
        "backend generated TS SDK must export ProblemDetails interface"
    );
    assert!(
        backend_ts.contains("code?: string;"),
        "backend generated TS SDK must model ProblemDetails.code for runtime problem codes"
    );
    for code in observed_app.into_iter().chain(observed_backend.into_iter()) {
        assert!(
            code.starts_with("api."),
            "runtime problem code {code} must use the api.* namespace"
        );
    }
}

fn openapi_problem_details_schema(openapi_json: &serde_json::Value) -> &serde_json::Value {
    openapi_json
        .get("components")
        .and_then(|value| value.get("schemas"))
        .and_then(|value| value.get("ProblemDetails"))
        .unwrap_or_else(|| panic!("OpenAPI must define components.schemas.ProblemDetails"))
}

fn schema_required_fields(schema: &serde_json::Value) -> std::collections::BTreeSet<String> {
    schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flat_map(|array| array.iter())
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn problem_code_from_response(response: &str) -> String {
    assert!(
        response.contains("application/problem+json"),
        "expected problem response, got {response}"
    );
    let body = response_body_json(response);
    body.get("code")
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_i64().map(|code| code.to_string()))
        })
        .unwrap_or_else(|| panic!("problem response missing code field: {response}"))
}

fn assert_problem_component_declares_problem_json_media_type(openapi_json: &serde_json::Value) {
    let media_type = openapi_json
        .get("components")
        .and_then(|value| value.get("responses"))
        .and_then(|value| value.get("Problem"))
        .and_then(|value| value.get("content"))
        .and_then(|value| value.get("application/problem+json"));
    assert!(
        media_type.is_some(),
        "OpenAPI components.responses.Problem must declare application/problem+json content"
    );
}

fn openapi_operation_responses(
    openapi_json: &serde_json::Value,
) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, serde_json::Value>> {
    let mut responses_by_operation = std::collections::BTreeMap::new();
    if let Some(paths) = openapi_json
        .get("paths")
        .and_then(serde_json::Value::as_object)
    {
        for path_item in paths.values() {
            let Some(path_object) = path_item.as_object() else {
                continue;
            };
            for operation in path_object.values() {
                let Some(operation_object) = operation.as_object() else {
                    continue;
                };
                let Some(operation_id) = operation_object
                    .get("operationId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let Some(responses) = operation_object
                    .get("responses")
                    .and_then(serde_json::Value::as_object)
                else {
                    continue;
                };
                let mut response_map = std::collections::BTreeMap::new();
                for (status, response_decl) in responses {
                    response_map.insert(status.clone(), response_decl.clone());
                }
                responses_by_operation.insert(operation_id.to_string(), response_map);
            }
        }
    }
    responses_by_operation
}

fn assert_runtime_error_status_is_declared_via_openapi_problem_default(
    operation_id: &str,
    response: &str,
    responses_by_operation: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, serde_json::Value>,
    >,
) {
    let status = response_status_code(response);
    assert!(
        (400..600).contains(&status),
        "expected runtime error status for operation {operation_id}, got {status} in response: {response}"
    );
    let declared = responses_by_operation
        .get(operation_id)
        .unwrap_or_else(|| panic!("operation {operation_id} is missing responses in OpenAPI"));
    let status_key = status.to_string();
    if declared.contains_key(&status_key) {
        return;
    }
    let default = declared.get("default").unwrap_or_else(|| {
        panic!("operation {operation_id} must declare explicit {status_key} or default response")
    });
    assert_eq!(
        default
            .get("$ref")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default(),
        "#/components/responses/Problem",
        "operation {operation_id} default response must reference components.responses.Problem"
    );
}

fn response_status_code(response: &str) -> i64 {
    let first_line = response.lines().next().expect("http status line");
    first_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<i64>().ok())
        .expect("http status code")
}

fn response_body_json(response: &str) -> serde_json::Value {
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .expect("http response body");
    serde_json::from_str(body).expect("response body json")
}

fn assert_success_envelope(value: &serde_json::Value) {
    let code = value.get("code").expect("success envelope code");
    assert!(
        code.as_i64() == Some(0) || code.as_str() == Some("0"),
        "expected success code 0, got {code}"
    );
    assert!(
        value
            .get("traceId")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "expected traceId in success envelope, got {value}"
    );
}

fn list_data_items(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    value
        .pointer("/data/items")
        .and_then(serde_json::Value::as_array)
        .or_else(|| value.pointer("/data").and_then(serde_json::Value::as_array))
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn resource_data_pointer<'a>(
    value: &'a serde_json::Value,
    suffix: &str,
) -> Option<&'a serde_json::Value> {
    let item_path = format!("/data/item{suffix}");
    value
        .pointer(&item_path)
        .or_else(|| value.pointer(&format!("/data{suffix}")))
}

fn resource_data_str<'a>(value: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    resource_data_pointer(value, &format!("/{field}")).and_then(serde_json::Value::as_str)
}

fn command_acceptance_resource_id(value: &serde_json::Value) -> Option<&str> {
    value
        .pointer("/data/resourceId")
        .and_then(serde_json::Value::as_str)
        .or_else(|| resource_data_str(value, "commandId"))
}

fn command_acceptance_status(value: &serde_json::Value) -> Option<&str> {
    value
        .pointer("/data/status")
        .and_then(serde_json::Value::as_str)
        .or_else(|| resource_data_str(value, "status"))
}

fn assert_command_acceptance(value: &serde_json::Value) {
    assert_success_envelope(value);
    assert_eq!(
        value
            .pointer("/data/accepted")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(command_acceptance_resource_id(value).is_some());
}

fn assert_json_string_at(value: &serde_json::Value, pointer: &str) {
    if pointer.starts_with("/data/") && !pointer.starts_with("/data/item/") {
        let item_pointer = pointer.replacen("/data/", "/data/item/", 1);
        if value
            .pointer(&item_pointer)
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            return;
        }
    }
    assert!(
        value
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "expected string at pointer {pointer}, got {value}"
    );
}

fn assert_json_string_value(value: &serde_json::Value, key: &str) {
    assert!(
        value.get(key).and_then(serde_json::Value::as_str).is_some(),
        "expected string field {key}, got {value}"
    );
}

fn assert_problem_json_fields(
    response: &str,
    expected_status: i64,
    expected_code: &str,
) -> serde_json::Value {
    assert!(
        response.contains("application/problem+json"),
        "expected problem+json response, got {response}"
    );
    let body = response_body_json(response);
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("about:blank")
    );
    assert!(
        body.get("title")
            .and_then(serde_json::Value::as_str)
            .is_some(),
        "expected title in problem response, got {body}"
    );
    assert_eq!(
        body.get("status").and_then(serde_json::Value::as_i64),
        Some(expected_status)
    );
    assert_eq!(
        body.get("code").and_then(serde_json::Value::as_str),
        Some(expected_code)
    );
    body
}

#[test]
fn sqlite_credential_repository_adapter_persists_and_lists_credentials() {
    let temp_path = std::env::temp_dir().join(format!(
        "sdkwork-iot-platform-service-cred-{}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let adapter = sdkwork_iot_platform_service::SqliteCredentialRepositoryAdapter::open(&temp_path)
        .expect("open sqlite credential adapter");
    let repository =
        Arc::new(adapter) as Arc<dyn sdkwork_iot_platform_service::AiotCredentialRepository>;

    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_credential_repository(repository);

    let create_device = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"deviceId\":\"sqlite-cred-001\",\"displayName\":\"SQLite Credential Device\",\"productId\":\"1005\"}",
    )
    .expect("backend devices.create sqlite credential target");
    assert!(create_device.starts_with("HTTP/1.1 201"));

    let created = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/devices/sqlite-cred-001/credentials HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.write\r\n\r\n{\"credentialType\":\"hmac\"}",
    )
    .expect("backend devices.credentials.create sqlite");
    assert!(created.starts_with("HTTP/1.1 201"));
    assert!(created.contains(r#""deviceId":"sqlite-cred-001""#));
    assert!(created.contains(r#""credentialType":"hmac""#));

    let listed = handle_api_request_bytes(
        &admin,
        b"GET /backend/v3/api/iot/devices/sqlite-cred-001/credentials HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.devices.read\r\n\r\n",
    )
    .expect("backend devices.credentials.list sqlite");
    assert!(listed.starts_with("HTTP/1.1 200"));
    assert!(listed.contains(r#""credentialType":"hmac""#));

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn sqlite_catalog_and_firmware_handles_persist_across_reopen() {
    let temp_path = std::env::temp_dir().join(format!(
        "sdkwork-iot-platform-service-admin-{}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    let catalog = Arc::new(
        sdkwork_iot_platform_service::AiotCatalogRepositoryHandle::open_sqlite(&temp_path)
            .expect("open sqlite catalog handle"),
    );
    let firmware = Arc::new(
        sdkwork_iot_platform_service::AiotFirmwareRepositoryHandle::open_sqlite(&temp_path)
            .expect("open sqlite firmware handle"),
    );
    let admin = standard_admin_api_server()
        .expect("admin api server")
        .with_catalog_repository(catalog)
        .with_firmware_repository(firmware);

    let create_product = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/products HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.products.write\r\n\r\n{\"productId\":\"sqlite-product-001\",\"displayName\":\"SQLite Product\",\"defaultHardwareProfileId\":\"hw-esp32-s3\",\"defaultProtocolProfileId\":\"proto-xiaozhi\",\"defaultCapabilityModelId\":\"capmodel-xiaozhi-core\"}",
    )
    .expect("backend products.create sqlite");
    assert!(create_product.starts_with("HTTP/1.1 201"));
    assert!(create_product.contains(r#""productId":"sqlite-product-001""#));

    let create_hardware = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/hardware_profiles HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.profiles.write\r\n\r\n{\"hardwareProfileId\":\"hw-sqlite-001\",\"chipFamily\":\"esp32_s3\",\"hardwareClasses\":[\"mcu\"],\"runtimeProfiles\":[\"esp_idf\"],\"connectivityProfiles\":[\"wifi\"],\"securityProfiles\":[\"device_secret\"],\"otaProfiles\":[\"xiaozhi_ota\"]}",
    )
    .expect("backend hardwareProfiles.create sqlite");
    assert!(create_hardware.starts_with("HTTP/1.1 201"));
    assert!(create_hardware.contains(r#""hardwareProfileId":"hw-sqlite-001""#));

    let create_artifact = handle_api_request_bytes(
        &admin,
        b"POST /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.write\r\n\r\n{\"artifactKey\":\"fw-sqlite\",\"version\":\"1.0.0\",\"resource\":{\"id\":\"media-res-sqlite\",\"kind\":\"document\",\"source\":\"object_storage\",\"objectBlobId\":\"obj-blob-sqlite\",\"mimeType\":\"application/octet-stream\",\"sizeBytes\":\"1024\"},\"sha256\":\"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789\"}",
    )
    .expect("backend firmwareArtifacts.create sqlite");
    assert!(create_artifact.starts_with("HTTP/1.1 201"));
    assert!(create_artifact.contains(r#""artifactId":"firmware-artifact-0001""#));

    drop(admin);

    let reopened_catalog = Arc::new(
        sdkwork_iot_platform_service::AiotCatalogRepositoryHandle::open_sqlite(&temp_path)
            .expect("reopen sqlite catalog handle"),
    );
    let reopened_firmware = Arc::new(
        sdkwork_iot_platform_service::AiotFirmwareRepositoryHandle::open_sqlite(&temp_path)
            .expect("reopen sqlite firmware handle"),
    );
    let reopened_admin = standard_admin_api_server()
        .expect("reopened admin api server")
        .with_catalog_repository(reopened_catalog)
        .with_firmware_repository(reopened_firmware);

    let get_product = handle_api_request_bytes(
        &reopened_admin,
        b"GET /backend/v3/api/iot/products/sqlite-product-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.products.read\r\n\r\n",
    )
    .expect("backend products.retrieve sqlite");
    assert!(get_product.starts_with("HTTP/1.1 200"));
    assert!(get_product.contains(r#""productId":"sqlite-product-001""#));

    let get_hardware = handle_api_request_bytes(
        &reopened_admin,
        b"GET /backend/v3/api/iot/hardware_profiles/hw-sqlite-001 HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.profiles.read\r\n\r\n",
    )
    .expect("backend hardwareProfiles.retrieve sqlite");
    assert!(get_hardware.starts_with("HTTP/1.1 200"));
    assert!(get_hardware.contains(r#""hardwareProfileId":"hw-sqlite-001""#));

    let list_artifacts = handle_api_request_bytes(
        &reopened_admin,
        b"GET /backend/v3/api/iot/firmware_artifacts HTTP/1.1\r\nHost: local\r\nAuthorization: Bearer app-token\r\nAccess-Token: user-token\r\nX-Sdkwork-Tenant-Id: 100001\r\nX-Sdkwork-Organization-Id: 0\r\nX-Sdkwork-Permission-Scope: iot.firmware.read\r\n\r\n",
    )
    .expect("backend firmwareArtifacts.list sqlite");
    assert!(list_artifacts.starts_with("HTTP/1.1 200"));
    assert!(list_artifacts.contains(r#""artifactId":"firmware-artifact-0001""#));
    assert!(list_artifacts.contains(r#""artifactKey":"fw-sqlite""#));

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn export_route_manifest_artifacts_when_requested() {
    if std::env::var("SDKWORK_EXPORT_ROUTE_MANIFESTS").as_deref() != Ok("1") {
        return;
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root");
    let app_dir = root.join("sdks/_route-manifests/app-api");
    let backend_dir = root.join("sdks/_route-manifests/backend-api");
    std::fs::create_dir_all(&app_dir).expect("create app-api route manifest directory");
    std::fs::create_dir_all(&backend_dir).expect("create backend-api route manifest directory");
    std::fs::write(
        app_dir.join("sdkwork-aiot-app-api.route-manifest.json"),
        sdkwork_iot_platform_service::standard_route_manifest_json(AiotApiSurface::App),
    )
    .expect("write app route manifest");
    std::fs::write(
        backend_dir.join("sdkwork-aiot-admin-api.route-manifest.json"),
        sdkwork_iot_platform_service::standard_route_manifest_json(AiotApiSurface::Admin),
    )
    .expect("write backend route manifest");
}

#[test]
fn standard_route_manifest_documents_match_route_contracts() {
    for surface in [AiotApiSurface::App, AiotApiSurface::Admin] {
        let document = sdkwork_iot_platform_service::standard_route_manifest_document(surface);
        let routes = document
            .get("routes")
            .and_then(serde_json::Value::as_array)
            .expect("routes array");
        let contract_count = standard_api_route_contracts()
            .into_iter()
            .filter(|route| route.surface == surface)
            .count();
        assert_eq!(routes.len(), contract_count);
    }
}
