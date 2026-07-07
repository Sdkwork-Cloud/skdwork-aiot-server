use sdkwork_aiot_adapter_xiaozhi::XiaozhiOtaMetadata;
use sdkwork_aiot_cloud_gateway::{
    standard_gateway_server,
    standard_gateway_server_and_session_options_with_plugins_activation_registry_and_mcp_tools,
    standard_gateway_server_with_plugins,
    standard_gateway_server_with_plugins_activation_registry_and_mcp_tools,
    xiaozhi_device_token_valid, xiaozhi_mqtt_goodbye_message, xiaozhi_mqtt_server_teardown_reply,
    xiaozhi_mqtt_session_reply, xiaozhi_mqtt_udp_decode_audio,
    xiaozhi_mqtt_udp_uplink_speech_reply, xiaozhi_simulator_http_handler,
    xiaozhi_websocket_session_reply, xiaozhi_websocket_session_reply_with_mcp_tool_provider,
    DefaultXiaozhiActivationVerifier, DefaultXiaozhiSimulatorMcpToolProvider,
    InMemoryXiaozhiActivationChallengeRegistry, RolloutAwareXiaozhiOtaProfileProvider,
    WebSocketSessionReply, XiaozhiActivationVerifier, XiaozhiMqttUdpSession,
    XiaozhiOtaProfileProvider, XiaozhiSessionOptions,
};
use sdkwork_aiot_storage::AiotStorageAssociation;
use sdkwork_aiot_storage_sqlx::{
    FirmwareOtaCatalog, SqliteCredentialCreateCommand, SqlitePersistedEntityRepository,
    SqliteSqlxCredentialRepository,
};
use sdkwork_aiot_transport::parse_http_request_bytes;
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod test_env {
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub fn setup() {
        INIT.call_once(|| {
            std::env::set_var("SDKWORK_AIOT_DEV_MODE", "1");
        });
    }
}

fn handle_http_request_bytes(
    server: &sdkwork_aiot_transport::TransportServer,
    bytes: &[u8],
) -> Result<String, sdkwork_aiot_transport::TransportError> {
    test_env::setup();
    sdkwork_aiot_transport::handle_http_request_bytes(server, bytes)
}

#[derive(Debug, Default)]
struct ErroringToolInvoker;

impl sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolInvoker for ErroringToolInvoker {
    fn invoke(
        &self,
        _context: &sdkwork_aiot_cloud_gateway::XiaozhiMcpInvocationContext,
        _tool: &sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec,
        _tool_arguments_json: Option<&str>,
    ) -> Result<String, String> {
        Err("invoker rejected tool call".to_string())
    }
}

#[derive(Debug, Default)]
struct ContextEchoToolInvoker;

impl sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolInvoker for ContextEchoToolInvoker {
    fn invoke(
        &self,
        context: &sdkwork_aiot_cloud_gateway::XiaozhiMcpInvocationContext,
        _tool: &sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec,
        _tool_arguments_json: Option<&str>,
    ) -> Result<String, String> {
        Ok(format!(
            "transport={} session={} device={} client={}",
            context.transport,
            context.session_id,
            context.device_id.as_deref().unwrap_or(""),
            context.client_id.as_deref().unwrap_or("")
        ))
    }
}

#[derive(Debug, Default)]
struct DenyAllToolPolicy;

impl sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolPolicy for DenyAllToolPolicy {
    fn allow(
        &self,
        _context: &sdkwork_aiot_cloud_gateway::XiaozhiMcpInvocationContext,
        _tool: &sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec,
        _tool_arguments_json: Option<&str>,
    ) -> Result<(), String> {
        Err("tool call denied by policy".to_string())
    }
}

#[test]
fn standard_gateway_server_mounts_xiaozhi_ota_compatibility_route() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""websocket":{"url":"wss://domain/iot/xiaozhi/ws""#));
    assert!(response.contains(r#""version":3"#));
    assert!(response.contains(r#""server_time":{"timestamp":"#));
}

#[test]
fn standard_gateway_server_uses_plain_ws_for_local_xiaozhi_ota_debugging() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: 127.0.0.1:18080\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi local ota response");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""websocket":{"url":"ws://127.0.0.1:18080/iot/xiaozhi/ws""#));
}

#[test]
fn standard_gateway_server_can_publish_udp_crypto_profile_in_ota_response() {
    let _guard = EnvGuard::set_all_locked(&[
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_SERVER", Some("127.0.0.1")),
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_PORT", Some("8888")),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_KEY_HEX",
            Some("00112233445566778899AABBCCDDEEFF"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_NONCE_HEX",
            Some("01000000A1A2A3A40000000000000000"),
        ),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(
        r#""udp":{"server":"127.0.0.1","port":8888,"key":"00112233445566778899AABBCCDDEEFF","nonce":"01000000A1A2A3A40000000000000000"}"#
    ));
}

#[test]
fn standard_gateway_server_supports_pluggable_activation_verifier_and_ota_provider() {
    #[derive(Debug)]
    struct TestVerifier;
    impl XiaozhiActivationVerifier for TestVerifier {
        fn is_accepted(&self, request: &sdkwork_aiot_transport::HttpRequest) -> bool {
            request.body == br#"{"allow":true}"#.to_vec()
        }
    }

    #[derive(Debug)]
    struct TestOtaProvider;
    impl XiaozhiOtaProfileProvider for TestOtaProvider {
        fn enrich(
            &self,
            _request: &sdkwork_aiot_transport::HttpRequest,
            metadata: XiaozhiOtaMetadata,
        ) -> XiaozhiOtaMetadata {
            metadata.with_firmware("9.9.9", "https://example.com/fw.bin", true)
        }
    }

    let server =
        standard_gateway_server_with_plugins(Arc::new(TestOtaProvider), Arc::new(TestVerifier))
            .expect("gateway server");

    let activation_ok = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{\"allow\":true}",
    )
    .expect("activation accepted");
    assert!(activation_ok.starts_with("HTTP/1.1 200 OK"));

    let activation_pending = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{\"allow\":false}",
    )
    .expect("activation pending");
    assert!(activation_pending.starts_with("HTTP/1.1 202 Accepted"));

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("ota with plugin provider");
    assert!(ota.contains(
        r#""firmware":{"version":"9.9.9","url":"https://example.com/fw.bin","force":1}"#
    ));
}

#[test]
fn xiaozhi_mqtt_session_hello_reply_contains_udp_profile_and_session_coordinates() {
    let _guard = EnvGuard::set_all_locked(&[
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_SERVER", Some("127.0.0.1")),
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_PORT", Some("8888")),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_KEY_HEX",
            Some("00112233445566778899AABBCCDDEEFF"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_NONCE_HEX",
            Some("01000000A1A2A3A40000000000000000"),
        ),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let (reply, session) = xiaozhi_mqtt_session_reply(
        &server,
        None,
        r#"{"type":"hello","version":3,"transport":"udp","device_id":"dev-xyz","client_id":"client-xyz","features":{"mcp":true},"audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
    )
    .expect("mqtt hello response");

    let session = session.expect("session");
    assert_eq!(session.session_id, "dev-xyz-client-xyz");
    assert!(reply
        .outbound_json
        .iter()
        .any(|value| value.contains(r#""session_id":"dev-xyz-client-xyz""#)));
    assert!(reply
        .outbound_json
        .iter()
        .any(|value| value.contains(r#""udp":{"server":"127.0.0.1","port":8888"#)));
}

#[test]
fn standard_gateway_server_mounts_xiaozhi_activation_pending_route() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi activation response");

    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
    assert!(response.contains(r#""activation":{"status":"pending""#));
}

#[test]
fn standard_gateway_server_mounts_xiaozhi_ota_activate_alias_route() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota/activate HTTP/1.1\r\nHost: domain\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota/activate response");

    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
    assert!(response.contains(r#""activation":{"status":"pending""#));
}

#[test]
fn standard_gateway_server_accepts_activation_with_issued_challenge_and_rejects_replay() {
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-001"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-001"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-001"),
        ),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let before_issue = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-001\r\nClient-Id: client-001\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-001\",\"hmac\":\"hmac-001\"}",
    )
    .expect("xiaozhi activation pending before issue");
    assert!(before_issue.starts_with("HTTP/1.1 202 Accepted"));

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-001\r\nClient-Id: client-001\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");
    assert!(ota.contains(r#""activation":{"message":"activation pending","challenge":"challenge-001","timeout_ms":30000}"#));

    let accepted = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-001\r\nClient-Id: client-001\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-001\",\"hmac\":\"hmac-001\"}",
    )
    .expect("xiaozhi ota/activate accepted response");
    assert!(accepted.starts_with("HTTP/1.1 200 OK"));
    assert!(accepted.contains(r#""activation":{"status":"accepted"}"#));

    let replay = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-001\r\nClient-Id: client-001\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-001\",\"hmac\":\"hmac-001\"}",
    )
    .expect("xiaozhi activation replay response");
    assert!(replay.starts_with("HTTP/1.1 202 Accepted"));
    assert!(replay.contains(r#""activation":{"status":"pending""#));
}

#[test]
fn standard_gateway_server_strict_v2_activation_requires_algorithm_and_serial_alignment() {
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-v2"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-v2"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-v2"),
        ),
        ("SDKWORK_AIOT_XIAOZHI_ACTIVATE_STRICT_V2", Some("1")),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-v2\r\nClient-Id: client-v2\r\nActivation-Version: 2\r\nSerial-Number: SN-V2-001\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");
    assert!(ota.contains(r#""challenge":"challenge-v2""#));

    let missing_v2_fields = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-v2\r\nClient-Id: client-v2\r\nActivation-Version: 2\r\nSerial-Number: SN-V2-001\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-v2\",\"hmac\":\"hmac-v2\"}",
    )
    .expect("xiaozhi strict v2 missing fields response");
    assert!(missing_v2_fields.starts_with("HTTP/1.1 202 Accepted"));

    let invalid_algorithm = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-v2\r\nClient-Id: client-v2\r\nActivation-Version: 2\r\nSerial-Number: SN-V2-001\r\nContent-Type: application/json\r\n\r\n{\"algorithm\":\"hmac-sha1\",\"serial_number\":\"SN-V2-001\",\"challenge\":\"challenge-v2\",\"hmac\":\"hmac-v2\"}",
    )
    .expect("xiaozhi strict v2 invalid algorithm response");
    assert!(invalid_algorithm.starts_with("HTTP/1.1 202 Accepted"));

    let serial_mismatch = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-v2\r\nClient-Id: client-v2\r\nActivation-Version: 2\r\nSerial-Number: SN-V2-001\r\nContent-Type: application/json\r\n\r\n{\"algorithm\":\"hmac-sha256\",\"serial_number\":\"SN-V2-999\",\"challenge\":\"challenge-v2\",\"hmac\":\"hmac-v2\"}",
    )
    .expect("xiaozhi strict v2 serial mismatch response");
    assert!(serial_mismatch.starts_with("HTTP/1.1 202 Accepted"));

    let accepted = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-v2\r\nClient-Id: client-v2\r\nActivation-Version: 2\r\nSerial-Number: SN-V2-001\r\nContent-Type: application/json\r\n\r\n{\"algorithm\":\"hmac-sha256\",\"serial_number\":\"SN-V2-001\",\"challenge\":\"challenge-v2\",\"hmac\":\"hmac-v2\"}",
    )
    .expect("xiaozhi strict v2 accepted response");
    assert!(accepted.starts_with("HTTP/1.1 200 OK"));
    assert!(accepted.contains(r#""activation":{"status":"accepted"}"#));
}

#[test]
fn standard_gateway_server_strict_v2_activation_keeps_legacy_non_v2_path() {
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-legacy"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-legacy"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-legacy"),
        ),
        ("SDKWORK_AIOT_XIAOZHI_ACTIVATE_STRICT_V2", Some("1")),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-legacy\r\nClient-Id: client-legacy\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");
    assert!(ota.contains(r#""challenge":"challenge-legacy""#));

    let accepted = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-legacy\r\nClient-Id: client-legacy\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-legacy\",\"hmac\":\"hmac-legacy\"}",
    )
    .expect("xiaozhi strict v2 legacy accepted response");
    assert!(accepted.starts_with("HTTP/1.1 200 OK"));
    assert!(accepted.contains(r#""activation":{"status":"accepted"}"#));
}

#[test]
fn standard_gateway_server_rejects_expired_activation_challenge() {
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-expired"),
        ),
        ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_TIMEOUT_MS", Some("0")),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-expired"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-expired"),
        ),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-exp\r\nClient-Id: client-exp\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response");
    assert!(ota.contains(r#""challenge":"challenge-expired""#));

    let activation = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-exp\r\nClient-Id: client-exp\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-expired\",\"hmac\":\"hmac-expired\"}",
    )
    .expect("xiaozhi activation timeout response");
    assert!(activation.starts_with("HTTP/1.1 202 Accepted"));
    assert!(activation.contains(r#""activation":{"status":"pending""#));
}

#[test]
fn standard_gateway_server_persists_activation_challenge_when_registry_path_is_configured() {
    let registry_path = unique_temp_file_path("xiaozhi-activation-registry", "state");
    let registry_path_text = registry_path.to_string_lossy().into_owned();
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
            Some(registry_path_text.as_str()),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-persist"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-persist"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-persist"),
        ),
    ]);

    let server_before_restart = standard_gateway_server().expect("gateway server before restart");
    let ota = handle_http_request_bytes(
        &server_before_restart,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-persist\r\nClient-Id: client-persist\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response before restart");
    assert!(ota.contains(r#""challenge":"challenge-persist""#));
    drop(server_before_restart);

    let server_after_restart = standard_gateway_server().expect("gateway server after restart");
    let activation = handle_http_request_bytes(
        &server_after_restart,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-persist\r\nClient-Id: client-persist\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-persist\",\"hmac\":\"hmac-persist\"}",
    )
    .expect("xiaozhi activation response after restart");
    assert!(activation.starts_with("HTTP/1.1 200 OK"));
    assert!(activation.contains(r#""activation":{"status":"accepted"}"#));

    let _ = fs::remove_file(&registry_path);
}

#[test]
fn file_backed_activation_registry_keeps_state_across_two_concurrent_servers() {
    let registry_path = unique_temp_file_path("xiaozhi-activation-registry-concurrent", "state");
    let registry_path_text = registry_path.to_string_lossy().into_owned();
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
            Some(registry_path_text.as_str()),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-shared"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-shared"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-shared"),
        ),
    ]);

    let server_a = standard_gateway_server().expect("gateway server a");
    let server_b = standard_gateway_server().expect("gateway server b");

    let ota_a = handle_http_request_bytes(
        &server_a,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-a\r\nClient-Id: client-a\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response a");
    assert!(ota_a.contains(r#""challenge":"challenge-shared""#));

    let ota_b = handle_http_request_bytes(
        &server_b,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-b\r\nClient-Id: client-b\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response b");
    assert!(ota_b.contains(r#""challenge":"challenge-shared""#));

    let activate_a = handle_http_request_bytes(
        &server_a,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-a\r\nClient-Id: client-a\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-shared\",\"hmac\":\"hmac-shared\"}",
    )
    .expect("xiaozhi activate response a");
    assert!(activate_a.starts_with("HTTP/1.1 200 OK"));

    let activate_b = handle_http_request_bytes(
        &server_b,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-b\r\nClient-Id: client-b\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-shared\",\"hmac\":\"hmac-shared\"}",
    )
    .expect("xiaozhi activate response b");
    assert!(activate_b.starts_with("HTTP/1.1 200 OK"));

    let _ = fs::remove_file(&registry_path);
    let lock_path = PathBuf::from(format!("{}.lock", registry_path.to_string_lossy()));
    let _ = fs::remove_file(lock_path);
}

#[test]
fn sqlite_activation_registry_keeps_state_across_two_concurrent_servers() {
    let registry_path =
        unique_temp_file_path("xiaozhi-activation-registry-sqlite-concurrent", "db");
    let registry_path_text = registry_path.to_string_lossy().into_owned();
    let _guard = EnvGuard::set_all_locked(&[
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
            Some(registry_path_text.as_str()),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
            Some("sqlite"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
            Some("challenge-shared"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
            Some("challenge-shared"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
            Some("hmac-shared"),
        ),
    ]);

    let server_a = standard_gateway_server().expect("gateway server a");
    let server_b = standard_gateway_server().expect("gateway server b");

    let ota_a = handle_http_request_bytes(
        &server_a,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-a\r\nClient-Id: client-a\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response a");
    assert!(ota_a.contains(r#""challenge":"challenge-shared""#));

    let ota_b = handle_http_request_bytes(
        &server_b,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-b\r\nClient-Id: client-b\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("xiaozhi ota response b");
    assert!(ota_b.contains(r#""challenge":"challenge-shared""#));

    let activate_a = handle_http_request_bytes(
        &server_a,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-a\r\nClient-Id: client-a\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-shared\",\"hmac\":\"hmac-shared\"}",
    )
    .expect("xiaozhi activate response a");
    assert!(activate_a.starts_with("HTTP/1.1 200 OK"));

    let activate_b = handle_http_request_bytes(
        &server_b,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\nDevice-Id: dev-b\r\nClient-Id: client-b\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-shared\",\"hmac\":\"hmac-shared\"}",
    )
    .expect("xiaozhi activate response b");
    assert!(activate_b.starts_with("HTTP/1.1 200 OK"));

    let _ = fs::remove_file(&registry_path);
}

#[test]
fn standard_gateway_server_builder_accepts_injected_mcp_tool_provider() {
    #[derive(Debug)]
    struct TestOtaProvider;
    impl XiaozhiOtaProfileProvider for TestOtaProvider {
        fn enrich(
            &self,
            _request: &sdkwork_aiot_transport::HttpRequest,
            metadata: XiaozhiOtaMetadata,
        ) -> XiaozhiOtaMetadata {
            metadata
        }
    }

    #[derive(Debug)]
    struct TestVerifier;
    impl XiaozhiActivationVerifier for TestVerifier {
        fn is_accepted(&self, _request: &sdkwork_aiot_transport::HttpRequest) -> bool {
            false
        }
    }

    let server = standard_gateway_server_with_plugins_activation_registry_and_mcp_tools(
        Arc::new(TestOtaProvider),
        Arc::new(TestVerifier),
        Arc::new(InMemoryXiaozhiActivationChallengeRegistry::new()),
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.ping",
                "Ping custom provider.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
    )
    .expect("gateway server");

    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");
    let provider = DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
        sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
            "self.custom.ping",
            "Ping custom provider.",
            r#"{"type":"object","properties":{},"required":[]}"#,
        ),
    ]);
    let replies = xiaozhi_websocket_session_reply_with_mcp_tool_provider(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":31,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
        &provider,
    )
    .expect("tools list response");
    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value.contains(r#""name":"self.custom.ping""#)));
}

#[test]
fn xiaozhi_websocket_session_reply_with_options_uses_injected_provider() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.only_from_options",
                "Option-scoped provider tool.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ]),
    ));

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":311,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
        &options,
    )
    .expect("tools list response");
    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value.contains(r#""name":"self.custom.only_from_options""#)));
}

#[test]
fn xiaozhi_websocket_session_reply_with_options_surfaces_invoker_error() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_and_invoker(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.invoker_error",
                "Tool for invoker error test.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ErroringToolInvoker),
    );

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":313,"method":"tools/call","params":{"name":"self.custom.invoker_error","arguments":{}}}}"#,
        ),
        &options,
    )
    .expect("tools call response");
    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value
            .contains(r#""error":{"code":-32601,"message":"invoker rejected tool call"}"#)));
}

#[test]
fn xiaozhi_websocket_session_reply_with_options_passes_invocation_context() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=ctx-device&client_id=ctx-client HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_and_invoker(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.context_echo",
                "Tool for context echo.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ContextEchoToolInvoker),
    );

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"ctx-device-ctx-client","type":"mcp","payload":{"jsonrpc":"2.0","id":314,"method":"tools/call","params":{"name":"self.custom.context_echo","arguments":{}}}}"#,
        ),
        &options,
    )
    .expect("tools call response");
    let text = reply_texts(&replies);
    assert!(text.iter().any(|value| value.contains(
        r#""text":"transport=websocket session=ctx-device-ctx-client device=ctx-device client=ctx-client""#
    )));
}

#[test]
fn xiaozhi_websocket_session_reply_with_options_surfaces_policy_rejection() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=deny-device&client_id=deny-client HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_invoker_and_policy(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.policy_denied",
                "Tool for policy deny test.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ContextEchoToolInvoker),
        Arc::new(DenyAllToolPolicy),
    );

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"deny-device-deny-client","type":"mcp","payload":{"jsonrpc":"2.0","id":315,"method":"tools/call","params":{"name":"self.custom.policy_denied","arguments":{}}}}"#,
        ),
        &options,
    )
    .expect("tools call response");
    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value
            .contains(r#""error":{"code":-32601,"message":"tool call denied by policy"}"#)));
}

#[test]
fn standard_gateway_server_and_session_options_builder_propagates_mcp_provider() {
    #[derive(Debug)]
    struct TestOtaProvider;
    impl XiaozhiOtaProfileProvider for TestOtaProvider {
        fn enrich(
            &self,
            _request: &sdkwork_aiot_transport::HttpRequest,
            metadata: XiaozhiOtaMetadata,
        ) -> XiaozhiOtaMetadata {
            metadata
        }
    }

    #[derive(Debug)]
    struct TestVerifier;
    impl XiaozhiActivationVerifier for TestVerifier {
        fn is_accepted(&self, _request: &sdkwork_aiot_transport::HttpRequest) -> bool {
            false
        }
    }

    let (server, options) =
        standard_gateway_server_and_session_options_with_plugins_activation_registry_and_mcp_tools(
            Arc::new(TestOtaProvider),
            Arc::new(TestVerifier),
            Arc::new(InMemoryXiaozhiActivationChallengeRegistry::new()),
            Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
                sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                    "self.custom.from_assembly_options",
                    "Assembly-propagated provider tool.",
                    r#"{"type":"object","properties":{},"required":[]}"#,
                ),
            ])),
        )
        .expect("gateway assembly with options");

    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":312,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
        &options,
    )
    .expect("tools list response");
    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value.contains(r#""name":"self.custom.from_assembly_options""#)));
}

#[test]
fn standard_gateway_server_enforces_xiaozhi_activation_post_method() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"GET /iot/xiaozhi/activate HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("xiaozhi activation method response");

    assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(response.contains("gateway.xiaozhi.activate.method"));
}

#[test]
fn standard_gateway_server_serves_xiaozhi_ui_simulator() {
    let server = standard_gateway_server().expect("gateway server");

    let response = handle_http_request_bytes(
        &server,
        b"GET /simulators/xiaozhi HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("simulator response");

    assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(response.contains("gateway.xiaozhi.simulator.ui.migrated"));
    assert!(response.contains("sdkwork-aiot-xiaozhi-simulator-ui"));
}

#[test]
fn xiaozhi_websocket_session_replies_to_browser_simulator_hello_with_server_hello() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
    )
    .expect("request");

    let replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"type":"hello","version":3,"features":{"mcp":true},"transport":"websocket","audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
        ),
    )
    .expect("reply");

    assert_eq!(replies.len(), 2);
    let WebSocketSessionReply::Text(first) = &replies[0] else {
        panic!("expected text hello reply");
    };
    let WebSocketSessionReply::Text(second) = &replies[1] else {
        panic!("expected text mcp reply");
    };
    assert!(first.contains(r#""type":"hello""#));
    assert!(first.contains(r#""transport":"websocket""#));
    assert!(first.contains(r#""session_id":"dev-001-browser-001""#));
    assert!(second.contains(r#""type":"mcp""#));
    assert!(second.contains(r#""method":"initialize""#));
}

#[test]
fn xiaozhi_websocket_session_reply_with_options_persists_protocol_storage_command() {
    use sdkwork_aiot_storage::{AiotStorageWriteKind, InMemoryProtocolIngestUnitOfWork};

    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
    )
    .expect("request");
    let protocol_ingest = Arc::new(InMemoryProtocolIngestUnitOfWork::new());
    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_env(),
    ))
    .with_protocol_ingest(protocol_ingest.clone());

    let replies = sdkwork_aiot_cloud_gateway::xiaozhi_websocket_session_reply_with_options(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"type":"hello","version":3,"features":{"mcp":true},"transport":"websocket","session_id":"session-001","message_id":"msg-001","idempotency_key":"idem-001","trace_id":"trace-001","audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
        ),
        &options,
    )
    .expect("reply");

    assert!(!replies.is_empty());
    let snapshot = protocol_ingest.snapshot();
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
    assert_eq!(
        snapshot.primary_writes[0].write_kind,
        AiotStorageWriteKind::OpenSession
    );
    assert_eq!(
        snapshot.primary_writes[0].idempotency_key.as_str(),
        "idem-001"
    );
}

#[test]
fn xiaozhi_mqtt_session_reply_with_options_persists_protocol_storage_command() {
    use sdkwork_aiot_storage::{AiotStorageWriteKind, InMemoryProtocolIngestUnitOfWork};

    let server = standard_gateway_server().expect("gateway server");
    let protocol_ingest = Arc::new(InMemoryProtocolIngestUnitOfWork::new());
    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_env(),
    ))
    .with_protocol_ingest(protocol_ingest.clone());

    let (reply, _session) = sdkwork_aiot_cloud_gateway::xiaozhi_mqtt_session_reply_with_options(
        &server,
        None,
        r#"{"type":"hello","version":3,"features":{"mcp":true},"transport":"udp","device_id":"mqtt-device-001","client_id":"mqtt-client-001","session_id":"mqtt-session-001","message_id":"mqtt-msg-001","idempotency_key":"mqtt-idem-001","trace_id":"mqtt-trace-001","audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
        &options,
    )
    .expect("mqtt reply");

    assert!(!reply.outbound_json.is_empty());
    let snapshot = protocol_ingest.snapshot();
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
    assert_eq!(
        snapshot.primary_writes[0].write_kind,
        AiotStorageWriteKind::OpenSession
    );
    assert!(
        !snapshot.primary_writes[0].idempotency_key.is_empty(),
        "mqtt protocol ingest must preserve idempotency metadata",
    );
}

#[test]
fn xiaozhi_websocket_session_acknowledges_listen_mcp_and_audio_frames() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let listen_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"listen","state":"start","mode":"manual"}"#,
        ),
    )
    .expect("listen reply");
    let listen_reply_text = reply_texts(&listen_replies);
    assert!(listen_reply_text
        .iter()
        .any(|reply| reply.contains(r#""type":"stt""#)));
    assert!(listen_reply_text
        .iter()
        .any(|reply| reply.contains(r#""type":"tts","state":"start""#)));
    assert!(
        listen_replies
            .iter()
            .any(|reply| matches!(reply, WebSocketSessionReply::Binary(_))),
        "listen flow must include binary opus downlink after tts start"
    );
    assert!(listen_reply_text
        .iter()
        .any(|reply| { reply.contains(r#""type":"tts","state":"sentence_start""#) }));

    let mcp_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"ok"}],"isError":false}}}"#,
        ),
    )
    .expect("mcp reply");
    let mcp_reply_text = reply_texts(&mcp_replies);
    assert!(mcp_reply_text
        .iter()
        .any(|reply| reply.contains(r#""method":"tools/list""#)));

    let mcp_notification_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","method":"notifications/state_changed","params":{"newState":"idle","oldState":"connecting"}}}"#,
        ),
    )
    .expect("mcp notification reply");
    assert!(mcp_notification_replies.is_empty());

    let invalid_jsonrpc_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"1.0","id":8,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
    )
    .expect("invalid jsonrpc reply");
    assert!(invalid_jsonrpc_replies.is_empty());

    let invalid_params_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":81,"method":"tools/list","params":"invalid"}}"#,
        ),
    )
    .expect("invalid params reply");
    assert!(invalid_params_replies.is_empty());

    let payload_only_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":82}}"#,
        ),
    )
    .expect("payload-only reply");
    assert!(payload_only_replies.is_empty());

    let audio_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame {
            opcode: sdkwork_aiot_transport::WebSocketOpcode::Binary,
            payload: vec![0, 0, 0, 3, 0x01, 0x02, 0x03],
        },
    )
    .expect("audio reply");
    let audio_reply_text = reply_texts(&audio_replies);
    assert!(audio_reply_text
        .iter()
        .any(|reply| reply.contains("received 3 bytes of opus audio")));
    assert!(audio_reply_text
        .iter()
        .any(|reply| reply.contains(r#""type":"tts","state":"stop""#)));
}

#[test]
fn xiaozhi_websocket_session_handles_control_frames_and_jsonrpc_string_ids() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let ping_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame {
            opcode: sdkwork_aiot_transport::WebSocketOpcode::Ping,
            payload: b"keepalive".to_vec(),
        },
    )
    .expect("ping reply");
    assert_eq!(
        ping_replies,
        vec![WebSocketSessionReply::Pong(b"keepalive".to_vec())]
    );

    let mcp_replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":"call-001","method":"tools/call","params":{"name":"self.light.set_rgb"}}}"#,
        ),
    )
    .expect("mcp reply");
    let mcp_reply_text = reply_texts(&mcp_replies);
    assert!(mcp_reply_text
        .iter()
        .any(|reply| reply.contains(r#""id":"call-001""#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_initialize_returns_protocol_metadata() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{"vision":{"url":"http://localhost/iot/xiaozhi/vision","token":"simulator-token"}}}}}"#,
        ),
    )
    .expect("mcp initialize reply");

    let text = reply_texts(&replies);
    assert!(text
        .iter()
        .any(|value| value.contains(r#""protocolVersion":"2024-11-05""#)));
    assert!(text.iter().any(|value| value
        .contains(r#""serverInfo":{"name":"sdkwork-aiot-cloud-gateway","version":"0.1.0"}"#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_list_honors_cursor_and_with_user_tools() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let first_page = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
    )
    .expect("tools list first page");
    let first_page_text = reply_texts(&first_page);
    assert!(first_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.get_device_status""#)));
    assert!(first_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.audio_speaker.set_volume""#)));
    assert!(first_page_text
        .iter()
        .any(|value| value.contains(r#""nextCursor":"self.screen.set_brightness""#)));
    assert!(first_page_text
        .iter()
        .all(|value| !value.contains(r#""name":"self.reboot""#)));

    let user_page = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"cursor":"self.screen.set_brightness","withUserTools":true}}}"#,
        ),
    )
    .expect("tools list user page");
    let user_page_text = reply_texts(&user_page);
    assert!(user_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.screen.set_brightness""#)));
    assert!(user_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.reboot""#)));
    assert!(user_page_text
        .iter()
        .any(|value| value.contains(r#""annotations":{"audience":["user"]}"#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_list_returns_error_for_unknown_cursor() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":9,"method":"tools/list","params":{"cursor":"cursor-not-exists","withUserTools":false}}}"#,
        ),
    )
    .expect("tools list error reply");
    let reply_text = reply_texts(&replies);
    assert!(reply_text.iter().any(|value| value
        .contains(r#""error":{"code":-32601,"message":"Unknown cursor: cursor-not-exists"}"#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_returns_missing_valid_argument_error() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let missing_argument = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":41,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{}}}}"#,
        ),
    )
    .expect("mcp missing argument");
    let missing_argument_text = reply_texts(&missing_argument);
    assert!(missing_argument_text.iter().any(|value| value
        .contains(r#""error":{"code":-32601,"message":"Missing valid argument: volume"}"#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_returns_external_style_range_errors() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let out_of_range = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":101}}}}"#,
        ),
    )
    .expect("mcp out of range argument");
    let out_of_range_text = reply_texts(&out_of_range);
    assert!(out_of_range_text.iter().any(|value| value
        .contains(r#""error":{"code":-32601,"message":"Value exceeds maximum allowed: 100"}"#)));

    let below_range = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":421,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":-1}}}}"#,
        ),
    )
    .expect("mcp below range argument");
    let below_range_text = reply_texts(&below_range);
    assert!(below_range_text.iter().any(|value| value
        .contains(r#""error":{"code":-32601,"message":"Value is below minimum allowed: 0"}"#)));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_accepts_decimal_for_integer_argument() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let decimal_integer = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":422,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":99.9}}}}"#,
        ),
    )
    .expect("mcp decimal integer argument");
    let decimal_integer_text = reply_texts(&decimal_integer);
    assert!(decimal_integer_text.iter().any(|value| value.contains(
        r#""result":{"content":[{"type":"text","text":"accepted by SDKWork simulator"}],"isError":false}"#
    )));

    let valid_boundary = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":43,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":100}}}}"#,
        ),
    )
    .expect("mcp valid argument");
    let valid_boundary_text = reply_texts(&valid_boundary);
    assert!(valid_boundary_text.iter().any(|value| value.contains(
        r#""result":{"content":[{"type":"text","text":"accepted by SDKWork simulator"}],"isError":false}"#
    )));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_honors_env_policy_rules() {
    let _guard = EnvGuard::set_all_locked(&[(
        "SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES",
        Some("deny|tool=self.audio_speaker.set_volume|transport=websocket|device_prefix=dev-001"),
    )]);
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let denied = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":423,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":100}}}}"#,
        ),
    )
    .expect("mcp policy denied argument");
    let denied_text = reply_texts(&denied);
    assert!(denied_text.iter().any(|value| value.contains(
        r#""error":{"code":-32601,"message":"Tool not allowed by policy: self.audio_speaker.set_volume"}"#
    )));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_honors_numeric_argument_policy_rules() {
    let _guard = EnvGuard::set_all_locked(&[(
        "SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES",
        Some("deny|tool=self.audio_speaker.set_volume|transport=websocket|arg_volume_gt=80"),
    )]);
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let allowed = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":424,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":80}}}}"#,
        ),
    )
    .expect("mcp policy allowed argument");
    let allowed_text = reply_texts(&allowed);
    assert!(allowed_text.iter().any(|value| value.contains(
        r#""result":{"content":[{"type":"text","text":"accepted by SDKWork simulator"}],"isError":false}"#
    )));

    let denied = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":425,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":{"volume":81}}}}"#,
        ),
    )
    .expect("mcp policy denied argument");
    let denied_text = reply_texts(&denied);
    assert!(denied_text.iter().any(|value| value.contains(
        r#""error":{"code":-32601,"message":"Tool not allowed by policy: self.audio_speaker.set_volume"}"#
    )));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_honors_string_argument_policy_rules() {
    let _guard = EnvGuard::set_all_locked(&[(
        "SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES",
        Some("deny|tool=self.reboot|transport=websocket|arg_mode_str_eq=night"),
    )]);
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let denied = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":426,"method":"tools/call","params":{"name":"self.reboot","arguments":{"mode":"night"}}}}"#,
        ),
    )
    .expect("mcp policy denied by string argument");
    let denied_text = reply_texts(&denied);
    assert!(denied_text.iter().any(|value| value.contains(
        r#""error":{"code":-32601,"message":"Tool not allowed by policy: self.reboot"}"#
    )));
}

#[test]
fn xiaozhi_websocket_session_mcp_tools_call_returns_external_style_precondition_errors() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let missing_params = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":51,"method":"tools/call"}}"#,
        ),
    )
    .expect("mcp missing params");
    let missing_params_text = reply_texts(&missing_params);
    assert!(missing_params_text
        .iter()
        .any(|value| value.contains(r#""error":{"code":-32601,"message":"Missing params"}"#)));

    let missing_name = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":52,"method":"tools/call","params":{}}}"#,
        ),
    )
    .expect("mcp missing name");
    let missing_name_text = reply_texts(&missing_name);
    assert!(missing_name_text
        .iter()
        .any(|value| value.contains(r#""error":{"code":-32601,"message":"Missing name"}"#)));

    let invalid_arguments = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":53,"method":"tools/call","params":{"name":"self.audio_speaker.set_volume","arguments":"bad-args"}}}"#,
        ),
    )
    .expect("mcp invalid arguments");
    let invalid_arguments_text = reply_texts(&invalid_arguments);
    assert!(invalid_arguments_text
        .iter()
        .any(|value| value.contains(r#""error":{"code":-32601,"message":"Invalid arguments"}"#)));
}

#[test]
fn xiaozhi_websocket_session_loads_mcp_tools_from_config_file() {
    let tools_path = unique_temp_file_path("xiaozhi-simulator-tools", "json");
    fs::write(
        &tools_path,
        r#"{
  "tools": [
    {
      "name": "self.device.echo",
      "description": "Echo a message.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "message": { "type": "string" }
        },
        "required": ["message"]
      },
      "userOnly": false,
      "resultText": "echo from config"
    },
    {
      "name": "self.device.factory_reset",
      "description": "Factory reset device.",
      "inputSchema": {
        "type": "object",
        "properties": {},
        "required": []
      },
      "userOnly": true
    }
  ]
}"#,
    )
    .expect("write mcp tools config");
    let mcp_tool_provider =
        DefaultXiaozhiSimulatorMcpToolProvider::from_path(&tools_path).expect("mcp tools provider");

    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let first_page = xiaozhi_websocket_session_reply_with_mcp_tool_provider(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":12,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        ),
        &mcp_tool_provider,
    )
    .expect("tools list first page");
    let first_page_text = reply_texts(&first_page);
    assert!(first_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.device.echo""#)));
    assert!(first_page_text
        .iter()
        .all(|value| !value.contains(r#""name":"self.device.factory_reset""#)));
    assert!(first_page_text
        .iter()
        .all(|value| !value.contains(r#""name":"self.get_device_status""#)));

    let call_echo = xiaozhi_websocket_session_reply_with_mcp_tool_provider(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":121,"method":"tools/call","params":{"name":"self.device.echo","arguments":{"message":"hello"}}}}"#,
        ),
        &mcp_tool_provider,
    )
    .expect("tools call echo");
    let call_echo_text = reply_texts(&call_echo);
    assert!(call_echo_text
        .iter()
        .any(|value| value.contains(r#""text":"echo from config""#)));

    let user_page = xiaozhi_websocket_session_reply_with_mcp_tool_provider(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"mcp","payload":{"jsonrpc":"2.0","id":13,"method":"tools/list","params":{"cursor":"","withUserTools":true}}}"#,
        ),
        &mcp_tool_provider,
    )
    .expect("tools list user page");
    let user_page_text = reply_texts(&user_page);
    assert!(user_page_text
        .iter()
        .any(|value| value.contains(r#""name":"self.device.factory_reset""#)));
    assert!(user_page_text
        .iter()
        .any(|value| value.contains(r#""annotations":{"audience":["user"]}"#)));

    let _ = fs::remove_file(&tools_path);
}

#[test]
fn xiaozhi_mqtt_session_reply_generates_udp_hello_and_mcp_init() {
    let _guard = EnvGuard::set_all_locked(&[
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_SERVER", Some("127.0.0.1")),
        ("SDKWORK_AIOT_XIAOZHI_MQTT_UDP_PORT", Some("8888")),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_KEY_HEX",
            Some("00112233445566778899AABBCCDDEEFF"),
        ),
        (
            "SDKWORK_AIOT_XIAOZHI_MQTT_UDP_NONCE_HEX",
            Some("01000000A1A2A3A40000000000000000"),
        ),
    ]);
    let server = standard_gateway_server().expect("gateway server");

    let (reply, session) = xiaozhi_mqtt_session_reply(
        &server,
        None,
        r#"{"type":"hello","version":3,"transport":"udp","device_id":"dev-001","client_id":"client-001","features":{"mcp":true},"audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
    )
    .expect("mqtt hello reply");

    let session = session.expect("session");
    assert_eq!(session.device_id, "dev-001");
    assert_eq!(session.client_id, "client-001");
    assert_eq!(session.udp_server, "127.0.0.1");
    assert_eq!(session.udp_port, 8888);
    assert_eq!(reply.outbound_json.len(), 2);
    assert!(reply.outbound_json[0].contains(r#""type":"hello""#));
    assert!(reply.outbound_json[0].contains(r#""transport":"udp""#));
    assert!(reply.outbound_json[0].contains(r#""udp":{"server":"127.0.0.1","port":8888"#));
    assert!(reply.outbound_json[1].contains(r#""type":"mcp""#));
    assert!(!reply.close_audio_channel);
}

#[test]
fn xiaozhi_mqtt_session_reply_ignores_goodbye_for_mismatched_session_id() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"other-session","type":"goodbye"}"#,
    )
    .expect("goodbye reply");

    assert!(!reply.close_audio_channel);
    assert!(next_session.is_some());
    assert!(reply.outbound_json.is_empty());
}

#[test]
fn xiaozhi_mqtt_session_reply_listen_includes_udp_audio_downlink() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"listen","state":"start","mode":"manual"}"#,
    )
    .expect("listen reply");

    assert_eq!(reply.outbound_udp_packets.len(), 1);
    assert!(reply.outbound_udp_packets[0].len() >= 16);
    assert_eq!(next_session.as_ref().unwrap().local_sequence, 1);
}

#[test]
fn xiaozhi_mqtt_session_reply_marks_goodbye_as_channel_close() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"goodbye"}"#,
    )
    .expect("goodbye reply");

    assert!(reply.close_audio_channel);
    assert!(next_session.is_none());
    assert!(
        reply.outbound_json.is_empty(),
        "device-initiated MQTT goodbye must not echo goodbye back to avoid ping-pong"
    );
}

#[test]
fn xiaozhi_mqtt_session_reply_with_options_uses_injected_provider() {
    let server = standard_gateway_server().expect("gateway server");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.mqtt_only",
                "MQTT option-scoped provider tool.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ]),
    ));
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = sdkwork_aiot_cloud_gateway::xiaozhi_mqtt_session_reply_with_options(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"2.0","id":901,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
        &options,
    )
    .expect("mcp tools list by mqtt options");

    assert!(next_session.is_some());
    assert!(!reply.close_audio_channel);
    assert!(reply
        .outbound_json
        .iter()
        .any(|value| value.contains(r#""name":"self.custom.mqtt_only""#)));
}

#[test]
fn xiaozhi_mqtt_session_reply_with_options_surfaces_invoker_error() {
    let server = standard_gateway_server().expect("gateway server");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_and_invoker(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.mqtt_invoker_error",
                "MQTT tool for invoker error test.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ErroringToolInvoker),
    );
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = sdkwork_aiot_cloud_gateway::xiaozhi_mqtt_session_reply_with_options(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"2.0","id":902,"method":"tools/call","params":{"name":"self.custom.mqtt_invoker_error","arguments":{}}}}"#,
        &options,
    )
    .expect("mcp tools call by mqtt options");

    assert!(next_session.is_some());
    assert!(!reply.close_audio_channel);
    assert!(reply
        .outbound_json
        .iter()
        .any(|value| value
            .contains(r#""error":{"code":-32601,"message":"invoker rejected tool call"}"#)));
}

#[test]
fn xiaozhi_mqtt_session_reply_with_options_passes_invocation_context() {
    let server = standard_gateway_server().expect("gateway server");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_and_invoker(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.mqtt_context_echo",
                "MQTT context echo tool.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ContextEchoToolInvoker),
    );
    let session = XiaozhiMqttUdpSession {
        device_id: "mqtt-device".to_string(),
        client_id: "mqtt-client".to_string(),
        session_id: "mqtt-device-mqtt-client".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = sdkwork_aiot_cloud_gateway::xiaozhi_mqtt_session_reply_with_options(
        &server,
        Some(&session),
        r#"{"session_id":"mqtt-device-mqtt-client","type":"mcp","payload":{"jsonrpc":"2.0","id":903,"method":"tools/call","params":{"name":"self.custom.mqtt_context_echo","arguments":{}}}}"#,
        &options,
    )
    .expect("mcp context call by mqtt options");

    assert!(next_session.is_some());
    assert!(!reply.close_audio_channel);
    assert!(reply.outbound_json.iter().any(|value| value.contains(
        r#""text":"transport=mqtt session=mqtt-device-mqtt-client device=mqtt-device client=mqtt-client""#
    )));
}

#[test]
fn xiaozhi_mqtt_session_reply_with_options_surfaces_policy_rejection() {
    let server = standard_gateway_server().expect("gateway server");
    let options = XiaozhiSessionOptions::from_mcp_tool_provider_invoker_and_policy(
        Arc::new(DefaultXiaozhiSimulatorMcpToolProvider::from_tools(vec![
            sdkwork_aiot_cloud_gateway::XiaozhiSimulatorMcpToolSpec::new(
                "self.custom.mqtt_policy_denied",
                "MQTT tool for policy deny test.",
                r#"{"type":"object","properties":{},"required":[]}"#,
            ),
        ])),
        Arc::new(ContextEchoToolInvoker),
        Arc::new(DenyAllToolPolicy),
    );
    let session = XiaozhiMqttUdpSession {
        device_id: "deny-mqtt-device".to_string(),
        client_id: "deny-mqtt-client".to_string(),
        session_id: "deny-mqtt-device-deny-mqtt-client".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = sdkwork_aiot_cloud_gateway::xiaozhi_mqtt_session_reply_with_options(
        &server,
        Some(&session),
        r#"{"session_id":"deny-mqtt-device-deny-mqtt-client","type":"mcp","payload":{"jsonrpc":"2.0","id":904,"method":"tools/call","params":{"name":"self.custom.mqtt_policy_denied","arguments":{}}}}"#,
        &options,
    )
    .expect("mcp policy call by mqtt options");

    assert!(next_session.is_some());
    assert!(!reply.close_audio_channel);
    assert!(reply
        .outbound_json
        .iter()
        .any(|value| value
            .contains(r#""error":{"code":-32601,"message":"tool call denied by policy"}"#)));
}

#[test]
fn xiaozhi_mqtt_session_reply_honors_boolean_argument_policy_rules() {
    let _guard = EnvGuard::set_all_locked(&[(
        "SDKWORK_AIOT_XIAOZHI_MCP_POLICY_RULES",
        Some("deny|tool=self.reboot|transport=mqtt|arg_enabled_bool_eq=true"),
    )]);
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "mqtt-deny-device".to_string(),
        client_id: "mqtt-deny-client".to_string(),
        session_id: "mqtt-deny-device-mqtt-deny-client".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"mqtt-deny-device-mqtt-deny-client","type":"mcp","payload":{"jsonrpc":"2.0","id":905,"method":"tools/call","params":{"name":"self.reboot","arguments":{"enabled":true}}}}"#,
    )
    .expect("mcp policy denied by boolean argument");

    assert!(next_session.is_some());
    assert!(!reply.close_audio_channel);
    assert!(reply.outbound_json.iter().any(|value| value.contains(
        r#""error":{"code":-32601,"message":"Tool not allowed by policy: self.reboot"}"#
    )));
}

#[test]
fn xiaozhi_mqtt_session_reply_ignores_mcp_notifications() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "0123456789ABCDEF0123456789ABCDEF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"2.0","method":"notifications/state_changed","params":{"newState":"idle","oldState":"connecting"}}}"#,
    )
    .expect("mcp notification by mqtt");
    assert!(reply.outbound_json.is_empty());
    assert_eq!(next_session, Some(session));
}

#[test]
fn xiaozhi_mqtt_session_reply_ignores_invalid_jsonrpc_mcp_payload() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "0123456789ABCDEF0123456789ABCDEF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"1.0","id":906,"method":"tools/list","params":{"cursor":"","withUserTools":false}}}"#,
    )
    .expect("invalid jsonrpc by mqtt");
    assert!(reply.outbound_json.is_empty());
    assert_eq!(next_session, Some(session));
}

#[test]
fn xiaozhi_mqtt_session_reply_ignores_invalid_params_shape_for_mcp_payload() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "0123456789ABCDEF0123456789ABCDEF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"2.0","id":907,"method":"tools/list","params":"invalid"}}"#,
    )
    .expect("invalid params by mqtt");
    assert!(reply.outbound_json.is_empty());
    assert_eq!(next_session, Some(session));
}

#[test]
fn xiaozhi_mqtt_session_reply_ignores_mcp_payload_without_method_result_or_error() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "0123456789ABCDEF0123456789ABCDEF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, next_session) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"mcp","payload":{"jsonrpc":"2.0","id":908}}"#,
    )
    .expect("payload-only by mqtt");
    assert!(reply.outbound_json.is_empty());
    assert_eq!(next_session, Some(session));
}

#[test]
fn xiaozhi_mqtt_udp_audio_decoder_uses_session_crypto_profile() {
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 6,
        local_sequence: 0,
    };
    let udp_codec = session.udp_codec().expect("udp codec");
    let packet = udp_codec
        .encode_audio_packet(1234, 7, [0x11, 0x22, 0x33])
        .expect("encoded packet");

    let decoded = xiaozhi_mqtt_udp_decode_audio(&session, &packet).expect("decoded packet");
    assert_eq!(decoded.timestamp, 1234);
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.payload, vec![0x11, 0x22, 0x33]);
}

#[test]
fn xiaozhi_mqtt_server_teardown_reply_emits_goodbye_and_closes_channel() {
    let reply = xiaozhi_mqtt_server_teardown_reply("dev-001-client-001");

    assert!(reply.close_audio_channel);
    assert_eq!(reply.outbound_udp_packets.len(), 0);
    assert_eq!(reply.outbound_json.len(), 1);
    assert_eq!(
        reply.outbound_json[0],
        xiaozhi_mqtt_goodbye_message("dev-001-client-001")
    );
}

#[test]
fn xiaozhi_mqtt_udp_uplink_speech_reply_generates_full_speech_cycle() {
    let mut session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let reply =
        xiaozhi_mqtt_udp_uplink_speech_reply(&mut session, &[0u8; 128]).expect("speech reply");

    assert!(!reply.close_audio_channel);
    assert_eq!(reply.outbound_udp_packets.len(), 1);
    assert!(reply.outbound_udp_packets[0].len() >= 16);
    assert_eq!(session.local_sequence, 1);
    assert!(reply
        .outbound_json
        .iter()
        .any(|message| message.contains(r#""type":"stt""#)
            && message.contains("received 128 bytes of opus audio")));
    assert!(reply
        .outbound_json
        .iter()
        .any(|message| message.contains(r#""type":"tts","state":"start""#)));
    assert!(reply
        .outbound_json
        .iter()
        .any(|message| message.contains(r#""type":"tts","state":"stop""#)));
}

#[test]
fn xiaozhi_mqtt_session_reply_listen_detect_uses_wake_word_text() {
    let server = standard_gateway_server().expect("gateway server");
    let session = XiaozhiMqttUdpSession {
        device_id: "dev-001".to_string(),
        client_id: "client-001".to_string(),
        session_id: "dev-001-client-001".to_string(),
        udp_server: "127.0.0.1".to_string(),
        udp_port: 8888,
        udp_key_hex: "00112233445566778899AABBCCDDEEFF".to_string(),
        udp_nonce_hex: "01000000A1A2A3A40000000000000000".to_string(),
        remote_sequence: 0,
        local_sequence: 0,
    };

    let (reply, _) = xiaozhi_mqtt_session_reply(
        &server,
        Some(&session),
        r#"{"session_id":"dev-001-client-001","type":"listen","state":"detect","text":"你好小智"}"#,
    )
    .expect("detect listen reply");

    assert!(reply
        .outbound_json
        .iter()
        .any(|message| message.contains(r#""type":"stt""#) && message.contains("你好小智")));
}

#[test]
fn xiaozhi_websocket_session_listen_detect_uses_wake_word_text() {
    let server = standard_gateway_server().expect("gateway server");
    let request = sdkwork_aiot_transport::parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=dev-001&client_id=browser-001 HTTP/1.1\r\nHost: domain\r\n\r\n",
    )
    .expect("request");

    let replies = xiaozhi_websocket_session_reply(
        &server,
        &request,
        sdkwork_aiot_transport::WebSocketFrame::text(
            r#"{"session_id":"dev-001-browser-001","type":"listen","state":"detect","text":"你好小智"}"#,
        ),
    )
    .expect("detect listen reply");
    let reply_text = reply_texts(&replies);

    assert!(reply_text
        .iter()
        .any(|reply| reply.contains(r#""type":"stt""#) && reply.contains("你好小智")));
}

#[test]
fn xiaozhi_simulator_handler_returns_cross_platform_ui_migration_hint() {
    let request = sdkwork_aiot_transport::HttpRequest::new("GET", "/simulators/xiaozhi");

    let response = xiaozhi_simulator_http_handler(&request);

    assert_eq!(
        response.status,
        sdkwork_aiot_transport::HttpStatus::NotFound
    );
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert!(response
        .body
        .contains("gateway.xiaozhi.simulator.ui.migrated"));
    assert!(response.body.contains("sdkwork-aiot-xiaozhi-simulator-ui"));
}

#[test]
fn gateway_serves_bridge_health_endpoint() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("bridge health tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(b"GET /internal/bridge/health HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("bridge health request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""status":"disabled""#));
    assert!(response.contains(r#""bridge_enabled":false"#));
    assert!(response.contains(r#""stats":{"mqtt_reconnects":"#));
}

#[test]
fn gateway_serves_bridge_sessions_endpoint_when_bridge_is_disabled() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("bridge sessions tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(b"GET /internal/bridge/sessions HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("bridge sessions request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""bridge_enabled":false"#));
    assert!(response.contains(r#""sessions":[]"#));
}

#[test]
fn gateway_bridge_session_disconnect_returns_disabled_when_bridge_is_off() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("bridge disconnect tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(
        b"DELETE /internal/bridge/sessions/dev-001-client-001 HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("bridge disconnect request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 503"));
    assert!(response.contains("gateway.bridge.disabled"));
}

#[test]
fn gateway_bridge_session_disconnect_returns_not_found_when_bridge_is_enabled() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[("SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE", Some("1"))],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("bridge disconnect tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(
        b"DELETE /internal/bridge/sessions/dev-001-client-001 HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("bridge disconnect request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 404"));
    assert!(response.contains("gateway.bridge.session.not_found"));
}

#[test]
fn gateway_serves_bridge_metrics_endpoint_with_activation_registry_metrics() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("bridge metrics tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(b"GET /internal/bridge/metrics HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("bridge metrics request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("content-type: text/plain; version=0.0.4"));
    assert!(response.contains("sdkwork_aiot_bridge_mqtt_reconnects_total"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total"));
    assert!(
        response.contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"unknown\"}")
    );
}

#[test]
fn gateway_serves_xiaozhi_mcp_policy_stats_endpoint() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http = TcpStream::connect(&bind_addr).expect("mcp policy stats tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(b"GET /internal/xiaozhi/mcp-policy/stats HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("mcp policy stats request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""policy":"rule_based""#));
    assert!(response.contains(r#""allow_by_rule_matches":"#));
    assert!(response.contains(r#""allow_no_rule_matches":"#));
    assert!(response.contains(r#""deny_by_rule_matches":"#));
}

#[test]
fn gateway_serves_xiaozhi_activation_registry_stats_endpoint() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http =
        TcpStream::connect(&bind_addr).expect("activation registry stats tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("activation registry stats request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""backend":"#));
    assert!(response.contains(r#""register_total":"#));
    assert!(response.contains(r#""consume_total":"#));
    assert!(response.contains(r#""consume_hits":"#));
    assert!(response.contains(r#""consume_misses":"#));
    assert!(response.contains(r#""pruned_entries":"#));
}

#[test]
fn gateway_serves_xiaozhi_activation_registry_metrics_endpoint() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut http =
        TcpStream::connect(&bind_addr).expect("activation registry metrics tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(
        b"GET /internal/xiaozhi/activation-registry/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("activation registry metrics request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("content-type: text/plain; version=0.0.4"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_total"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_misses_total"));
    assert!(response.contains("sdkwork_aiot_xiaozhi_activation_registry_pruned_entries_total"));
    assert!(
        response.contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"unknown\"}")
    );
}

#[test]
fn gateway_activation_registry_metrics_reflect_in_memory_backend_after_activation_flow() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-metrics-in-memory"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-metrics-in-memory"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-metrics-in-memory"),
            ),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH", None),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND", None),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-im-device\r\nClient-Id: metrics-im-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));
    assert!(ota.contains(r#""challenge":"challenge-metrics-in-memory""#));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-im-device\r\nClient-Id: metrics-im-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-metrics-in-memory\",\"hmac\":\"hmac-metrics-in-memory\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"in_memory""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":1"#));
    assert!(stats.contains(r#""consume_hits":1"#));
    assert!(stats.contains(r#""consume_misses":0"#));

    let metrics = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(metrics
        .contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"in_memory\"} 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total 1"));
}

#[test]
fn gateway_activation_registry_metrics_reflect_file_backend_after_activation_flow() {
    let bind_addr = reserve_local_bind_addr();
    let registry_path = unique_temp_file_path("xiaozhi-activation-registry-metrics-file", "state");
    let registry_path_text = registry_path.to_string_lossy().into_owned();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-metrics-file"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-metrics-file"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-metrics-file"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
                Some(registry_path_text.as_str()),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
                Some("file"),
            ),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-file-device\r\nClient-Id: metrics-file-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));
    assert!(ota.contains(r#""challenge":"challenge-metrics-file""#));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-file-device\r\nClient-Id: metrics-file-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-metrics-file\",\"hmac\":\"hmac-metrics-file\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"file""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":1"#));
    assert!(stats.contains(r#""consume_hits":1"#));
    assert!(stats.contains(r#""consume_misses":0"#));

    let metrics = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(
        metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"file\"} 1")
    );
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total 1"));

    let _ = fs::remove_file(&registry_path);
    let _ = fs::remove_file(format!("{}.lock", registry_path.to_string_lossy()));
}

#[test]
fn gateway_activation_registry_metrics_reflect_sqlite_backend_after_activation_flow() {
    let bind_addr = reserve_local_bind_addr();
    let registry_path = unique_temp_file_path("xiaozhi-activation-registry-metrics-sqlite", "db");
    let registry_path_text = registry_path.to_string_lossy().into_owned();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-metrics-sqlite"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-metrics-sqlite"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-metrics-sqlite"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
                Some(registry_path_text.as_str()),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
                Some("sqlite"),
            ),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-sqlite-device\r\nClient-Id: metrics-sqlite-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));
    assert!(ota.contains(r#""challenge":"challenge-metrics-sqlite""#));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: metrics-sqlite-device\r\nClient-Id: metrics-sqlite-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-metrics-sqlite\",\"hmac\":\"hmac-metrics-sqlite\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"sqlite""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":1"#));
    assert!(stats.contains(r#""consume_hits":1"#));
    assert!(stats.contains(r#""consume_misses":0"#));

    let metrics = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(
        metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"sqlite\"} 1")
    );
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total 1"));

    let _ = fs::remove_file(&registry_path);
    let _ = fs::remove_file(format!("{}-wal", registry_path.to_string_lossy()));
    let _ = fs::remove_file(format!("{}-shm", registry_path.to_string_lossy()));
}

#[test]
fn gateway_bridge_metrics_reflect_activation_registry_backend_after_activation_flow() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-bridge-metrics"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-bridge-metrics"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-bridge-metrics"),
            ),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH", None),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND", None),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: bridge-metrics-device\r\nClient-Id: bridge-metrics-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: bridge-metrics-device\r\nClient-Id: bridge-metrics-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-bridge-metrics\",\"hmac\":\"hmac-bridge-metrics\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let metrics = http_get_or_post(
        &bind_addr,
        b"GET /internal/bridge/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(metrics.contains("sdkwork_aiot_bridge_mqtt_reconnects_total"));
    assert!(metrics
        .contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"in_memory\"} 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_register_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_total 1"));
    assert!(metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_consume_hits_total 1"));
}

#[test]
fn gateway_activation_registry_stats_record_consume_miss_on_replay_attempt() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-replay-metrics"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-replay-metrics"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-replay-metrics"),
            ),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH", None),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND", None),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: replay-device\r\nClient-Id: replay-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));

    let first_activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: replay-device\r\nClient-Id: replay-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-replay-metrics\",\"hmac\":\"hmac-replay-metrics\"}",
    );
    assert!(first_activation.starts_with("HTTP/1.1 200 OK"));

    let replay_activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: replay-device\r\nClient-Id: replay-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-replay-metrics\",\"hmac\":\"hmac-replay-metrics\"}",
    );
    assert!(replay_activation.starts_with("HTTP/1.1 202 Accepted"));
    assert!(replay_activation.contains(r#""activation":{"status":"pending""#));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"in_memory""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":2"#));
    assert!(stats.contains(r#""consume_hits":1"#));
    assert!(stats.contains(r#""consume_misses":1"#));
}

#[test]
fn gateway_redis_registry_kind_without_url_falls_back_to_in_memory_backend() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-redis-fallback"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-redis-fallback"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-redis-fallback"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
                Some("redis"),
            ),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL", None),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH", None),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: redis-fallback-device\r\nClient-Id: redis-fallback-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: redis-fallback-device\r\nClient-Id: redis-fallback-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-redis-fallback\",\"hmac\":\"hmac-redis-fallback\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"in_memory""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":1"#));
    assert!(stats.contains(r#""consume_hits":1"#));
}

#[test]
fn gateway_redis_registry_round_trip_when_test_redis_url_is_configured() {
    let Some(redis_url) = std::env::var("SDKWORK_AIOT_GATEWAY_TEST_REDIS_URL").ok() else {
        return;
    };
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn_with_env(
        &bind_addr,
        &[
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
                Some("challenge-redis-real"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
                Some("challenge-redis-real"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
                Some("hmac-redis-real"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
                Some("redis"),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL",
                Some(redis_url.as_str()),
            ),
            (
                "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_PREFIX",
                Some("sdkwork:test:activation-registry"),
            ),
            ("SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH", None),
        ],
    );

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let ota = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nDevice-Id: redis-real-device\r\nClient-Id: redis-real-client\r\nContent-Type: application/json\r\n\r\n{}",
    );
    assert!(ota.starts_with("HTTP/1.1 200 OK"));
    assert!(ota.contains(r#""challenge":"challenge-redis-real""#));

    let activation = http_get_or_post(
        &bind_addr,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nDevice-Id: redis-real-device\r\nClient-Id: redis-real-client\r\nContent-Type: application/json\r\n\r\n{\"challenge\":\"challenge-redis-real\",\"hmac\":\"hmac-redis-real\"}",
    );
    assert!(activation.starts_with("HTTP/1.1 200 OK"));

    let stats = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/stats HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(stats.starts_with("HTTP/1.1 200 OK"));
    assert!(stats.contains(r#""backend":"redis""#));
    assert!(stats.contains(r#""register_total":1"#));
    assert!(stats.contains(r#""consume_total":1"#));
    assert!(stats.contains(r#""consume_hits":1"#));

    let metrics = http_get_or_post(
        &bind_addr,
        b"GET /internal/xiaozhi/activation-registry/metrics HTTP/1.1\r\nHost: local\r\n\r\n",
    );
    assert!(metrics.starts_with("HTTP/1.1 200 OK"));
    assert!(
        metrics.contains("sdkwork_aiot_xiaozhi_activation_registry_backend{backend=\"redis\"} 1")
    );
}

#[test]
fn gateway_accepts_new_http_requests_while_xiaozhi_websocket_is_open() {
    let bind_addr = reserve_local_bind_addr();
    let _gateway = GatewayProcess::spawn(&bind_addr);

    wait_for_gateway(&bind_addr, Duration::from_secs(10));

    let mut ws = TcpStream::connect(&bind_addr).expect("held websocket tcp connection");
    ws.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("websocket timeout");
    ws.write_all(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=held-device&client_id=held-client HTTP/1.1\r\nHost: local\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
    )
    .expect("websocket handshake write");

    let handshake = read_http_response(&mut ws);
    assert!(handshake.starts_with("HTTP/1.1 101 Switching Protocols"));

    let mut http = TcpStream::connect(&bind_addr).expect("simulator tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(b"GET /simulators/xiaozhi HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("simulator request write");

    let response = read_http_response(&mut http);
    assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(response.contains("gateway.xiaozhi.simulator.ui.migrated"));
}

fn reply_texts(replies: &[WebSocketSessionReply]) -> Vec<&str> {
    replies
        .iter()
        .filter_map(|reply| match reply {
            WebSocketSessionReply::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn reserve_local_bind_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local port");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    addr.to_string()
}

fn http_get_or_post(bind_addr: &str, request: &[u8]) -> String {
    let mut http = TcpStream::connect(bind_addr).expect("gateway tcp connection");
    http.set_read_timeout(Some(Duration::from_secs(2)))
        .expect("http timeout");
    http.write_all(request).expect("gateway request write");
    read_http_response(&mut http)
}

const GATEWAY_SPAWN_ENV_ISOLATION_KEYS: &[&str] = &[
    "SDKWORK_AIOT_XIAOZHI_ACTIVATION_CHALLENGE",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_CHALLENGE",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATE_EXPECTED_HMAC",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATE_STRICT_V2",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATE_AUTO_ACCEPT",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_PATH",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_KIND",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_URL",
    "SDKWORK_AIOT_XIAOZHI_ACTIVATION_REGISTRY_REDIS_PREFIX",
    "SDKWORK_AIOT_DEVICE_DB_PATH",
    "SDKWORK_AIOT_DEVICE_DATABASE_URL",
    "SDKWORK_AIOT_DEVICE_DATABASE_ENGINE",
];

fn spawn_gateway_with_env(bind_addr: &str, env_overrides: &[(&str, Option<&str>)]) -> Child {
    let override_names: HashSet<&str> = env_overrides.iter().map(|(name, _)| *name).collect();
    let mut command = Command::new(gateway_binary());
    command
        .env("SDKWORK_AIOT_EDGE_DEVICE_INGRESS_BIND", bind_addr)
        .env("SDKWORK_AIOT_DEV_MODE", "1")
        .env_remove("SDKWORK_AIOT_GATEWAY_NO_LISTEN")
        .env_remove("SDKWORK_AIOT_GATEWAY_MQTT_BRIDGE_ENABLE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for key in GATEWAY_SPAWN_ENV_ISOLATION_KEYS {
        if !override_names.contains(key) {
            command.env_remove(key);
        }
    }
    for (name, value) in env_overrides {
        match value {
            Some(value) => {
                command.env(name, value);
            }
            None => {
                command.env_remove(name);
            }
        }
    }
    command.spawn().expect("spawn sdkwork-aiot-cloud-gateway")
}

fn spawn_gateway(bind_addr: &str) -> Child {
    spawn_gateway_with_env(bind_addr, &[])
}

struct GatewayProcess {
    child: Child,
}

impl GatewayProcess {
    fn spawn(bind_addr: &str) -> Self {
        Self {
            child: spawn_gateway(bind_addr),
        }
    }

    fn spawn_with_env(bind_addr: &str, env_overrides: &[(&str, Option<&str>)]) -> Self {
        Self {
            child: spawn_gateway_with_env(bind_addr, env_overrides),
        }
    }
}

impl Drop for GatewayProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn gateway_binary() -> String {
    option_env!("CARGO_BIN_EXE_sdkwork-aiot-cloud-gateway")
        .map(str::to_string)
        .unwrap_or_else(|| "target/debug/sdkwork-aiot-cloud-gateway.exe".to_string())
}

fn wait_for_gateway(bind_addr: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(bind_addr).is_ok() {
            return;
        }
        assert!(Instant::now() < deadline, "gateway did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read_http_response(stream: &mut TcpStream) -> String {
    let mut buffer = [0u8; 8192];
    let read = stream.read(&mut buffer).expect("read http response");
    String::from_utf8_lossy(&buffer[..read]).into_owned()
}

fn unique_temp_file_path(prefix: &str, suffix: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = format!("{prefix}-{}-{now}.{suffix}", std::process::id());
    std::env::temp_dir().join(file_name)
}

struct EnvGuard {
    values: Vec<(String, Option<String>)>,
    _lock: Option<std::sync::MutexGuard<'static, ()>>,
}

impl EnvGuard {
    fn set_all_locked(vars: &[(&str, Option<&str>)]) -> Self {
        static LOCK: Mutex<()> = Mutex::new(());
        let lock = LOCK.lock().expect("env lock");
        let mut guard = Self::set_all(vars);
        guard._lock = Some(lock);
        guard
    }

    fn set_all(vars: &[(&str, Option<&str>)]) -> Self {
        let mut values = Vec::with_capacity(vars.len());
        for (name, value) in vars {
            let previous = std::env::var(name).ok();
            values.push(((*name).to_string(), previous));
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        Self {
            values,
            _lock: None,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.values {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

#[test]
fn xiaozhi_device_token_valid_accepts_sqlite_stored_credentials_in_production_mode() {
    let _env = EnvGuard::set_all_locked(&[("SDKWORK_AIOT_DEV_MODE", None)]);

    let repository =
        Arc::new(SqliteSqlxCredentialRepository::new_in_memory().expect("credential repository"));
    let created = repository
        .create_credential(SqliteCredentialCreateCommand {
            association: AiotStorageAssociation::tenant_org(100001, 0),
            device_id: "gateway-device-001".to_string(),
            credential_type: "device-bearer".to_string(),
            expires_at: None,
        })
        .expect("create credential");
    let issued_secret = created
        .issued_secret
        .expect("issued secret returned on create");

    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_env(),
    ))
    .with_device_credential_repository(repository);

    let authorized = parse_http_request_bytes(
        format!(
            "GET /iot/xiaozhi/ws?device_id=gateway-device-001&client_id=client-001 HTTP/1.1\r\nHost: domain\r\nAuthorization: Bearer {issued_secret}\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("authorized request");
    assert!(xiaozhi_device_token_valid(&authorized, &options));

    let wrong_token = parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?device_id=gateway-device-001&client_id=client-001 HTTP/1.1\r\nHost: domain\r\nAuthorization: Bearer wrong-token\r\n\r\n",
    )
    .expect("wrong token request");
    assert!(!xiaozhi_device_token_valid(&wrong_token, &options));

    let missing_device = parse_http_request_bytes(
        format!(
            "GET /iot/xiaozhi/ws?client_id=client-001 HTTP/1.1\r\nHost: domain\r\nAuthorization: Bearer {issued_secret}\r\n\r\n"
        )
        .as_bytes(),
    )
    .expect("missing device request");
    assert!(!xiaozhi_device_token_valid(&missing_device, &options));
}

#[test]
fn rollout_aware_ota_provider_delivers_firmware_once_per_pending_deployment() {
    let store = SqlitePersistedEntityRepository::new_in_memory().expect("repo");
    let association = AiotStorageAssociation::tenant_org(100001, 0);

    store
        .upsert_entity(
            &association,
            sdkwork_aiot_storage_sqlx::ENTITY_FIRMWARE_ARTIFACT,
            "firmware-artifact-0001",
            r#"{"artifactId":"firmware-artifact-0001","version":"2.1.0","resourceJson":"{\"downloadUrl\":\"https://cdn.example.com/fw-2.1.bin\",\"force\":1}"}"#,
        )
        .expect("artifact");

    store
        .upsert_entity(
            &association,
            sdkwork_aiot_storage_sqlx::ENTITY_FIRMWARE_DEPLOYMENT,
            "firmware-deployment-0001",
            r#"{"deploymentId":"firmware-deployment-0001","tenantId":100001,"organizationId":0,"rolloutId":"firmware-rollout-0001","artifactId":"firmware-artifact-0001","deviceId":"rollout-device-001","deploymentState":"pending","force":1}"#,
        )
        .expect("deployment");

    let catalog = FirmwareOtaCatalog::from_repository(store);

    let server = standard_gateway_server_with_plugins(
        Arc::new(RolloutAwareXiaozhiOtaProfileProvider::with_catalog(catalog)),
        Arc::new(DefaultXiaozhiActivationVerifier::stateless()),
    )
    .expect("gateway server");

    let first = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: rollout-device-001\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("first ota response");
    assert!(first.contains(
        r#""firmware":{"version":"2.1.0","url":"https://cdn.example.com/fw-2.1.bin","force":1}"#
    ));

    let second = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: domain\r\nDevice-Id: rollout-device-001\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("second ota response");
    assert!(
        !second.contains(r#""firmware":{"version":"2.1.0""#),
        "offered deployments must not be re-served on subsequent OTA polls"
    );
}

#[test]
fn internal_route_requires_token_when_dev_mode_disabled() {
    let _env = EnvGuard::set_all_locked(&[
        ("SDKWORK_AIOT_DEV_MODE", None),
        (
            "SDKWORK_AIOT_INTERNAL_TOKEN",
            Some("gateway-production-internal-token-32chars"),
        ),
    ]);

    let missing =
        parse_http_request_bytes(b"GET /internal/bridge/health HTTP/1.1\r\nHost: local\r\n\r\n")
            .expect("missing token request");
    assert!(!sdkwork_aiot_cloud_gateway::internal_route_authorized(
        &missing
    ));

    let authorized = parse_http_request_bytes(
        b"GET /internal/bridge/health HTTP/1.1\r\nHost: local\r\nx-sdkwork-internal-token: gateway-production-internal-token-32chars\r\n\r\n",
    )
    .expect("authorized request");
    assert!(sdkwork_aiot_cloud_gateway::internal_route_authorized(
        &authorized
    ));
}

#[test]
fn xiaozhi_device_token_rejects_missing_credential_in_non_dev_mode() {
    let _env = EnvGuard::set_all_locked(&[("SDKWORK_AIOT_DEV_MODE", None)]);

    let options = XiaozhiSessionOptions::from_mcp_tool_provider(Arc::new(
        DefaultXiaozhiSimulatorMcpToolProvider::from_env(),
    ));

    let request = parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?device_id=gateway-device-001&client_id=client-001 HTTP/1.1\r\nHost: domain\r\nAuthorization: Bearer any-token\r\n\r\n",
    )
    .expect("ws request");
    assert!(!xiaozhi_device_token_valid(&request, &options));
}
