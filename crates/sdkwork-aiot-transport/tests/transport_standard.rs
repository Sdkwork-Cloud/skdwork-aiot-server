use sdkwork_aiot_contract::AiotRequestContext;
use sdkwork_aiot_protocol::{
    InboundFrame, MessageClass, MessageCodec, OutboundFrame, ProtocolEnvelope, ProtocolError,
};
use sdkwork_aiot_service_host::AiotProtocolMessageAction;
use sdkwork_aiot_storage::AiotStorageWriteKind;
use sdkwork_aiot_transport::{
    build_health_response, build_websocket_handshake_response, decode_websocket_frame,
    encode_websocket_frame, handle_http_request_bytes, handle_websocket_message_bytes,
    handle_websocket_message_bytes_with_context, parse_http_request_bytes,
    parse_http_request_prefix, websocket_frame_to_inbound_frame, HttpRequest, HttpResponse,
    HttpStatus, TransportServer, WebSocketFrame, WebSocketOpcode,
};

#[derive(Debug, Clone)]
struct FakeCodec {
    envelope: ProtocolEnvelope,
}

impl MessageCodec for FakeCodec {
    fn decode(&self, _frame: InboundFrame) -> Result<ProtocolEnvelope, ProtocolError> {
        Ok(self.envelope.clone())
    }

    fn encode(&self, _envelope: ProtocolEnvelope) -> Result<OutboundFrame, ProtocolError> {
        Ok(OutboundFrame::text("{}"))
    }
}

#[test]
fn health_response_is_plain_http_and_safe_for_load_balancers() {
    let response = build_health_response("sdkwork-aiot-device-edge-runtime", true);

    assert_eq!(response.status, HttpStatus::Ok);
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert!(response.body.contains("\"ready\":true"));
    assert!(response.body.contains("sdkwork-aiot-device-edge-runtime"));
}

#[test]
fn http_status_conflict_maps_to_standard_http_409_semantics() {
    assert_eq!(HttpStatus::Conflict.code(), 409);
    assert_eq!(HttpStatus::Conflict.reason(), "Conflict");
}

#[test]
fn websocket_handshake_response_uses_standard_upgrade_headers() {
    let request = HttpRequest::new("GET", "/iot/xiaozhi/ws")
        .with_header("Host", "localhost")
        .with_header("Upgrade", "websocket")
        .with_header("Connection", "Upgrade")
        .with_header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .with_header("Sec-WebSocket-Version", "13");

    let response = build_websocket_handshake_response(&request).expect("handshake");

    assert_eq!(response.status, HttpStatus::SwitchingProtocols);
    assert_eq!(response.header("upgrade"), Some("websocket"));
    assert_eq!(response.header("connection"), Some("Upgrade"));
    assert_eq!(
        response.header("sec-websocket-accept"),
        Some("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")
    );
}

#[test]
fn websocket_decoder_supports_unmasked_server_side_test_frames() {
    let frame =
        decode_websocket_frame(&[0x81, 0x05, b'h', b'e', b'l', b'l', b'o']).expect("text frame");

    assert_eq!(frame, WebSocketFrame::text("hello"));

    let binary = decode_websocket_frame(&[0x82, 0x02, 0x01, 0x02]).expect("binary frame");
    assert_eq!(binary.opcode, WebSocketOpcode::Binary);
    assert_eq!(binary.payload, vec![1, 2]);

    let close = decode_websocket_frame(&[0x88, 0x00]).expect("close frame");
    assert_eq!(close.opcode, WebSocketOpcode::Close);
}

#[test]
fn http_request_parser_splits_query_from_route_and_decodes_query_pairs() {
    let request = parse_http_request_bytes(
        b"GET /iot/xiaozhi/ws?protocol_version=3&device_id=device-001&client_id=client%20abc HTTP/1.1\r\nHost: local\r\n\r\n",
    )
    .expect("http request");

    assert_eq!(request.path, "/iot/xiaozhi/ws");
    assert_eq!(
        request.raw_path,
        "/iot/xiaozhi/ws?protocol_version=3&device_id=device-001&client_id=client%20abc"
    );
    assert_eq!(request.query_param("protocol_version"), Some("3"));
    assert_eq!(request.query_param("device_id"), Some("device-001"));
    assert_eq!(request.query_param("client_id"), Some("client abc"));
}

#[test]
fn http_request_parser_reports_header_length_and_preserves_trailing_body_bytes() {
    let header = b"GET /iot/xiaozhi/ws?protocol_version=3 HTTP/1.1\r\nHost: local\r\n\r\n";
    let mut bytes = header.to_vec();
    bytes.extend_from_slice(&[0x82, 0x02, 0xff, 0x00]);

    let (request, used) = parse_http_request_prefix(&bytes).expect("http request prefix");

    assert_eq!(request.path, "/iot/xiaozhi/ws");
    assert_eq!(request.query_param("protocol_version"), Some("3"));
    assert_eq!(used, header.len());
    assert_eq!(request.body, vec![0x82, 0x02, 0xff, 0x00]);

    let request = parse_http_request_bytes(&bytes).expect("http request bytes");
    assert_eq!(request.path, "/iot/xiaozhi/ws");
    assert_eq!(request.body, vec![0x82, 0x02, 0xff, 0x00]);
}

#[test]
fn websocket_encoder_writes_unmasked_server_frames_for_browser_clients() {
    let text = encode_websocket_frame(&WebSocketFrame::text(r#"{"type":"hello"}"#));
    assert_eq!(text[0], 0x81);
    assert_eq!(text[1] & 0x80, 0);
    let decoded = decode_websocket_frame(&text).expect("encoded text frame");
    assert_eq!(decoded, WebSocketFrame::text(r#"{"type":"hello"}"#));

    let binary = encode_websocket_frame(&WebSocketFrame {
        opcode: WebSocketOpcode::Binary,
        payload: vec![0x01; 130],
    });
    assert_eq!(binary[0], 0x82);
    assert_eq!(binary[1], 126);
    let decoded = decode_websocket_frame(&binary).expect("encoded binary frame");
    assert_eq!(decoded.opcode, WebSocketOpcode::Binary);
    assert_eq!(decoded.payload.len(), 130);
}

#[test]
fn websocket_decoder_reports_complete_frame_length_for_streaming_sessions() {
    let first = encode_websocket_frame(&WebSocketFrame::text(r#"{"type":"hello"}"#));
    let second = encode_websocket_frame(&WebSocketFrame::text(r#"{"type":"listen"}"#));
    let mut combined = first.clone();
    combined.extend_from_slice(&second);

    let (frame, used) =
        sdkwork_aiot_transport::decode_websocket_frame_prefix(&combined).expect("first frame");
    assert_eq!(frame, WebSocketFrame::text(r#"{"type":"hello"}"#));
    assert_eq!(used, first.len());

    let (frame, used) =
        sdkwork_aiot_transport::decode_websocket_frame_prefix(&combined[first.len()..])
            .expect("second frame");
    assert_eq!(frame, WebSocketFrame::text(r#"{"type":"listen"}"#));
    assert_eq!(used, second.len());

    let partial = &first[..first.len() - 1];
    let error = sdkwork_aiot_transport::decode_websocket_frame_prefix(partial)
        .expect_err("partial frame must not parse");
    assert_eq!(error.code, "transport.websocket.incomplete_frame");
}

#[test]
fn transport_server_can_be_composed_from_runtime_bundle_without_binding_ports() {
    let server = TransportServer::standard_standalone().expect("server");

    assert!(server.runtime.supports_protocol("xiaozhi.websocket"));
    assert!(server
        .listeners
        .websocket_routes
        .contains(&"/iot/xiaozhi/ws"));
    assert!(server.health.ready);
}

#[test]
fn transport_server_resolves_protocol_route_metadata_from_runtime_registry() {
    let server = TransportServer::standard_standalone().expect("server");

    let route = server
        .protocol_route_for_path("/iot/xiaozhi/ws")
        .expect("xiaozhi route");

    assert_eq!(route.protocol_id, "xiaozhi.websocket");
    assert_eq!(route.plugin_id, "xiaozhi");
    assert_eq!(route.path, "/iot/xiaozhi/ws");
}

#[test]
fn websocket_frames_convert_to_transport_neutral_inbound_frames() {
    let text = websocket_frame_to_inbound_frame(WebSocketFrame::text(r#"{"type":"hello"}"#));
    assert!(!text.binary);
    assert_eq!(text.payload, br#"{"type":"hello"}"#);

    let binary = websocket_frame_to_inbound_frame(WebSocketFrame {
        opcode: WebSocketOpcode::Binary,
        payload: vec![0x01, 0x02, 0x03],
    });
    assert!(binary.binary);
    assert_eq!(binary.payload, vec![0x01, 0x02, 0x03]);
}

#[test]
fn transport_server_handles_health_ready_and_websocket_upgrade_requests() {
    let server = TransportServer::standard_standalone().expect("server");

    let health =
        handle_http_request_bytes(&server, b"GET /healthz HTTP/1.1\r\nHost: local\r\n\r\n")
            .expect("health");
    assert!(health.starts_with("HTTP/1.1 200"));
    assert!(health.contains("\"ready\":true"));

    let ready = handle_http_request_bytes(&server, b"GET /readyz HTTP/1.1\r\nHost: local\r\n\r\n")
        .expect("ready");
    assert!(ready.starts_with("HTTP/1.1 200"));

    let upgrade = handle_http_request_bytes(
        &server,
        b"GET /iot/xiaozhi/ws HTTP/1.1\r\nHost: local\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
    )
    .expect("upgrade");
    assert!(upgrade.starts_with("HTTP/1.1 101"));
    assert!(upgrade.contains("sec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
}

#[test]
fn transport_server_mounts_compatibility_http_routes_without_protocol_specific_dependency() {
    let server = TransportServer::standard_standalone()
        .expect("server")
        .with_http_compatibility_route("/iot/xiaozhi/ota", |request| {
            assert_eq!(request.method, "POST");
            assert_eq!(request.body, b"{}".to_vec());
            HttpResponse::new(HttpStatus::Ok)
                .with_header("content-type", "application/json")
                .with_body(r#"{"websocket":{"url":"wss://domain/iot/xiaozhi/ws","token":"device-token","version":3}}"#)
        })
        .with_http_compatibility_route("/iot/xiaozhi/activate", |request| {
            assert_eq!(request.body, b"{}".to_vec());
            HttpResponse::new(HttpStatus::Accepted)
                .with_header("content-type", "application/json")
                .with_body(r#"{"activation":{"status":"pending"}}"#)
        });

    let ota = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/ota HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("ota response");
    let activate = handle_http_request_bytes(
        &server,
        b"POST /iot/xiaozhi/activate HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\n\r\n{}",
    )
    .expect("activation response");

    assert!(ota.starts_with("HTTP/1.1 200 OK"));
    assert!(ota.contains(r#""websocket":{"url":"wss://domain/iot/xiaozhi/ws""#));
    assert!(activate.starts_with("HTTP/1.1 202 Accepted"));
    assert!(activate.contains(r#""activation":{"status":"pending"}"#));
}

#[test]
fn transport_websocket_message_handler_uses_runtime_pipeline_without_protocol_specific_dependency()
{
    let server = TransportServer::standard_standalone().expect("server");
    let codec = FakeCodec {
        envelope: ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
            .device("device-001")
            .session("session-001")
            .semantic_type("hello")
            .build(),
    };
    let frame_bytes = [
        0x81, 0x10, b'{', b'"', b't', b'y', b'p', b'e', b'"', b':', b'"', b'h', b'e', b'l', b'l',
        b'o', b'"', b'}',
    ];

    let result = handle_websocket_message_bytes(&server, "/iot/xiaozhi/ws", &codec, &frame_bytes)
        .expect("pipeline result");

    assert_eq!(
        result.message.action,
        AiotProtocolMessageAction::OpenSession
    );
    assert_eq!(
        result.storage_command.kind,
        AiotStorageWriteKind::OpenSession
    );
    assert_eq!(result.storage_command.primary_table, "iot_device_session");
}

#[test]
fn transport_websocket_message_handler_can_pass_resolved_appbase_context_to_runtime_pipeline() {
    let server = TransportServer::standard_standalone().expect("server");
    let codec = FakeCodec {
        envelope: ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
            .device("device-001")
            .session("session-001")
            .semantic_type("hello")
            .build(),
    };
    let ctx = AiotRequestContext::new("100001", "0")
        .with_user("30001")
        .with_data_scope("7");
    let frame_bytes = [
        0x81, 0x10, b'{', b'"', b't', b'y', b'p', b'e', b'"', b':', b'"', b'h', b'e', b'l', b'l',
        b'o', b'"', b'}',
    ];

    let result = handle_websocket_message_bytes_with_context(
        &server,
        "/iot/xiaozhi/ws",
        &ctx,
        &codec,
        &frame_bytes,
    )
    .expect("pipeline result");

    assert_eq!(result.storage_command.association.tenant_id, 100001);
    assert_eq!(result.storage_command.association.organization_id, 0);
    assert_eq!(result.storage_command.association.user_id, Some(30001));
    assert_eq!(result.storage_command.association.data_scope, 7);
}
