use sdkwork_aiot_contract::AiotRequestContext;
use sdkwork_aiot_protocol::{
    InboundFrame, MessageClass, MessageCodec, OutboundFrame, ProtocolAdapterManifest,
    ProtocolEnvelope, ProtocolError, TransportBinding,
};
use sdkwork_aiot_service_host::{
    standard_aiot_integration_bundle, standard_aiot_runtime, AiotConfig, AiotGatewayListenerBundle,
    AiotHealthCheck, AiotHttpRouteBundle, AiotProtocolBundle, AiotProtocolMessageAction,
    AiotProtocolRouteKind, AiotRuntime, AiotRuntimeCapacityPolicy, AiotRuntimePressure,
    AiotStorageBundle, BackpressureAction, ComponentKind, RuntimeMode, RuntimeServicePlan,
};
use sdkwork_aiot_storage::AiotStorageWriteKind;
use sdkwork_iot_device_service::{DomainEventKind, ProtocolIngestAction};

#[derive(Debug, Clone)]
struct FakeCodec {
    envelope: ProtocolEnvelope,
}

impl FakeCodec {
    fn new(envelope: ProtocolEnvelope) -> Self {
        Self { envelope }
    }
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
fn runtime_builder_supports_embedded_and_standalone_modes_with_same_components() {
    let xiaozhi = ProtocolAdapterManifest::new("xiaozhi", "0.1.0")
        .with_protocol("xiaozhi.websocket")
        .with_transport(TransportBinding::WebSocket);

    let embedded = AiotRuntime::builder()
        .with_mode(RuntimeMode::Embedded)
        .with_component("storage")
        .register_protocol_adapter(xiaozhi.clone())
        .build()
        .expect("embedded runtime");

    let standalone = AiotRuntime::builder()
        .with_mode(RuntimeMode::Standalone)
        .with_component("storage")
        .register_protocol_adapter(xiaozhi)
        .build()
        .expect("standalone runtime");

    assert_eq!(embedded.component_names(), standalone.component_names());
    assert!(embedded.supports_protocol("xiaozhi.websocket"));
    assert!(standalone.supports_protocol("xiaozhi.websocket"));
}

#[test]
fn runtime_indexes_protocol_adapters_by_protocol_id() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");

    let adapter = runtime
        .protocol_adapter_for("xiaozhi.websocket")
        .expect("xiaozhi websocket adapter");

    assert_eq!(adapter.plugin_id, "xiaozhi");
    assert!(adapter.transports.contains(&TransportBinding::WebSocket));
    assert!(runtime.protocol_adapter_for("unknown.protocol").is_none());
}

#[test]
fn runtime_rejects_duplicate_protocol_ids_during_build() {
    let first = ProtocolAdapterManifest::new("xiaozhi-a", "0.1.0")
        .with_protocol("xiaozhi.websocket")
        .with_transport(TransportBinding::WebSocket);
    let second = ProtocolAdapterManifest::new("xiaozhi-b", "0.1.0")
        .with_protocol("xiaozhi.websocket")
        .with_transport(TransportBinding::WebSocket);

    let error = AiotRuntime::builder()
        .with_component("storage")
        .register_protocol_adapter(first)
        .register_protocol_adapter(second)
        .build()
        .expect_err("duplicate protocol id must fail");

    assert_eq!(error.code, "runtime.protocol_id.duplicate");
}

#[test]
fn runtime_maps_device_protocol_routes_to_protocol_plugins() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");

    let ws = runtime
        .protocol_route_for_path("/iot/xiaozhi/ws")
        .expect("websocket route");
    assert_eq!(ws.protocol_id, "xiaozhi.websocket");
    assert_eq!(ws.plugin_id, "xiaozhi");
    assert_eq!(ws.transport, TransportBinding::WebSocket);
    assert_eq!(ws.kind, AiotProtocolRouteKind::DeviceSession);

    let ota = runtime
        .protocol_route_for_path("/iot/xiaozhi/ota")
        .expect("ota route");
    assert_eq!(ota.protocol_id, "xiaozhi.websocket");
    assert_eq!(ota.kind, AiotProtocolRouteKind::OtaMetadata);
    assert!(ota.capability_bridges.contains(&"firmware_ota".to_string()));

    assert!(runtime
        .protocol_routes()
        .iter()
        .any(|route| route.path == "/iot/xiaozhi/activate"
            && route.kind == AiotProtocolRouteKind::Provisioning));

    let mqtt = runtime
        .protocol_route_for_path("/iot/xiaozhi/mqtt")
        .expect("mqtt control route");
    assert_eq!(mqtt.protocol_id, "xiaozhi.mqtt_udp");
    assert_eq!(mqtt.transport, TransportBinding::Mqtt);
    assert_eq!(mqtt.kind, AiotProtocolRouteKind::DeviceSession);

    let udp = runtime
        .protocol_route_for_path("/iot/xiaozhi/udp")
        .expect("udp media route");
    assert_eq!(udp.protocol_id, "xiaozhi.mqtt_udp");
    assert_eq!(udp.transport, TransportBinding::Udp);
    assert_eq!(udp.kind, AiotProtocolRouteKind::BridgeIngress);
}

#[test]
fn runtime_processes_registered_protocol_envelopes_into_standard_actions() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
        .device("device-001")
        .client("client-abc")
        .semantic_type("hello")
        .json_payload(r#"{"type":"hello"}"#)
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("protocol result");

    assert_eq!(result.protocol_id, "xiaozhi.websocket");
    assert_eq!(result.plugin_id, "xiaozhi");
    assert_eq!(result.action, AiotProtocolMessageAction::OpenSession);
    assert_eq!(result.pipeline, "device_session");
    assert_eq!(result.device_id.as_deref(), Some("device-001"));
    assert!(result.should_ack);
}

#[test]
fn runtime_rejects_unregistered_protocol_envelopes() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("unknown.protocol", MessageClass::Telemetry)
        .semantic_type("telemetry")
        .json_payload(r#"{"temperature":21}"#)
        .build();

    let error = runtime
        .handle_protocol_envelope(envelope)
        .expect_err("unknown protocol must fail");

    assert_eq!(error.code, "runtime.protocol.unsupported");
}

#[test]
fn runtime_maps_message_classes_to_domain_pipelines() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");

    for (message_class, expected_action, expected_pipeline) in [
        (
            MessageClass::MediaFrame,
            AiotProtocolMessageAction::ProcessMediaFrame,
            "media_ingest",
        ),
        (
            MessageClass::PropertyReport,
            AiotProtocolMessageAction::RecordTelemetry,
            "telemetry_ingest",
        ),
        (
            MessageClass::CommandRequest,
            AiotProtocolMessageAction::DispatchCommand,
            "command_router",
        ),
        (
            MessageClass::OtaCheck,
            AiotProtocolMessageAction::EvaluateOta,
            "ota",
        ),
    ] {
        let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", message_class)
            .device("device-001")
            .semantic_type(expected_pipeline)
            .build();

        let result = runtime
            .handle_protocol_envelope(envelope)
            .expect("protocol result");

        assert_eq!(result.action, expected_action);
        assert_eq!(result.pipeline, expected_pipeline);
        assert_eq!(result.plugin_id, "xiaozhi");
    }
}

#[test]
fn runtime_protocol_result_converts_to_core_ingest_plan() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::PropertyReport)
        .device("device-001")
        .client("client-abc")
        .session("session-001")
        .semantic_type("iot")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let record = result.to_core_ingest_record();
    let plan = result.to_core_ingest_plan();

    assert_eq!(record.action, ProtocolIngestAction::RecordTelemetry);
    assert_eq!(record.device_id, "device-001");
    assert_eq!(record.client_id.as_deref(), Some("client-abc"));
    assert_eq!(record.session_id.as_deref(), Some("session-001"));
    assert_eq!(plan.event_kind, DomainEventKind::TelemetryRecorded);
    assert_eq!(plan.primary_table, "iot_telemetry_event");
}

#[test]
fn runtime_protocol_result_preserves_media_reference_for_media_frame_ingest() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::MediaFrame)
        .device("device-001")
        .client("client-abc")
        .session("session-001")
        .semantic_type("audio")
        .media_resource_id("media-res-001")
        .object_blob_id("obj-blob-001")
        .media_resource_snapshot(
            r#"{"id":"media-res-001","kind":"audio","source":"object_storage"}"#,
        )
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let record = result.to_core_ingest_record();
    let plan = result.to_core_ingest_plan();
    let command = result.to_storage_command();

    assert_eq!(record.action, ProtocolIngestAction::ProcessMediaFrame);
    assert_eq!(record.media_resource_id.as_deref(), Some("media-res-001"));
    assert_eq!(record.object_blob_id.as_deref(), Some("obj-blob-001"));
    assert!(record
        .media_resource_snapshot
        .as_deref()
        .unwrap_or_default()
        .contains(r#""kind":"audio""#));
    assert_eq!(plan.primary_table, "iot_device_event");
    assert_eq!(command.media_resource_id.as_deref(), Some("media-res-001"));
    assert_eq!(command.object_blob_id.as_deref(), Some("obj-blob-001"));
    assert!(command
        .media_resource_snapshot
        .as_deref()
        .unwrap_or_default()
        .contains(r#""source":"object_storage""#));
}

#[test]
fn runtime_protocol_result_converts_to_storage_transaction_command() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
        .device("device-001")
        .client("client-abc")
        .session("session-001")
        .semantic_type("hello")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let command = result.to_storage_command();

    assert_eq!(command.protocol_id, "xiaozhi.websocket");
    assert_eq!(command.adapter_id, "xiaozhi");
    assert_eq!(command.device_id, "device-001");
    assert_eq!(command.kind, AiotStorageWriteKind::OpenSession);
    assert_eq!(command.primary_table, "iot_device_session");
    assert_eq!(command.session_id.as_deref(), Some("session-001"));
    assert!(command.requires_transaction);
    assert!(command.dead_letter_on_failure);

    let outbox = command.outbox.expect("session event outbox");
    assert_eq!(outbox.event_type, "iot.device.session.started");
    assert_eq!(outbox.aggregate_type, "device_session");
    assert_eq!(outbox.aggregate_id, "session-001");
    assert_eq!(outbox.topic, "iot.protocol.ingested");
}

#[test]
fn runtime_preserves_protocol_reliability_metadata_into_storage_command() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::PropertyReport)
        .message_id("msg-001")
        .correlation_id("corr-001")
        .idempotency_key("idem-001")
        .trace_id("trace-001")
        .device("device-001")
        .semantic_type("iot")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let command = result.to_storage_command();

    assert_eq!(result.message_id.as_deref(), Some("msg-001"));
    assert_eq!(result.correlation_id.as_deref(), Some("corr-001"));
    assert_eq!(result.idempotency_key.as_deref(), Some("idem-001"));
    assert_eq!(result.trace_id.as_deref(), Some("trace-001"));
    assert_eq!(command.message_id.as_deref(), Some("msg-001"));
    assert_eq!(command.correlation_id.as_deref(), Some("corr-001"));
    assert_eq!(command.idempotency_key.as_deref(), Some("idem-001"));
    assert_eq!(command.trace_id.as_deref(), Some("trace-001"));
}

#[test]
fn runtime_storage_command_outbox_payload_is_versioned_and_contains_media_identity() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::MediaFrame)
        .device("device-009")
        .session("session-009")
        .trace_id("trace-009")
        .semantic_type("audio")
        .media_resource_id("media-res-009")
        .object_blob_id("obj-blob-009")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let command = result.to_storage_command();
    let outbox = command.outbox.as_ref().expect("outbox");

    assert_eq!(outbox.event_version, "1");
    assert!(outbox.payload_json.contains(r#""eventVersion":"1""#));
    assert!(outbox
        .payload_json
        .contains(r#""messageClass":"mediaFrame""#));
    assert!(outbox
        .payload_json
        .contains(r#""mediaResourceId":"media-res-009""#));
    assert!(outbox
        .payload_json
        .contains(r#""objectBlobId":"obj-blob-009""#));
}

#[test]
fn runtime_storage_command_outbox_payload_hash_matches_payload_json_sha256() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Telemetry)
        .device("device-010")
        .trace_id("trace-010")
        .semantic_type("telemetry")
        .build();
    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let command = result.to_storage_command();
    let outbox = command.outbox.as_ref().expect("outbox");

    let expected = sdkwork_utils_rust::sha256_hash(outbox.payload_json.as_bytes());

    assert_eq!(outbox.payload_hash.as_deref(), Some(expected.as_str()));
}

#[test]
fn runtime_maps_request_context_to_storage_command_association_without_iam_dependency() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::PropertyReport)
        .device("device-001")
        .semantic_type("iot")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let ctx = AiotRequestContext::new("100001", "0")
        .with_user("30001")
        .with_data_scope("7");

    let command = result
        .to_storage_command_with_context(&ctx)
        .expect("storage command with context");

    assert_eq!(command.association.tenant_id, 100001);
    assert_eq!(command.association.organization_id, 0);
    assert_eq!(command.association.user_id, Some(30001));
    assert_eq!(command.association.data_scope, 7);
}

#[test]
fn runtime_rejects_invalid_request_context_association_for_storage_command() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::PropertyReport)
        .device("device-001")
        .semantic_type("iot")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");

    let invalid_tenant = result
        .to_storage_command_with_context(&AiotRequestContext::new("tenant-a", "1"))
        .expect_err("invalid tenant id must fail");
    assert_eq!(invalid_tenant.code, "runtime.context.invalid_tenant_id");

    let invalid_organization = result
        .to_storage_command_with_context(&AiotRequestContext::new("100001", "org-a"))
        .expect_err("invalid organization id must fail");
    assert_eq!(
        invalid_organization.code,
        "runtime.context.invalid_organization_id"
    );

    let invalid_user = result
        .to_storage_command_with_context(
            &AiotRequestContext::new("100001", "0").with_user("user-a"),
        )
        .expect_err("invalid user id must fail");
    assert_eq!(invalid_user.code, "runtime.context.invalid_user_id");

    let invalid_scope = result
        .to_storage_command_with_context(
            &AiotRequestContext::new("100001", "0").with_data_scope("tenant:t1"),
        )
        .expect_err("invalid data scope must fail");
    assert_eq!(invalid_scope.code, "runtime.context.invalid_data_scope");
}

#[test]
fn runtime_storage_command_uses_device_as_fallback_aggregate_for_stateless_messages() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let envelope = ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::PropertyReport)
        .device("device-001")
        .semantic_type("iot")
        .build();

    let result = runtime
        .handle_protocol_envelope(envelope)
        .expect("runtime result");
    let command = result.to_storage_command();

    assert_eq!(command.kind, AiotStorageWriteKind::RecordTelemetry);
    assert_eq!(command.primary_table, "iot_telemetry_event");
    let outbox = command.outbox.expect("telemetry event outbox");
    assert_eq!(outbox.event_type, "iot.telemetry.received");
    assert_eq!(outbox.aggregate_type, "device");
    assert_eq!(outbox.aggregate_id, "device-001");
}

#[test]
fn runtime_protocol_pipeline_decodes_frame_and_returns_storage_command() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let codec = FakeCodec::new(
        ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
            .device("device-001")
            .client("client-abc")
            .session("session-001")
            .semantic_type("hello")
            .build(),
    );

    let result = runtime
        .handle_inbound_frame_with_codec(
            "/iot/xiaozhi/ws",
            &codec,
            InboundFrame::text(r#"{"type":"hello"}"#),
        )
        .expect("pipeline result");

    assert_eq!(result.route.path, "/iot/xiaozhi/ws");
    assert_eq!(result.envelope.protocol_id, "xiaozhi.websocket");
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
fn runtime_protocol_pipeline_with_context_maps_storage_association() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let codec = FakeCodec::new(
        ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
            .device("device-001")
            .client("client-abc")
            .session("session-001")
            .semantic_type("hello")
            .build(),
    );
    let ctx = AiotRequestContext::new("100001", "0")
        .with_user("30001")
        .with_data_scope("7");

    let result = runtime
        .handle_inbound_frame_with_context(
            "/iot/xiaozhi/ws",
            &ctx,
            &codec,
            InboundFrame::text(r#"{"type":"hello"}"#),
        )
        .expect("pipeline result");

    assert_eq!(result.storage_command.association.tenant_id, 100001);
    assert_eq!(result.storage_command.association.organization_id, 0);
    assert_eq!(result.storage_command.association.user_id, Some(30001));
    assert_eq!(result.storage_command.association.data_scope, 7);
}

#[test]
fn runtime_protocol_pipeline_with_context_rejects_invalid_association() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let codec = FakeCodec::new(
        ProtocolEnvelope::builder("xiaozhi.websocket", MessageClass::Handshake)
            .device("device-001")
            .semantic_type("hello")
            .build(),
    );
    let ctx = AiotRequestContext::new("tenant-a", "1");

    let error = runtime
        .handle_inbound_frame_with_context(
            "/iot/xiaozhi/ws",
            &ctx,
            &codec,
            InboundFrame::text(r#"{"type":"hello"}"#),
        )
        .expect_err("invalid context must fail");

    assert_eq!(error.code, "runtime.context.invalid_tenant_id");
}

#[test]
fn runtime_protocol_pipeline_rejects_unknown_or_mismatched_protocol_routes() {
    let runtime = standard_aiot_runtime(RuntimeMode::Embedded).expect("runtime");
    let mqtt_codec = FakeCodec::new(
        ProtocolEnvelope::builder("mqtt.v5", MessageClass::Telemetry)
            .device("device-001")
            .semantic_type("telemetry")
            .build(),
    );

    let unknown_path = runtime
        .handle_inbound_frame_with_codec(
            "/iot/unknown/ws",
            &mqtt_codec,
            InboundFrame::text(r#"{"temperature":21}"#),
        )
        .expect_err("unknown route must fail");
    assert_eq!(unknown_path.code, "runtime.protocol_route.unsupported");

    let mismatched = runtime
        .handle_inbound_frame_with_codec(
            "/iot/xiaozhi/ws",
            &mqtt_codec,
            InboundFrame::text(r#"{"temperature":21}"#),
        )
        .expect_err("mismatched protocol must fail");
    assert_eq!(
        mismatched.code,
        "runtime.protocol_route.mismatched_protocol"
    );
}

#[test]
fn runtime_requires_resolved_request_context_from_host() {
    let ctx = AiotRequestContext::new("t1", "o1").with_permission("iot.devices.read");
    let runtime = AiotRuntime::builder()
        .with_mode(RuntimeMode::Embedded)
        .with_component("storage")
        .build()
        .expect("runtime");

    assert!(runtime.accepts_context(&ctx));
}

#[test]
fn default_runtime_bundle_is_library_first_and_service_shells_reuse_it() {
    let embedded = standard_aiot_runtime(RuntimeMode::Embedded).expect("embedded");
    let standalone = standard_aiot_runtime(RuntimeMode::Standalone).expect("standalone");

    assert_eq!(embedded.component_names(), standalone.component_names());
    assert!(embedded.supports_protocol("xiaozhi.websocket"));
    assert!(standalone.supports_protocol("xiaozhi.websocket"));
    assert!(embedded
        .component_kinds()
        .contains(&ComponentKind::ProtocolAdapter));
    assert!(embedded
        .component_kinds()
        .contains(&ComponentKind::StoragePort));
    assert!(embedded
        .component_kinds()
        .contains(&ComponentKind::SecurityPort));
    assert!(embedded.is_embeddable());
    assert!(standalone.is_standalone());
}

#[test]
fn service_plan_declares_independent_mountable_surfaces() {
    let plan = RuntimeServicePlan::standard();

    assert!(plan.gateway_routes.contains(&"/iot/xiaozhi/ws"));
    assert!(plan
        .backend_routes
        .iter()
        .all(|route| route.starts_with("/backend/v3/api/iot")));
    assert!(plan
        .app_routes
        .iter()
        .all(|route| route.starts_with("/app/v3/api/iot")));
    assert!(plan.embedded_mountable);
    assert!(plan.standalone_startable);
    assert!(plan.requires_external_iam_context);
}

#[test]
fn integration_bundle_exposes_fast_embedding_contract() {
    let bundle = standard_aiot_integration_bundle(RuntimeMode::Embedded).expect("bundle");

    assert_eq!(bundle.runtime.mode(), RuntimeMode::Embedded);
    assert_eq!(bundle.component_manifest.name, "sdkwork-aiot-server");
    assert!(bundle.service_plan.embedded_mountable);
    assert!(bundle.service_plan.standalone_startable);
    assert!(bundle
        .protocol_catalog
        .iter()
        .any(|protocol| protocol.protocol_id == "xiaozhi.websocket"));
    assert!(bundle
        .sdk_families
        .iter()
        .any(|sdk| sdk.name == "app" && sdk.package_name == "@sdkwork/aiot-app-sdk"));
    assert!(bundle
        .sdk_families
        .iter()
        .any(|sdk| sdk.name == "backend" && sdk.package_name == "@sdkwork/aiot-backend-sdk"));
    assert!(bundle
        .sdk_families
        .iter()
        .all(|sdk| sdk.openapi_path.ends_with(".openapi.json")));
}

#[test]
fn composable_bundle_types_model_storage_protocol_routes_listeners_and_health() {
    let config = AiotConfig::standard();
    let storage = AiotStorageBundle::standard_sqlx();
    let protocols = AiotProtocolBundle::standard();
    let routes = AiotHttpRouteBundle::standard();
    let listeners = AiotGatewayListenerBundle::standard();
    let health = AiotHealthCheck::ready("sdkwork-aiot-service-host");

    assert_eq!(config.app_api_prefix, "/app/v3/api/iot");
    assert_eq!(config.backend_api_prefix, "/backend/v3/api/iot");
    assert!(config.requires_external_iam_context);
    assert_eq!(storage.schema_version, "0.2.0");
    assert!(storage.migrations_required);
    assert!(protocols.protocol_ids.contains(&"xiaozhi.websocket"));
    assert!(protocols.protocol_ids.contains(&"mqtt.v5"));
    assert!(protocols
        .protocol_ids
        .contains(&"raspberrypi.linux_gateway"));
    assert!(protocols.protocol_ids.contains(&"raspberrypi.pico_mqtt"));
    assert!(routes
        .app_routes
        .iter()
        .all(|route| route.starts_with("/app/v3/api/iot")));
    assert!(routes
        .backend_routes
        .iter()
        .all(|route| route.starts_with("/backend/v3/api/iot")));
    assert!(listeners.websocket_routes.contains(&"/iot/xiaozhi/ws"));
    assert!(listeners.supports_socket);
    assert!(health.ready);
}

#[test]
fn standard_runtime_policy_models_high_availability_and_backpressure() {
    let policy = AiotRuntimeCapacityPolicy::standard();

    assert_eq!(policy.node_id, "local");
    assert_eq!(policy.max_connections_per_node, 100_000);
    assert_eq!(policy.max_sessions_per_tenant, 1_000_000);
    assert_eq!(policy.max_inflight_per_device, 64);
    assert_eq!(policy.session_lease_ttl_seconds, 90);
    assert_eq!(policy.session_lease_renew_seconds, 30);
    assert_eq!(policy.outbox_max_attempts, 12);
    assert_eq!(policy.dead_letter_after_attempts, 12);
    assert!(policy.enable_ordered_device_commands);
    assert!(policy.enable_idempotent_ingest);
}

#[test]
fn runtime_policy_backpressure_decision_is_deterministic() {
    let policy = AiotRuntimeCapacityPolicy::standard();

    let normal = AiotRuntimePressure {
        node_connections: 10_000,
        tenant_sessions: 100_000,
        device_inflight: 12,
        outbox_lag: 1_000,
    };
    assert_eq!(
        policy.backpressure_action(&normal),
        BackpressureAction::Accept
    );

    let slow_down = AiotRuntimePressure {
        node_connections: 90_000,
        tenant_sessions: 100_000,
        device_inflight: 12,
        outbox_lag: 1_000,
    };
    assert_eq!(
        policy.backpressure_action(&slow_down),
        BackpressureAction::SlowDown
    );

    let reject = AiotRuntimePressure {
        node_connections: 100_001,
        tenant_sessions: 100_000,
        device_inflight: 12,
        outbox_lag: 1_000,
    };
    assert_eq!(
        policy.backpressure_action(&reject),
        BackpressureAction::Reject
    );

    let dead_letter = AiotRuntimePressure {
        node_connections: 10_000,
        tenant_sessions: 100_000,
        device_inflight: 12,
        outbox_lag: 1_000_001,
    };
    assert_eq!(
        policy.backpressure_action(&dead_letter),
        BackpressureAction::DeadLetterNonCritical
    );
}

#[test]
fn integration_bundle_exposes_runtime_capacity_policy_for_embedded_hosts() {
    let bundle = standard_aiot_integration_bundle(RuntimeMode::Embedded).expect("bundle");

    assert_eq!(bundle.capacity_policy.max_inflight_per_device, 64);
    assert_eq!(bundle.capacity_policy.session_lease_ttl_seconds, 90);
    assert_eq!(bundle.capacity_policy.outbox_max_attempts, 12);
}
