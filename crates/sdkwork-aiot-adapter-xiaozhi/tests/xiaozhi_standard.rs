use sdkwork_aiot_adapter_xiaozhi::{
    map_xiaozhi_message_class, resolve_xiaozhi_message_class, xiaozhi_activation_pending_response,
    xiaozhi_handshake_context, xiaozhi_manifest, xiaozhi_ota_response, xiaozhi_routes,
    xiaozhi_server_hello_response, XiaozhiAudioParams, XiaozhiMqttCodec, XiaozhiOtaMetadata,
    XiaozhiServerHello, XiaozhiUdpAudioCodec, XiaozhiWebSocketCodec, AUTHORIZATION_HEADER,
    CLIENT_ID_HEADER, DEVICE_ID_HEADER, PROTOCOL_VERSION_HEADER, XIAOZHI_ACTIVATE_PATH,
    XIAOZHI_BASE_PATH, XIAOZHI_MQTT_UDP_PROTOCOL_ID, XIAOZHI_OTA_PATH,
    XIAOZHI_WEBSOCKET_PROTOCOL_ID, XIAOZHI_WS_PATH,
};
use sdkwork_aiot_protocol::{
    CodecKind, InboundFrame, MessageClass, MessageCodec, ProtocolEnvelope, ProtocolPluginScope,
    SessionPolicy, TransportBinding,
};
use sdkwork_aiot_security::DeviceAuthMode;
use sdkwork_aiot_service_host::{standard_aiot_runtime, AiotProtocolMessageAction, RuntimeMode};

#[test]
fn xiaozhi_manifest_declares_plugin_not_core_protocol() {
    let manifest = xiaozhi_manifest();

    assert_eq!(manifest.plugin_id, "xiaozhi");
    assert_eq!(manifest.scope, ProtocolPluginScope::CompatibilityPlugin);
    assert!(manifest
        .protocol_ids
        .contains(&"xiaozhi.websocket".to_string()));
    assert!(manifest.transports.contains(&TransportBinding::WebSocket));
    assert!(manifest.codecs.contains(&CodecKind::JsonText));
    assert!(manifest.codecs.contains(&CodecKind::BinaryMedia));
    assert!(manifest
        .session_policies
        .contains(&SessionPolicy::StatefulDeviceSession));
    assert!(manifest
        .capability_bridges
        .contains(&"mcp_jsonrpc".to_string()));
    assert!(manifest
        .security_modes
        .contains(&DeviceAuthMode::BearerToken.manifest_name().to_string()));
    assert!(manifest
        .security_modes
        .contains(&DeviceAuthMode::Hmac.manifest_name().to_string()));
    assert!(manifest.hardware_families.contains(&"esp32".to_string()));
    assert!(manifest.runtime_profiles.contains(&"esp_idf".to_string()));
    assert!(manifest
        .firmware_profiles
        .contains(&"xiaozhi_ota".to_string()));
}

#[test]
fn xiaozhi_routes_and_headers_are_compatibility_surface() {
    assert_eq!(XIAOZHI_BASE_PATH, "/iot/xiaozhi");
    assert_eq!(XIAOZHI_WS_PATH, "/iot/xiaozhi/ws");
    assert_eq!(XIAOZHI_OTA_PATH, "/iot/xiaozhi/ota");
    assert_eq!(PROTOCOL_VERSION_HEADER, "Protocol-Version");
    assert_eq!(DEVICE_ID_HEADER, "Device-Id");
    assert_eq!(CLIENT_ID_HEADER, "Client-Id");
}

#[test]
fn xiaozhi_routes_are_mountable_without_becoming_core_routes() {
    let routes = xiaozhi_routes();

    assert!(routes.contains(&XIAOZHI_WS_PATH));
    assert!(routes.contains(&XIAOZHI_OTA_PATH));
    assert!(routes.contains(&XIAOZHI_ACTIVATE_PATH));
    assert!(routes.contains(&sdkwork_aiot_adapter_xiaozhi::XIAOZHI_MQTT_PATH));
    assert!(routes.contains(&sdkwork_aiot_adapter_xiaozhi::XIAOZHI_UDP_PATH));
    assert!(routes
        .iter()
        .all(|route| route.starts_with(XIAOZHI_BASE_PATH)));
}

#[test]
fn xiaozhi_message_names_map_to_standard_message_classes() {
    assert_eq!(
        map_xiaozhi_message_class("hello"),
        Some(MessageClass::Handshake)
    );
    assert_eq!(
        map_xiaozhi_message_class("listen"),
        Some(MessageClass::Event)
    );
    assert_eq!(
        map_xiaozhi_message_class("iot"),
        Some(MessageClass::PropertyReport)
    );
    assert_eq!(
        map_xiaozhi_message_class("mcp"),
        Some(MessageClass::CommandRequest)
    );
    assert_eq!(
        map_xiaozhi_message_class("firmware"),
        Some(MessageClass::OtaCheck)
    );
    assert_eq!(
        map_xiaozhi_message_class("abort"),
        Some(MessageClass::CommandRequest)
    );
    assert_eq!(map_xiaozhi_message_class("tts"), Some(MessageClass::Event));
    assert_eq!(
        map_xiaozhi_message_class("system"),
        Some(MessageClass::CommandRequest)
    );
    assert_eq!(map_xiaozhi_message_class("unknown"), None);
}

#[test]
fn resolve_xiaozhi_message_class_maps_firmware_completion_reports_to_ota_deploy() {
    assert_eq!(
        map_xiaozhi_message_class("firmware_complete"),
        Some(MessageClass::OtaDeploy)
    );
    assert_eq!(
        resolve_xiaozhi_message_class(
            "firmware",
            r#"{"type":"firmware","state":"success","version":"2.1.0"}"#
        ),
        Some(MessageClass::OtaDeploy)
    );
    assert_eq!(
        resolve_xiaozhi_message_class(
            "ota",
            r#"{"type":"ota","status":"completed","version":"2.1.0"}"#
        ),
        Some(MessageClass::OtaDeploy)
    );
    assert_eq!(
        resolve_xiaozhi_message_class(
            "firmware",
            r#"{"type":"firmware","state":"checking","version":"2.1.0"}"#
        ),
        Some(MessageClass::OtaCheck)
    );
}

#[test]
fn xiaozhi_handshake_context_maps_headers_without_owning_iam() {
    let ctx = xiaozhi_handshake_context([
        (AUTHORIZATION_HEADER, "Bearer device-token"),
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]);

    assert_eq!(ctx.transport, TransportBinding::WebSocket);
    assert_eq!(ctx.path.as_deref(), Some(XIAOZHI_WS_PATH));
    assert_eq!(ctx.header(PROTOCOL_VERSION_HEADER), Some("3"));
    assert_eq!(ctx.header(DEVICE_ID_HEADER), Some("device-001"));
    assert_eq!(ctx.header(CLIENT_ID_HEADER), Some("client-abc"));
}

#[test]
fn xiaozhi_websocket_codec_decodes_hello_into_standard_envelope() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));

    let envelope = codec
        .decode(InboundFrame::text(
            r#"{"type":"hello","version":3,"features":{"mcp":true,"aec":true},"transport":"websocket","audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
        ))
        .expect("hello envelope");

    assert_eq!(envelope.protocol_id, XIAOZHI_WEBSOCKET_PROTOCOL_ID);
    assert_eq!(envelope.protocol_version.as_deref(), Some("3"));
    assert_eq!(envelope.message_class, MessageClass::Handshake);
    assert_eq!(envelope.semantic_type, "hello");
    assert_eq!(envelope.content_type, "application/json");
    assert_eq!(envelope.payload_encoding, "utf8");
    assert_eq!(envelope.device_id.as_deref(), Some("device-001"));
    assert_eq!(envelope.client_id.as_deref(), Some("client-abc"));
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.transport")
            .map(String::as_str),
        Some("websocket")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.feature.mcp")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.audio.format")
            .map(String::as_str),
        Some("opus")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.audio.sample_rate")
            .map(String::as_str),
        Some("16000")
    );
}

#[test]
fn xiaozhi_server_hello_response_matches_esp32_websocket_expectations() {
    let response = xiaozhi_server_hello_response(
        XiaozhiServerHello::websocket("session-001")
            .with_audio_params(XiaozhiAudioParams::opus(24000, 1, 60)),
    );

    assert_eq!(
        response,
        r#"{"type":"hello","transport":"websocket","session_id":"session-001","audio_params":{"format":"opus","sample_rate":24000,"channels":1,"frame_duration":60}}"#
    );
}

#[test]
fn xiaozhi_server_hello_response_supports_mqtt_udp_profile() {
    let response = xiaozhi_server_hello_response(
        XiaozhiServerHello::mqtt_udp(
            "session-002",
            "192.168.1.100",
            8888,
            "0123456789ABCDEF0123456789ABCDEF",
            "01000000A1A2A3A40000000000000000",
        )
        .with_audio_params(XiaozhiAudioParams::opus(24_000, 1, 60)),
    );

    assert_eq!(
        response,
        r#"{"type":"hello","transport":"udp","session_id":"session-002","audio_params":{"format":"opus","sample_rate":24000,"channels":1,"frame_duration":60},"udp":{"server":"192.168.1.100","port":8888,"key":"0123456789ABCDEF0123456789ABCDEF","nonce":"01000000A1A2A3A40000000000000000"}}"#
    );
}

#[test]
fn xiaozhi_websocket_codec_decodes_binary_protocol_v1_audio_as_media_frame() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "1"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));

    let envelope = codec
        .decode(InboundFrame::binary([0x01, 0x02, 0x03, 0x04]))
        .expect("audio envelope");

    assert_eq!(envelope.protocol_id, XIAOZHI_WEBSOCKET_PROTOCOL_ID);
    assert_eq!(envelope.message_class, MessageClass::MediaFrame);
    assert_eq!(envelope.semantic_type, "audio");
    assert_eq!(envelope.content_type, "application/octet-stream");
    assert_eq!(envelope.payload_encoding, "binary");
    assert_eq!(envelope.payload, vec![1, 2, 3, 4]);
    assert_eq!(envelope.device_id.as_deref(), Some("device-001"));
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.protocol_version")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.message_type")
            .map(String::as_str),
        Some("opus")
    );
}

#[test]
fn xiaozhi_websocket_codec_decodes_binary_protocol_v2_audio_header() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "2"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let mut frame = Vec::new();
    frame.extend_from_slice(&2u16.to_be_bytes());
    frame.extend_from_slice(&0u16.to_be_bytes());
    frame.extend_from_slice(&0u32.to_be_bytes());
    frame.extend_from_slice(&42u32.to_be_bytes());
    frame.extend_from_slice(&3u32.to_be_bytes());
    frame.extend_from_slice(&[0x0a, 0x0b, 0x0c]);

    let envelope = codec
        .decode(InboundFrame::binary(frame))
        .expect("v2 audio envelope");

    assert_eq!(envelope.message_class, MessageClass::MediaFrame);
    assert_eq!(envelope.semantic_type, "audio");
    assert_eq!(envelope.payload, vec![0x0a, 0x0b, 0x0c]);
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.protocol_version")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.message_type")
            .map(String::as_str),
        Some("opus")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.audio.timestamp_ms")
            .map(String::as_str),
        Some("42")
    );
}

#[test]
fn xiaozhi_websocket_codec_decodes_binary_protocol_v3_audio_header() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let mut frame = Vec::new();
    frame.push(0);
    frame.push(0);
    frame.extend_from_slice(&3u16.to_be_bytes());
    frame.extend_from_slice(&[0x0d, 0x0e, 0x0f]);

    let envelope = codec
        .decode(InboundFrame::binary(frame))
        .expect("v3 audio envelope");

    assert_eq!(envelope.message_class, MessageClass::MediaFrame);
    assert_eq!(envelope.payload, vec![0x0d, 0x0e, 0x0f]);
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.protocol_version")
            .map(String::as_str),
        Some("3")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.binary.message_type")
            .map(String::as_str),
        Some("opus")
    );
}

#[test]
fn xiaozhi_websocket_codec_encodes_binary_protocol_v2_audio_for_device_playback() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "2"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));
    let envelope =
        ProtocolEnvelope::builder(XIAOZHI_WEBSOCKET_PROTOCOL_ID, MessageClass::MediaFrame)
            .semantic_type("audio")
            .binary_payload([0x21, 0x22, 0x23])
            .extension("xiaozhi.audio.timestamp_ms", "99")
            .build();

    let frame = codec.encode(envelope).expect("encoded v2 audio frame");

    assert!(frame.binary);
    assert_eq!(&frame.payload[0..2], &2u16.to_be_bytes());
    assert_eq!(&frame.payload[2..4], &0u16.to_be_bytes());
    assert_eq!(&frame.payload[8..12], &99u32.to_be_bytes());
    assert_eq!(&frame.payload[12..16], &3u32.to_be_bytes());
    assert_eq!(&frame.payload[16..], &[0x21, 0x22, 0x23]);
}

#[test]
fn xiaozhi_websocket_codec_encodes_binary_protocol_v3_audio_for_device_playback() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));
    let envelope =
        ProtocolEnvelope::builder(XIAOZHI_WEBSOCKET_PROTOCOL_ID, MessageClass::MediaFrame)
            .semantic_type("audio")
            .binary_payload([0x31, 0x32, 0x33])
            .build();

    let frame = codec.encode(envelope).expect("encoded v3 audio frame");

    assert!(frame.binary);
    assert_eq!(frame.payload[0], 0);
    assert_eq!(frame.payload[1], 0);
    assert_eq!(&frame.payload[2..4], &3u16.to_be_bytes());
    assert_eq!(&frame.payload[4..], &[0x31, 0x32, 0x33]);
}

#[test]
fn xiaozhi_websocket_codec_rejects_truncated_binary_protocol_v3_frames() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let error = codec
        .decode(InboundFrame::binary([0, 0, 0, 4, 0x01]))
        .expect_err("truncated v3 frame must fail");

    assert_eq!(error.code, "xiaozhi.binary.payload_size_mismatch");
}

#[test]
fn xiaozhi_mcp_jsonrpc_frame_preserves_method_id_and_correlation() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let envelope = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"mcp","payload":{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"self.light.set_rgb"}}}"#,
        ))
        .expect("mcp envelope");

    assert_eq!(envelope.message_class, MessageClass::CommandRequest);
    assert_eq!(envelope.correlation_id.as_deref(), Some("1"));
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.mcp.jsonrpc")
            .map(String::as_str),
        Some("2.0")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.mcp.method")
            .map(String::as_str),
        Some("tools/call")
    );
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.mcp.kind")
            .map(String::as_str),
        Some("request")
    );
}

#[test]
fn xiaozhi_mcp_response_error_and_notification_are_classified_for_server_client_flow() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let response = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"mcp","payload":{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}}"#,
        ))
        .expect("mcp response");
    let error = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"mcp","payload":{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"Unknown tool"}}}"#,
        ))
        .expect("mcp error");
    let notification = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"mcp","payload":{"jsonrpc":"2.0","method":"notifications/state_changed","params":{"newState":"idle"}}}"#,
        ))
        .expect("mcp notification");

    assert_eq!(
        response
            .extensions
            .get("xiaozhi.mcp.kind")
            .map(String::as_str),
        Some("response")
    );
    assert_eq!(response.correlation_id.as_deref(), Some("2"));
    assert_eq!(
        error.extensions.get("xiaozhi.mcp.kind").map(String::as_str),
        Some("error")
    );
    assert_eq!(error.correlation_id.as_deref(), Some("3"));
    assert_eq!(
        notification
            .extensions
            .get("xiaozhi.mcp.kind")
            .map(String::as_str),
        Some("notification")
    );
    assert_eq!(
        notification
            .extensions
            .get("xiaozhi.mcp.method")
            .map(String::as_str),
        Some("notifications/state_changed")
    );
}

#[test]
fn xiaozhi_mcp_string_id_preserves_correlation_and_json_literal() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let request = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"mcp","payload":{"jsonrpc":"2.0","id":"call-001","method":"tools/call","params":{"name":"self.light.set_rgb"}}}"#,
        ))
        .expect("mcp request");

    assert_eq!(request.correlation_id.as_deref(), Some("call-001"));
    assert_eq!(
        request.extensions.get("xiaozhi.mcp.id").map(String::as_str),
        Some("call-001")
    );
    assert_eq!(
        request
            .extensions
            .get("xiaozhi.mcp.id_json")
            .map(String::as_str),
        Some(r#""call-001""#)
    );
    assert_eq!(
        request
            .extensions
            .get("xiaozhi.mcp.kind")
            .map(String::as_str),
        Some("request")
    );
}

#[test]
fn xiaozhi_listen_and_abort_preserve_session_control_semantics() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let listen = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"listen","state":"detect","mode":"manual","text":"hi"}"#,
        ))
        .expect("listen envelope");
    let abort = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"abort","reason":"wake_word_detected"}"#,
        ))
        .expect("abort envelope");

    assert_eq!(listen.message_class, MessageClass::Event);
    assert_eq!(
        listen
            .extensions
            .get("xiaozhi.listen.state")
            .map(String::as_str),
        Some("detect")
    );
    assert_eq!(
        listen
            .extensions
            .get("xiaozhi.listen.mode")
            .map(String::as_str),
        Some("manual")
    );
    assert_eq!(abort.message_class, MessageClass::CommandRequest);
    assert_eq!(
        abort
            .extensions
            .get("xiaozhi.abort.reason")
            .map(String::as_str),
        Some("wake_word_detected")
    );
}

#[test]
fn xiaozhi_server_to_device_state_messages_preserve_display_and_system_fields() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));

    let alert = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"alert","status":"Warning","message":"Battery low","emotion":"sad"}"#,
        ))
        .expect("alert envelope");
    let custom = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"custom","payload":{"message":"anything"}}"#,
        ))
        .expect("custom envelope");
    let goodbye = codec
        .decode(InboundFrame::text(
            r#"{"session_id":"session-001","type":"goodbye"}"#,
        ))
        .expect("goodbye envelope");

    assert_eq!(alert.message_class, MessageClass::Event);
    assert_eq!(alert.semantic_type, "alert");
    assert_eq!(
        alert
            .extensions
            .get("xiaozhi.alert.status")
            .map(String::as_str),
        Some("Warning")
    );
    assert_eq!(
        alert
            .extensions
            .get("xiaozhi.alert.message")
            .map(String::as_str),
        Some("Battery low")
    );
    assert_eq!(
        alert
            .extensions
            .get("xiaozhi.alert.emotion")
            .map(String::as_str),
        Some("sad")
    );
    assert_eq!(custom.message_class, MessageClass::Event);
    assert_eq!(custom.semantic_type, "custom");
    assert_eq!(goodbye.message_class, MessageClass::Disconnect);
}

#[test]
fn xiaozhi_binary_json_frames_decode_through_json_message_pipeline() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
    ]));
    let json = br#"{"session_id":"session-001","type":"listen","state":"stop"}"#;
    let mut frame = Vec::new();
    frame.push(1);
    frame.push(0);
    frame.extend_from_slice(&(json.len() as u16).to_be_bytes());
    frame.extend_from_slice(json);

    let envelope = codec
        .decode(InboundFrame::binary(frame))
        .expect("binary json envelope");

    assert_eq!(envelope.message_class, MessageClass::Event);
    assert_eq!(envelope.semantic_type, "listen");
    assert_eq!(envelope.content_type, "application/json");
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.listen.state")
            .map(String::as_str),
        Some("stop")
    );
}

#[test]
fn xiaozhi_websocket_codec_rejects_unknown_message_type() {
    let codec = XiaozhiWebSocketCodec::new();

    let error = codec
        .decode(InboundFrame::text(r#"{"type":"not-standard"}"#))
        .expect_err("unknown message type must fail");

    assert_eq!(error.code, "xiaozhi.message_type.unsupported");
}

#[test]
fn xiaozhi_ota_response_matches_firmware_activation_and_connection_shape() {
    let body = xiaozhi_ota_response(
        XiaozhiOtaMetadata::new()
            .with_websocket("wss://domain/iot/xiaozhi/ws", "device-token", 3)
            .with_firmware("1.2.3", "https://cdn.example.com/fw.bin", true)
            .with_activation_challenge("Bind this device", "challenge-001", 30000)
            .with_server_time(1_717_171_717_000, 480),
    );

    assert!(body.contains(
        r#""websocket":{"url":"wss://domain/iot/xiaozhi/ws","token":"device-token","version":3}"#
    ));
    assert!(body.contains(
        r#""firmware":{"version":"1.2.3","url":"https://cdn.example.com/fw.bin","force":1}"#
    ));
    assert!(body.contains(r#""activation":{"message":"Bind this device","challenge":"challenge-001","timeout_ms":30000}"#));
    assert!(body.contains(r#""server_time":{"timestamp":1717171717000,"timezone_offset":480}"#));
}

#[test]
fn xiaozhi_ota_response_can_deliver_mqtt_udp_settings_from_external_protocol() {
    let body = xiaozhi_ota_response(
        XiaozhiOtaMetadata::new()
            .with_mqtt(
                "mqtts://broker.example.com:8883",
                "client-001",
                "device-user",
                "device-pass",
                "devices/client-001/up",
                "devices/client-001/down",
                240,
            )
            .with_mqtt_udp(
                "192.168.1.100",
                8888,
                "0123456789ABCDEF0123456789ABCDEF",
                "01000000A1A2A3A40000000000000000",
            ),
    );

    assert!(body.contains(r#""mqtt":{"endpoint":"mqtts://broker.example.com:8883""#));
    assert!(body.contains(r#""client_id":"client-001""#));
    assert!(body.contains(r#""username":"device-user""#));
    assert!(body.contains(r#""password":"device-pass""#));
    assert!(body.contains(r#""publish_topic":"devices/client-001/up""#));
    assert!(body.contains(r#""subscribe_topic":"devices/client-001/down""#));
    assert!(body.contains(r#""keepalive":240"#));
    assert!(body.contains(
        r#""udp":{"server":"192.168.1.100","port":8888,"key":"0123456789ABCDEF0123456789ABCDEF","nonce":"01000000A1A2A3A40000000000000000"}"#
    ));
}

#[test]
fn xiaozhi_message_types_cover_external_esp32_main_branch() {
    // Keep aligned with external/xiaozhi-esp32 main/protocols + application.cc handlers.
    for message_type in [
        "hello", "listen", "abort", "mcp", "goodbye", "stt", "tts", "llm", "system", "alert",
        "custom", "audio", "iot", // legacy firmware; still decoded for compatibility
    ] {
        assert!(
            map_xiaozhi_message_class(message_type).is_some(),
            "missing xiaozhi message type mapping for {message_type}"
        );
    }
}

#[test]
fn xiaozhi_activation_pending_response_keeps_esp32_polling_semantics() {
    let body = xiaozhi_activation_pending_response("activation pending");

    assert_eq!(
        body,
        r#"{"activation":{"status":"pending","message":"activation pending"}}"#
    );
}

#[test]
fn xiaozhi_codec_output_flows_into_runtime_without_runtime_knowing_xiaozhi_payloads() {
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");

    let envelope = codec
        .decode(InboundFrame::text(r#"{"type":"hello","version":3}"#))
        .expect("hello envelope");
    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime protocol result");

    assert_eq!(result.action, AiotProtocolMessageAction::OpenSession);
    assert_eq!(result.pipeline, "device_session");
    assert_eq!(result.protocol_id, XIAOZHI_WEBSOCKET_PROTOCOL_ID);
    assert_eq!(result.plugin_id, "xiaozhi");
    assert_eq!(result.device_id.as_deref(), Some("device-001"));
}

#[test]
fn xiaozhi_mqtt_codec_decodes_udp_hello_message_into_mqtt_udp_protocol_envelope() {
    let codec = XiaozhiMqttCodec::new().with_device_and_client("device-001", "client-001");

    let envelope = codec
        .decode(InboundFrame::text(
            r#"{"type":"hello","version":3,"transport":"udp","features":{"mcp":true,"aec":true},"audio_params":{"format":"opus","sample_rate":16000,"channels":1,"frame_duration":60}}"#,
        ))
        .expect("mqtt hello envelope");

    assert_eq!(envelope.protocol_id, XIAOZHI_MQTT_UDP_PROTOCOL_ID);
    assert_eq!(envelope.message_class, MessageClass::Handshake);
    assert_eq!(envelope.semantic_type, "hello");
    assert_eq!(envelope.protocol_version.as_deref(), Some("3"));
    assert_eq!(envelope.device_id.as_deref(), Some("device-001"));
    assert_eq!(envelope.client_id.as_deref(), Some("client-001"));
    assert_eq!(
        envelope
            .extensions
            .get("xiaozhi.transport")
            .map(String::as_str),
        Some("udp")
    );
}

#[test]
fn xiaozhi_udp_audio_codec_roundtrips_encrypted_packet_fields_and_payload() {
    let codec = XiaozhiUdpAudioCodec::new(
        "00112233445566778899AABBCCDDEEFF",
        "01000000A1A2A3A40000000000000000",
    )
    .expect("udp audio codec");

    let packet = codec
        .encode_audio_packet(1234, 7, [0x11, 0x22, 0x33, 0x44])
        .expect("encoded packet");
    assert_eq!(packet[0], 0x01);

    let decoded = codec.decode_audio_packet(&packet).expect("decoded packet");
    assert_eq!(decoded.flags, 0);
    assert_eq!(decoded.ssrc, 0xA1A2A3A4);
    assert_eq!(decoded.timestamp, 1234);
    assert_eq!(decoded.sequence, 7);
    assert_eq!(decoded.payload, vec![0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn xiaozhi_udp_audio_codec_rejects_stale_sequences_when_expected_sequence_is_provided() {
    let codec = XiaozhiUdpAudioCodec::new(
        "00112233445566778899AABBCCDDEEFF",
        "01000000A1A2A3A40000000000000000",
    )
    .expect("udp audio codec");

    let packet = codec
        .encode_audio_packet(1234, 4, [0x11, 0x22, 0x33, 0x44])
        .expect("encoded packet");

    let error = codec
        .decode_audio_packet_with_min_sequence(&packet, 5)
        .expect_err("stale sequence should fail");
    assert_eq!(error.code, "xiaozhi.udp.sequence.stale");
}
