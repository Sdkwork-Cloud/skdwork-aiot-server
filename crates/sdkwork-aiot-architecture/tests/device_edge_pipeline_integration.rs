use sdkwork_aiot_adapter_xiaozhi::{
    xiaozhi_handshake_context, XiaozhiWebSocketCodec, CLIENT_ID_HEADER, DEVICE_ID_HEADER,
    PROTOCOL_VERSION_HEADER, XIAOZHI_WEBSOCKET_PROTOCOL_ID, XIAOZHI_WS_PATH,
};
use sdkwork_aiot_contract::AiotRequestContext;
use sdkwork_aiot_protocol::MessageClass;
use sdkwork_aiot_service_host::AiotProtocolMessageAction;
use sdkwork_aiot_storage::{
    AiotProtocolDeadLetterIntent, AiotProtocolIngestUnitOfWork, AiotStorageWriteKind,
    InMemoryProtocolIngestUnitOfWork,
};
use sdkwork_aiot_transport::{
    handle_websocket_message_bytes, handle_websocket_message_bytes_with_context, TransportServer,
};

fn websocket_text_frame(text: &str) -> Vec<u8> {
    let payload = text.as_bytes();

    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.push(0x81);
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else {
        assert!(u16::try_from(payload.len()).is_ok());
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    frame
}

#[test]
fn xiaozhi_websocket_frame_flows_to_standard_storage_command_with_reliability_metadata() {
    let server = TransportServer::standard_standalone().expect("transport server");
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let payload = r#"{"type":"hello","session_id":"session-001","message_id":"msg-001","correlation_id":"corr-001","idempotency_key":"idem-001","trace_id":"trace-001","transport":"websocket"}"#;
    let frame = websocket_text_frame(payload);

    let result = handle_websocket_message_bytes(&server, XIAOZHI_WS_PATH, &codec, &frame)
        .expect("xiaozhi device edge pipeline result");

    assert_eq!(result.envelope.protocol_id, XIAOZHI_WEBSOCKET_PROTOCOL_ID);
    assert_eq!(result.envelope.message_class, MessageClass::Handshake);
    assert_eq!(
        result.message.action,
        AiotProtocolMessageAction::OpenSession
    );
    assert_eq!(
        result.storage_command.kind,
        AiotStorageWriteKind::OpenSession
    );
    assert_eq!(
        result.storage_command.session_id.as_deref(),
        Some("session-001")
    );
    assert_eq!(
        result.storage_command.message_id.as_deref(),
        Some("msg-001")
    );
    assert_eq!(
        result.storage_command.correlation_id.as_deref(),
        Some("corr-001")
    );
    assert_eq!(
        result.storage_command.idempotency_key.as_deref(),
        Some("idem-001")
    );
    assert_eq!(
        result.storage_command.trace_id.as_deref(),
        Some("trace-001")
    );

    let outbox = result
        .storage_command
        .outbox
        .expect("standard session outbox event");
    assert_eq!(outbox.event_type, "iot.device.session.started");
    assert_eq!(outbox.aggregate_type, "device_session");
    assert_eq!(outbox.aggregate_id, "session-001");
}

#[test]
fn xiaozhi_device_edge_pipeline_can_execute_standard_storage_uow_idempotently() {
    let server = TransportServer::standard_standalone().expect("transport server");
    let storage = InMemoryProtocolIngestUnitOfWork::new();
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let payload = r#"{"type":"hello","session_id":"session-001","message_id":"msg-001","idempotency_key":"idem-001","trace_id":"trace-001"}"#;
    let frame = websocket_text_frame(payload);

    let first = handle_websocket_message_bytes(&server, XIAOZHI_WS_PATH, &codec, &frame)
        .expect("first pipeline result");
    let first_receipt = storage.execute_protocol_command(&first.storage_command);
    let second = handle_websocket_message_bytes(&server, XIAOZHI_WS_PATH, &codec, &frame)
        .expect("second pipeline result");
    let second_receipt = storage.execute_protocol_command(&second.storage_command);
    let snapshot = storage.snapshot();

    assert!(first_receipt.accepted);
    assert!(!first_receipt.duplicate);
    assert!(second_receipt.accepted);
    assert!(second_receipt.duplicate);
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
    assert_eq!(snapshot.primary_writes[0].idempotency_key, "idem-001");
    assert_eq!(
        snapshot.primary_writes[0].message_id.as_deref(),
        Some("msg-001")
    );
    assert_eq!(
        snapshot.primary_writes[0].trace_id.as_deref(),
        Some("trace-001")
    );
}

#[test]
fn xiaozhi_device_edge_pipeline_preserves_host_resolved_appbase_context_into_storage_uow() {
    let server = TransportServer::standard_standalone().expect("transport server");
    let storage = InMemoryProtocolIngestUnitOfWork::new();
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let ctx = AiotRequestContext::new("100001", "0")
        .with_user("30001")
        .with_data_scope("7");
    let payload = r#"{"type":"hello","session_id":"session-001","message_id":"msg-001","idempotency_key":"idem-001","trace_id":"trace-001"}"#;
    let frame = websocket_text_frame(payload);

    let result =
        handle_websocket_message_bytes_with_context(&server, XIAOZHI_WS_PATH, &ctx, &codec, &frame)
            .expect("context-aware xiaozhi pipeline result");
    let receipt = storage.execute_protocol_command(&result.storage_command);
    let snapshot = storage.snapshot();

    assert!(receipt.accepted);
    assert_eq!(snapshot.primary_writes.len(), 1);
    assert_eq!(snapshot.outbox_events.len(), 1);
    assert_eq!(snapshot.primary_writes[0].association.tenant_id, 100001);
    assert_eq!(snapshot.primary_writes[0].association.organization_id, 0);
    assert_eq!(snapshot.primary_writes[0].association.user_id, Some(30001));
    assert_eq!(snapshot.primary_writes[0].association.data_scope, 7);
    assert_eq!(snapshot.outbox_events[0].association.tenant_id, 100001);
    assert_eq!(snapshot.outbox_events[0].association.organization_id, 0);
    assert_eq!(snapshot.outbox_events[0].association.user_id, Some(30001));
    assert_eq!(snapshot.outbox_events[0].association.data_scope, 7);
}

#[test]
fn xiaozhi_device_edge_pipeline_decode_failure_can_be_recorded_as_dead_letter() {
    let server = TransportServer::standard_standalone().expect("transport server");
    let storage = InMemoryProtocolIngestUnitOfWork::new();
    let codec = XiaozhiWebSocketCodec::new().with_handshake_context(xiaozhi_handshake_context([
        (PROTOCOL_VERSION_HEADER, "3"),
        (DEVICE_ID_HEADER, "device-001"),
        (CLIENT_ID_HEADER, "client-abc"),
    ]));
    let payload = r#"{"type":"not-standard","message_id":"msg-002","trace_id":"trace-002"}"#;
    let frame = websocket_text_frame(payload);

    let error = handle_websocket_message_bytes(&server, XIAOZHI_WS_PATH, &codec, &frame)
        .expect_err("unsupported message type must fail");
    let intent = AiotProtocolDeadLetterIntent::from_protocol_error(
        XIAOZHI_WEBSOCKET_PROTOCOL_ID,
        "xiaozhi",
        &error.code,
        "object-store://payloads/msg-002",
    )
    .with_device_id("device-001")
    .with_trace_id("trace-002");
    let receipt = storage.record_dead_letter(&intent);
    let snapshot = storage.snapshot();

    assert!(!receipt.accepted);
    assert_eq!(
        receipt.dead_letter_reason.as_deref(),
        Some("xiaozhi.message_type.unsupported")
    );
    assert_eq!(snapshot.dead_letters.len(), 1);
    assert_eq!(
        snapshot.dead_letters[0].reason_code,
        "xiaozhi.message_type.unsupported"
    );
    assert_eq!(
        snapshot.dead_letters[0].payload_ref.as_deref(),
        Some("object-store://payloads/msg-002")
    );
    assert!(snapshot.dead_letters[0].raw_payload.is_none());
}
