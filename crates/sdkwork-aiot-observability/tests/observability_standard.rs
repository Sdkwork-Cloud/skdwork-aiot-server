use sdkwork_aiot_observability::{redact_header, RuntimeMetricFields, TraceFields};

#[test]
fn trace_fields_include_safe_iot_correlation_context() {
    let fields = TraceFields::new("trace1")
        .tenant("t1")
        .device("dev1")
        .protocol("xiaozhi.websocket")
        .session("sess1");

    assert_eq!(fields.trace_id, "trace1");
    assert_eq!(fields.device_id.as_deref(), Some("dev1"));
}

#[test]
fn sensitive_headers_are_redacted() {
    assert_eq!(
        redact_header("Authorization", "Bearer secret"),
        "<redacted>"
    );
    assert_eq!(redact_header("Device-Id", "aa:bb"), "aa:bb");
}

#[test]
fn runtime_metric_fields_cover_capacity_and_backpressure_without_payloads() {
    let fields = RuntimeMetricFields::new("sdkwork-aiot-device-edge-runtime")
        .tenant("t1")
        .protocol("xiaozhi.websocket")
        .node("node-a")
        .connections(90_000)
        .sessions(900_000)
        .device_inflight(32)
        .outbox_lag(100_000)
        .backpressure("slow_down");

    assert_eq!(fields.component, "sdkwork-aiot-device-edge-runtime");
    assert_eq!(fields.tenant_id.as_deref(), Some("t1"));
    assert_eq!(fields.protocol_id.as_deref(), Some("xiaozhi.websocket"));
    assert_eq!(fields.node_id.as_deref(), Some("node-a"));
    assert_eq!(fields.node_connections, Some(90_000));
    assert_eq!(fields.tenant_sessions, Some(900_000));
    assert_eq!(fields.device_inflight, Some(32));
    assert_eq!(fields.outbox_lag, Some(100_000));
    assert_eq!(fields.backpressure_action.as_deref(), Some("slow_down"));
    assert!(!fields.contains_payload_fields());
}

#[test]
fn structured_trace_event_json_omits_empty_optional_fields() {
    let line = sdkwork_aiot_observability::format_trace_event(
        "gateway.session.open",
        &TraceFields::new("trace-001")
            .device("dev-001")
            .protocol("xiaozhi.websocket"),
    );

    assert!(line.contains(r#""event":"gateway.session.open""#));
    assert!(line.contains(r#""traceId":"trace-001""#));
    assert!(line.contains(r#""deviceId":"dev-001""#));
    assert!(line.contains(r#""protocolId":"xiaozhi.websocket""#));
    assert!(!line.contains("tenantId"));
}

#[test]
fn structured_runtime_metric_json_includes_numeric_fields() {
    let line = sdkwork_aiot_observability::format_runtime_metric(
        &RuntimeMetricFields::new("sdkwork-aiot-device-edge-runtime")
            .node("node-a")
            .connections(42)
            .sessions(7),
    );

    assert!(line.contains(r#""event":"runtime.metric""#));
    assert!(line.contains(r#""nodeId":"node-a""#));
    assert!(line.contains(r#""nodeConnections":42"#));
    assert!(line.contains(r#""tenantSessions":7"#));
}

#[test]
fn parse_otlp_endpoint_accepts_http_url_with_default_port() {
    let endpoint =
        sdkwork_aiot_observability::parse_otlp_endpoint("http://otel-collector").expect("endpoint");
    assert_eq!(endpoint.host, "otel-collector");
    assert_eq!(endpoint.port, 4318);
    assert_eq!(endpoint.path, "/v1/traces");
}

#[test]
fn parse_otlp_endpoint_accepts_explicit_host_port_and_path() {
    let endpoint =
        sdkwork_aiot_observability::parse_otlp_endpoint("http://127.0.0.1:4318/v1/traces")
            .expect("endpoint");
    assert_eq!(endpoint.host, "127.0.0.1");
    assert_eq!(endpoint.port, 4318);
    assert_eq!(endpoint.path, "/v1/traces");
}

#[test]
fn otlp_trace_payload_contains_service_name_and_correlation_fields() {
    let payload = sdkwork_aiot_observability::format_otlp_trace_payload(
        "gateway.session.open",
        &TraceFields::new("trace-abc")
            .device("dev-001")
            .protocol("xiaozhi.websocket"),
        "sdkwork-aiot-device-edge-runtime",
    );

    assert!(payload.contains(r#""service.name""#));
    assert!(payload.contains(r#""sdkwork-aiot-device-edge-runtime""#));
    assert!(payload.contains(r#""gateway.session.open""#));
    assert!(payload.contains(r#""device.id""#));
    assert!(payload.contains(r#""dev-001""#));
    assert!(payload.contains(r#""traceId""#));
    assert!(payload.contains(r#""spanId""#));
}

#[test]
fn otlp_runtime_metric_payload_contains_numeric_attributes() {
    let payload = sdkwork_aiot_observability::format_otlp_runtime_metric_payload(
        &RuntimeMetricFields::new("sdkwork-aiot-device-edge-runtime")
            .node("node-a")
            .connections(12)
            .sessions(3),
        "sdkwork-aiot-device-edge-runtime",
    );

    assert!(payload.contains(r#""runtime.metric""#));
    assert!(payload.contains(r#""node.id""#));
    assert!(payload.contains(r#""node-a""#));
    assert!(payload.contains(r#""intValue":"12""#));
    assert!(payload.contains(r#""intValue":"3""#));
}
