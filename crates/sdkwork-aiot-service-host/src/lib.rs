use sdkwork_aiot_contract::{
    aiot_component_manifest, AiotComponentManifest, AiotRequestContext, IOT_APP_API_PREFIX,
    IOT_BACKEND_API_PREFIX, IOT_XIAOZHI_BASE_PATH,
};
use sdkwork_aiot_protocol::{
    standard_protocol_catalog, CodecKind, InboundFrame, MessageClass, MessageCodec,
    ProtocolCatalogEntry, ProtocolEnvelope, ProtocolPluginScope, SessionPolicy,
};
use sdkwork_aiot_protocol::{ProtocolAdapterManifest, TransportBinding};
use sdkwork_aiot_security::DeviceAuthMode;
use sdkwork_aiot_storage::{
    AiotOutboxWriteIntent, AiotProtocolStorageCommand, AiotStorageAssociation, AiotStorageWriteKind,
};
use sdkwork_iot_device_service::{
    protocol_ingest_plan, ProtocolIngestAction, ProtocolIngestPlan, ProtocolIngestRecord,
};
use std::collections::BTreeSet;
use std::sync::Arc;

mod outbox_dispatcher;

pub use outbox_dispatcher::{
    outbox_publisher_from_env, AiotOutboxDispatcher, AiotOutboxDispatcherConfig,
    OutboxEventPublisher, StructuredLogOutboxPublisher, WebhookOutboxPublisher,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Embedded,
    Standalone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentKind {
    Contract,
    DomainCore,
    ProtocolAdapter,
    Runtime,
    StoragePort,
    StorageImplementation,
    SecurityPort,
    Observability,
    Gateway,
    HttpApi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComponent {
    pub manifest: AiotComponentManifest,
    pub kind: ComponentKind,
}

impl RuntimeComponent {
    pub fn new(manifest: AiotComponentManifest, kind: ComponentKind) -> Self {
        Self { manifest, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotRuntime {
    mode: RuntimeMode,
    components: Vec<RuntimeComponent>,
    protocol_adapters: Vec<ProtocolAdapterManifest>,
    protocol_routes: Vec<AiotProtocolRoute>,
    capacity_policy: AiotRuntimeCapacityPolicy,
}

impl AiotRuntime {
    pub fn builder() -> AiotRuntimeBuilder {
        AiotRuntimeBuilder {
            mode: RuntimeMode::Embedded,
            components: Vec::new(),
            protocol_adapters: Vec::new(),
            protocol_routes: Vec::new(),
        }
    }

    pub fn mode(&self) -> RuntimeMode {
        self.mode
    }

    pub fn component_names(&self) -> Vec<String> {
        self.components
            .iter()
            .map(|component| component.manifest.name.clone())
            .collect()
    }

    pub fn component_kinds(&self) -> Vec<ComponentKind> {
        self.components
            .iter()
            .map(|component| component.kind)
            .collect()
    }

    pub fn supports_protocol(&self, protocol_id: &str) -> bool {
        self.protocol_adapters.iter().any(|adapter| {
            adapter
                .protocol_ids
                .iter()
                .any(|candidate| candidate == protocol_id)
        })
    }

    pub fn protocol_adapter_for(&self, protocol_id: &str) -> Option<&ProtocolAdapterManifest> {
        self.protocol_adapters.iter().find(|adapter| {
            adapter
                .protocol_ids
                .iter()
                .any(|candidate| candidate == protocol_id)
        })
    }

    pub fn protocol_routes(&self) -> &[AiotProtocolRoute] {
        &self.protocol_routes
    }

    pub fn protocol_route_for_path(&self, path: &str) -> Option<&AiotProtocolRoute> {
        self.protocol_routes.iter().find(|route| route.path == path)
    }

    pub fn capacity_policy(&self) -> &AiotRuntimeCapacityPolicy {
        &self.capacity_policy
    }

    pub fn handle_protocol_envelope(
        &self,
        envelope: ProtocolEnvelope,
    ) -> Result<AiotProtocolMessageResult, RuntimeProtocolError> {
        let adapter = self
            .protocol_adapter_for(&envelope.protocol_id)
            .ok_or_else(|| RuntimeProtocolError::new("runtime.protocol.unsupported"))?;
        let (action, pipeline, should_ack) = protocol_message_action(envelope.message_class);

        Ok(AiotProtocolMessageResult {
            protocol_id: envelope.protocol_id,
            plugin_id: adapter.plugin_id.clone(),
            action,
            pipeline,
            message_class: envelope.message_class,
            semantic_type: envelope.semantic_type,
            message_id: envelope.message_id,
            correlation_id: envelope.correlation_id,
            idempotency_key: envelope.idempotency_key,
            device_id: envelope.device_id,
            client_id: envelope.client_id,
            session_id: envelope.session_id,
            trace_id: envelope.trace_id,
            media_resource_id: envelope.media_resource_id,
            object_blob_id: envelope.object_blob_id,
            media_resource_snapshot: envelope.media_resource_snapshot,
            should_ack,
        })
    }

    pub fn handle_inbound_frame_with_codec<C>(
        &self,
        path: &str,
        codec: &C,
        frame: InboundFrame,
    ) -> Result<AiotGatewayPipelineResult, RuntimeProtocolError>
    where
        C: MessageCodec,
    {
        self.handle_inbound_frame(path, None, codec, frame)
    }

    pub fn handle_inbound_frame_with_context<C>(
        &self,
        path: &str,
        ctx: &AiotRequestContext,
        codec: &C,
        frame: InboundFrame,
    ) -> Result<AiotGatewayPipelineResult, RuntimeProtocolError>
    where
        C: MessageCodec,
    {
        self.handle_inbound_frame(path, Some(ctx), codec, frame)
    }

    fn handle_inbound_frame<C>(
        &self,
        path: &str,
        ctx: Option<&AiotRequestContext>,
        codec: &C,
        frame: InboundFrame,
    ) -> Result<AiotGatewayPipelineResult, RuntimeProtocolError>
    where
        C: MessageCodec,
    {
        let route = self
            .protocol_route_for_path(path)
            .cloned()
            .ok_or_else(|| RuntimeProtocolError::new("runtime.protocol_route.unsupported"))?;
        let envelope = codec
            .decode(frame)
            .map_err(|error| RuntimeProtocolError::new(error.code))?;

        if envelope.protocol_id != route.protocol_id {
            return Err(RuntimeProtocolError::new(
                "runtime.protocol_route.mismatched_protocol",
            ));
        }

        let message = self.handle_protocol_envelope(envelope.clone())?;
        let storage_command = match ctx {
            Some(ctx) => message.to_storage_command_with_context(ctx)?,
            None => message.to_storage_command(),
        };

        Ok(AiotGatewayPipelineResult {
            route,
            envelope,
            message,
            storage_command,
        })
    }

    pub fn accepts_context(&self, ctx: &AiotRequestContext) -> bool {
        !ctx.tenant_id.is_empty() && !ctx.organization_id.is_empty()
    }

    pub fn is_embeddable(&self) -> bool {
        self.components.iter().any(|component| {
            component
                .manifest
                .capabilities
                .contains(&"embedded_runtime".to_string())
        })
    }

    pub fn is_standalone(&self) -> bool {
        self.mode == RuntimeMode::Standalone
            && self.components.iter().any(|component| {
                component
                    .manifest
                    .capabilities
                    .contains(&"standalone_server".to_string())
            })
    }
}

#[derive(Debug, Clone)]
pub struct AiotRuntimeBuilder {
    mode: RuntimeMode,
    components: Vec<RuntimeComponent>,
    protocol_adapters: Vec<ProtocolAdapterManifest>,
    protocol_routes: Vec<AiotProtocolRoute>,
}

impl AiotRuntimeBuilder {
    pub fn with_mode(mut self, mode: RuntimeMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_component(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.components.push(RuntimeComponent::new(
            AiotComponentManifest::new(name, "runtime_component"),
            ComponentKind::Runtime,
        ));
        self
    }

    pub fn with_component_manifest(mut self, manifest: AiotComponentManifest) -> Self {
        self.components
            .push(RuntimeComponent::new(manifest, ComponentKind::Runtime));
        self
    }

    pub fn with_component_kind(
        mut self,
        manifest: AiotComponentManifest,
        kind: ComponentKind,
    ) -> Self {
        self.components.push(RuntimeComponent::new(manifest, kind));
        self
    }

    pub fn register_protocol_adapter(mut self, adapter: ProtocolAdapterManifest) -> Self {
        self.protocol_adapters.push(adapter);
        self
    }

    pub fn register_protocol_route(mut self, route: AiotProtocolRoute) -> Self {
        self.protocol_routes.push(route);
        self
    }

    pub fn build(self) -> Result<AiotRuntime, RuntimeBuildError> {
        if self
            .components
            .iter()
            .any(|component| component.manifest.name.trim().is_empty())
        {
            return Err(RuntimeBuildError::new("runtime.empty_component_name"));
        }

        let mut protocol_ids = BTreeSet::new();
        for adapter in &self.protocol_adapters {
            for protocol_id in &adapter.protocol_ids {
                if protocol_id.trim().is_empty() {
                    return Err(RuntimeBuildError::new("runtime.protocol_id.empty"));
                }
                if !protocol_ids.insert(protocol_id.clone()) {
                    return Err(RuntimeBuildError::new("runtime.protocol_id.duplicate"));
                }
            }
        }

        let mut route_paths = BTreeSet::new();
        for route in &self.protocol_routes {
            if route.path.trim().is_empty() {
                return Err(RuntimeBuildError::new("runtime.protocol_route.empty_path"));
            }
            if !route_paths.insert(route.path.clone()) {
                return Err(RuntimeBuildError::new(
                    "runtime.protocol_route.duplicate_path",
                ));
            }
            if !protocol_ids.contains(&route.protocol_id) {
                return Err(RuntimeBuildError::new(
                    "runtime.protocol_route.unknown_protocol",
                ));
            }
        }

        Ok(AiotRuntime {
            mode: self.mode,
            components: self.components,
            protocol_adapters: self.protocol_adapters,
            protocol_routes: self.protocol_routes,
            capacity_policy: AiotRuntimeCapacityPolicy::standard(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBuildError {
    pub code: String,
}

impl RuntimeBuildError {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

impl std::fmt::Display for RuntimeBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.code)
    }
}

impl std::error::Error for RuntimeBuildError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotProtocolMessageAction {
    OpenSession,
    Authenticate,
    KeepAlive,
    CloseSession,
    ProvisionDevice,
    RecordTelemetry,
    ApplyDesiredTwin,
    DispatchCommand,
    RecordCommandAck,
    RecordCommandResult,
    ProcessMediaFrame,
    EvaluateOta,
    DispatchOta,
    UpdateGatewayTopology,
    RecordSecurityEvent,
    RecordDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotProtocolMessageResult {
    pub protocol_id: String,
    pub plugin_id: String,
    pub action: AiotProtocolMessageAction,
    pub pipeline: &'static str,
    pub message_class: MessageClass,
    pub semantic_type: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub device_id: Option<String>,
    pub client_id: Option<String>,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_resource_snapshot: Option<String>,
    pub should_ack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotGatewayPipelineResult {
    pub route: AiotProtocolRoute,
    pub envelope: ProtocolEnvelope,
    pub message: AiotProtocolMessageResult,
    pub storage_command: AiotProtocolStorageCommand,
}

impl AiotProtocolMessageResult {
    pub fn to_core_ingest_record(&self) -> ProtocolIngestRecord {
        let mut record = ProtocolIngestRecord::new(
            self.protocol_id.clone(),
            self.plugin_id.clone(),
            self.device_id.clone().unwrap_or_default(),
            protocol_ingest_action(self.action),
            self.pipeline,
        );

        if let Some(client_id) = &self.client_id {
            record = record.with_client_id(client_id.clone());
        }
        if let Some(session_id) = &self.session_id {
            record = record.with_session_id(session_id.clone());
        }
        if let Some(trace_id) = &self.trace_id {
            record = record.with_trace_id(trace_id.clone());
        }
        if let Some(media_resource_id) = &self.media_resource_id {
            record = record.with_media_reference(
                media_resource_id.clone(),
                self.object_blob_id.clone(),
                self.media_resource_snapshot.clone(),
            );
        }

        record
    }

    pub fn to_core_ingest_plan(&self) -> ProtocolIngestPlan {
        protocol_ingest_plan(&self.to_core_ingest_record())
    }

    pub fn to_storage_command(&self) -> AiotProtocolStorageCommand {
        let ingest_record = self.to_core_ingest_record();
        let ingest_plan = protocol_ingest_plan(&ingest_record);
        let write_kind = storage_write_kind(self.action);

        let mut command = AiotProtocolStorageCommand::new(
            self.protocol_id.clone(),
            self.plugin_id.clone(),
            self.device_id.clone().unwrap_or_default(),
            write_kind,
            ingest_plan.primary_table,
        );

        if let Some(session_id) = &self.session_id {
            command = command.with_session_id(session_id.clone());
        }
        if let Some(message_id) = &self.message_id {
            command = command.with_message_id(message_id.clone());
        }
        if let Some(correlation_id) = &self.correlation_id {
            command = command.with_correlation_id(correlation_id.clone());
        }
        if let Some(idempotency_key) = &self.idempotency_key {
            command = command.with_idempotency_key(idempotency_key.clone());
        }
        if let Some(trace_id) = &self.trace_id {
            command = command.with_trace_id(trace_id.clone());
        }
        if let Some(media_resource_id) = &self.media_resource_id {
            command = command.with_media_reference(
                media_resource_id.clone(),
                self.object_blob_id.clone(),
                self.media_resource_snapshot.clone(),
            );
        }
        if ingest_plan.emit_outbox_event {
            let aggregate_type = storage_aggregate_type(self.action);
            let aggregate_id = storage_aggregate_id(
                aggregate_type,
                self.device_id.as_deref(),
                self.session_id.as_deref(),
            );
            command = command.with_outbox(
                AiotOutboxWriteIntent::new(
                    ingest_plan.event_kind.event_type(),
                    aggregate_type,
                    aggregate_id,
                    ingest_plan.outbox_topic,
                )
                .with_event_version("1")
                .with_payload_json(self.standard_outbox_payload_json())
                .with_payload_hash(self.standard_outbox_payload_hash()),
            );
        }

        command
    }

    pub fn to_storage_command_with_context(
        &self,
        ctx: &AiotRequestContext,
    ) -> Result<AiotProtocolStorageCommand, RuntimeProtocolError> {
        let association = storage_association_from_context(ctx)?;

        Ok(self.to_storage_command().with_association(association))
    }

    fn standard_outbox_payload_json(&self) -> String {
        format!(
            r#"{{"eventVersion":"1","protocolId":"{}","pluginId":"{}","deviceId":"{}","sessionId":"{}","messageClass":"{}","semanticType":"{}","traceId":"{}","mediaResourceId":"{}","objectBlobId":"{}"}}"#,
            json_escape(&self.protocol_id),
            json_escape(&self.plugin_id),
            json_escape(&self.device_id.clone().unwrap_or_default()),
            json_escape(&self.session_id.clone().unwrap_or_default()),
            message_class_name(self.message_class),
            json_escape(&self.semantic_type),
            json_escape(&self.trace_id.clone().unwrap_or_default()),
            json_escape(&self.media_resource_id.clone().unwrap_or_default()),
            json_escape(&self.object_blob_id.clone().unwrap_or_default()),
        )
    }

    fn standard_outbox_payload_hash(&self) -> String {
        sdkwork_utils_rust::sha256_hash(self.standard_outbox_payload_json().as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProtocolError {
    pub code: String,
}

impl RuntimeProtocolError {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

fn storage_association_from_context(
    ctx: &AiotRequestContext,
) -> Result<AiotStorageAssociation, RuntimeProtocolError> {
    let tenant_id = parse_context_i64(&ctx.tenant_id, "runtime.context.invalid_tenant_id")?;
    let organization_id = parse_context_i64(
        &ctx.organization_id,
        "runtime.context.invalid_organization_id",
    )?;
    let mut association = AiotStorageAssociation::tenant_org(tenant_id, organization_id);

    if let Some(user_id) = &ctx.user_id {
        association = association.with_user_id(parse_context_i64(
            user_id,
            "runtime.context.invalid_user_id",
        )?);
    }

    if let Some(data_scope) = ctx.data_scope.first() {
        association = association.with_data_scope(parse_context_i32(
            data_scope,
            "runtime.context.invalid_data_scope",
        )?);
    }

    Ok(association)
}

fn parse_context_i64(value: &str, error_code: &'static str) -> Result<i64, RuntimeProtocolError> {
    value
        .trim()
        .parse::<i64>()
        .map_err(|_| RuntimeProtocolError::new(error_code))
}

fn parse_context_i32(value: &str, error_code: &'static str) -> Result<i32, RuntimeProtocolError> {
    value
        .trim()
        .parse::<i32>()
        .map_err(|_| RuntimeProtocolError::new(error_code))
}

fn protocol_ingest_action(action: AiotProtocolMessageAction) -> ProtocolIngestAction {
    match action {
        AiotProtocolMessageAction::OpenSession => ProtocolIngestAction::OpenSession,
        AiotProtocolMessageAction::Authenticate => ProtocolIngestAction::Authenticate,
        AiotProtocolMessageAction::KeepAlive => ProtocolIngestAction::KeepAlive,
        AiotProtocolMessageAction::CloseSession => ProtocolIngestAction::CloseSession,
        AiotProtocolMessageAction::ProvisionDevice => ProtocolIngestAction::ProvisionDevice,
        AiotProtocolMessageAction::RecordTelemetry => ProtocolIngestAction::RecordTelemetry,
        AiotProtocolMessageAction::ApplyDesiredTwin => ProtocolIngestAction::ApplyDesiredTwin,
        AiotProtocolMessageAction::DispatchCommand => ProtocolIngestAction::DispatchCommand,
        AiotProtocolMessageAction::RecordCommandAck => ProtocolIngestAction::RecordCommandAck,
        AiotProtocolMessageAction::RecordCommandResult => ProtocolIngestAction::RecordCommandResult,
        AiotProtocolMessageAction::ProcessMediaFrame => ProtocolIngestAction::ProcessMediaFrame,
        AiotProtocolMessageAction::EvaluateOta => ProtocolIngestAction::EvaluateOta,
        AiotProtocolMessageAction::DispatchOta => ProtocolIngestAction::DispatchOta,
        AiotProtocolMessageAction::UpdateGatewayTopology => {
            ProtocolIngestAction::UpdateGatewayTopology
        }
        AiotProtocolMessageAction::RecordSecurityEvent => ProtocolIngestAction::RecordSecurityEvent,
        AiotProtocolMessageAction::RecordDiagnostic => ProtocolIngestAction::RecordDiagnostic,
    }
}

fn storage_write_kind(action: AiotProtocolMessageAction) -> AiotStorageWriteKind {
    match action {
        AiotProtocolMessageAction::OpenSession => AiotStorageWriteKind::OpenSession,
        AiotProtocolMessageAction::Authenticate => AiotStorageWriteKind::Authenticate,
        AiotProtocolMessageAction::KeepAlive => AiotStorageWriteKind::KeepAlive,
        AiotProtocolMessageAction::CloseSession => AiotStorageWriteKind::CloseSession,
        AiotProtocolMessageAction::ProvisionDevice => AiotStorageWriteKind::ProvisionDevice,
        AiotProtocolMessageAction::RecordTelemetry => AiotStorageWriteKind::RecordTelemetry,
        AiotProtocolMessageAction::ApplyDesiredTwin => AiotStorageWriteKind::ApplyDesiredTwin,
        AiotProtocolMessageAction::DispatchCommand => AiotStorageWriteKind::DispatchCommand,
        AiotProtocolMessageAction::RecordCommandAck => AiotStorageWriteKind::RecordCommandAck,
        AiotProtocolMessageAction::RecordCommandResult => AiotStorageWriteKind::RecordCommandResult,
        AiotProtocolMessageAction::ProcessMediaFrame => AiotStorageWriteKind::ProcessMediaFrame,
        AiotProtocolMessageAction::EvaluateOta => AiotStorageWriteKind::EvaluateOta,
        AiotProtocolMessageAction::DispatchOta => AiotStorageWriteKind::DispatchOta,
        AiotProtocolMessageAction::UpdateGatewayTopology => {
            AiotStorageWriteKind::UpdateGatewayTopology
        }
        AiotProtocolMessageAction::RecordSecurityEvent => AiotStorageWriteKind::RecordSecurityEvent,
        AiotProtocolMessageAction::RecordDiagnostic => AiotStorageWriteKind::RecordDiagnostic,
    }
}

fn storage_aggregate_type(action: AiotProtocolMessageAction) -> &'static str {
    match action {
        AiotProtocolMessageAction::OpenSession
        | AiotProtocolMessageAction::KeepAlive
        | AiotProtocolMessageAction::CloseSession => "device_session",
        AiotProtocolMessageAction::DispatchCommand
        | AiotProtocolMessageAction::RecordCommandAck
        | AiotProtocolMessageAction::RecordCommandResult => "device_command",
        AiotProtocolMessageAction::EvaluateOta | AiotProtocolMessageAction::DispatchOta => {
            "firmware_deployment"
        }
        AiotProtocolMessageAction::ProvisionDevice => "provisioning_challenge",
        AiotProtocolMessageAction::UpdateGatewayTopology => "edge_gateway",
        AiotProtocolMessageAction::RecordSecurityEvent => "security_event",
        _ => "device",
    }
}

fn storage_aggregate_id(
    aggregate_type: &str,
    device_id: Option<&str>,
    session_id: Option<&str>,
) -> String {
    match aggregate_type {
        "device_session" => session_id.or(device_id).unwrap_or_default().to_string(),
        _ => device_id.or(session_id).unwrap_or_default().to_string(),
    }
}

fn message_class_name(class: MessageClass) -> &'static str {
    match class {
        MessageClass::Handshake => "handshake",
        MessageClass::Auth => "auth",
        MessageClass::Heartbeat => "heartbeat",
        MessageClass::Disconnect => "disconnect",
        MessageClass::Provisioning => "provisioning",
        MessageClass::Telemetry => "telemetry",
        MessageClass::Event => "event",
        MessageClass::PropertyReport => "propertyReport",
        MessageClass::PropertySet => "propertySet",
        MessageClass::TwinDesired => "twinDesired",
        MessageClass::TwinReported => "twinReported",
        MessageClass::CommandRequest => "commandRequest",
        MessageClass::CommandAck => "commandAck",
        MessageClass::CommandResult => "commandResult",
        MessageClass::MediaFrame => "mediaFrame",
        MessageClass::OtaCheck => "otaCheck",
        MessageClass::OtaDeploy => "otaDeploy",
        MessageClass::GatewayTopology => "gatewayTopology",
        MessageClass::SecurityEvent => "securityEvent",
        MessageClass::Diagnostic => "diagnostic",
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn protocol_message_action(
    message_class: MessageClass,
) -> (AiotProtocolMessageAction, &'static str, bool) {
    match message_class {
        MessageClass::Handshake => (
            AiotProtocolMessageAction::OpenSession,
            "device_session",
            true,
        ),
        MessageClass::Auth => (AiotProtocolMessageAction::Authenticate, "device_auth", true),
        MessageClass::Heartbeat => (AiotProtocolMessageAction::KeepAlive, "device_session", true),
        MessageClass::Disconnect => (
            AiotProtocolMessageAction::CloseSession,
            "device_session",
            false,
        ),
        MessageClass::Provisioning => (
            AiotProtocolMessageAction::ProvisionDevice,
            "provisioning",
            true,
        ),
        MessageClass::Telemetry | MessageClass::Event | MessageClass::PropertyReport => (
            AiotProtocolMessageAction::RecordTelemetry,
            "telemetry_ingest",
            false,
        ),
        MessageClass::PropertySet | MessageClass::TwinDesired | MessageClass::TwinReported => (
            AiotProtocolMessageAction::ApplyDesiredTwin,
            "digital_twin",
            true,
        ),
        MessageClass::CommandRequest => (
            AiotProtocolMessageAction::DispatchCommand,
            "command_router",
            true,
        ),
        MessageClass::CommandAck => (
            AiotProtocolMessageAction::RecordCommandAck,
            "command_router",
            false,
        ),
        MessageClass::CommandResult => (
            AiotProtocolMessageAction::RecordCommandResult,
            "command_router",
            false,
        ),
        MessageClass::MediaFrame => (
            AiotProtocolMessageAction::ProcessMediaFrame,
            "media_ingest",
            false,
        ),
        MessageClass::OtaCheck => (AiotProtocolMessageAction::EvaluateOta, "ota", true),
        MessageClass::OtaDeploy => (AiotProtocolMessageAction::DispatchOta, "ota", true),
        MessageClass::GatewayTopology => (
            AiotProtocolMessageAction::UpdateGatewayTopology,
            "gateway_topology",
            true,
        ),
        MessageClass::SecurityEvent => (
            AiotProtocolMessageAction::RecordSecurityEvent,
            "security_event",
            false,
        ),
        MessageClass::Diagnostic => (
            AiotProtocolMessageAction::RecordDiagnostic,
            "diagnostic",
            false,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotProtocolRouteKind {
    DeviceSession,
    OtaMetadata,
    Provisioning,
    BridgeIngress,
    Callback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotProtocolRoute {
    pub path: String,
    pub protocol_id: String,
    pub plugin_id: String,
    pub transport: TransportBinding,
    pub kind: AiotProtocolRouteKind,
    pub capability_bridges: Vec<String>,
}

impl AiotProtocolRoute {
    pub fn new(
        path: impl Into<String>,
        protocol_id: impl Into<String>,
        plugin_id: impl Into<String>,
        transport: TransportBinding,
        kind: AiotProtocolRouteKind,
    ) -> Self {
        Self {
            path: path.into(),
            protocol_id: protocol_id.into(),
            plugin_id: plugin_id.into(),
            transport,
            kind,
            capability_bridges: Vec::new(),
        }
    }

    pub fn with_capability_bridge(mut self, bridge: impl Into<String>) -> Self {
        self.capability_bridges.push(bridge.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeServicePlan {
    pub gateway_routes: Vec<&'static str>,
    pub backend_routes: Vec<&'static str>,
    pub app_routes: Vec<&'static str>,
    pub embedded_mountable: bool,
    pub standalone_startable: bool,
    pub requires_external_iam_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkFamilyContract {
    pub name: &'static str,
    pub package_name: &'static str,
    pub api_prefix: &'static str,
    pub openapi_path: &'static str,
    pub sdkgen_path: &'static str,
    pub sdk_manifest_path: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotIntegrationBundle {
    pub runtime: AiotRuntime,
    pub component_manifest: AiotComponentManifest,
    pub service_plan: RuntimeServicePlan,
    pub capacity_policy: AiotRuntimeCapacityPolicy,
    pub protocol_catalog: Vec<ProtocolCatalogEntry>,
    pub sdk_families: Vec<SdkFamilyContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotConfig {
    pub app_api_prefix: &'static str,
    pub backend_api_prefix: &'static str,
    pub device_base_path: &'static str,
    pub requires_external_iam_context: bool,
    pub embedded_enabled: bool,
    pub standalone_enabled: bool,
}

impl AiotConfig {
    pub fn standard() -> Self {
        Self {
            app_api_prefix: IOT_APP_API_PREFIX,
            backend_api_prefix: IOT_BACKEND_API_PREFIX,
            device_base_path: IOT_XIAOZHI_BASE_PATH,
            requires_external_iam_context: true,
            embedded_enabled: true,
            standalone_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureAction {
    Accept,
    SlowDown,
    Reject,
    DeadLetterNonCritical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotRuntimePressure {
    pub node_connections: u64,
    pub tenant_sessions: u64,
    pub device_inflight: u64,
    pub outbox_lag: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotRuntimeCapacityPolicy {
    pub node_id: &'static str,
    pub max_connections_per_node: u64,
    pub max_sessions_per_tenant: u64,
    pub max_inflight_per_device: u64,
    pub session_lease_ttl_seconds: u64,
    pub session_lease_renew_seconds: u64,
    pub outbox_warn_lag: u64,
    pub outbox_reject_lag: u64,
    pub outbox_dead_letter_lag: u64,
    pub outbox_max_attempts: u32,
    pub dead_letter_after_attempts: u32,
    pub enable_ordered_device_commands: bool,
    pub enable_idempotent_ingest: bool,
}

impl AiotRuntimeCapacityPolicy {
    pub fn standard() -> Self {
        Self {
            node_id: "local",
            max_connections_per_node: 100_000,
            max_sessions_per_tenant: 1_000_000,
            max_inflight_per_device: 64,
            session_lease_ttl_seconds: 90,
            session_lease_renew_seconds: 30,
            outbox_warn_lag: 100_000,
            outbox_reject_lag: 500_000,
            outbox_dead_letter_lag: 1_000_000,
            outbox_max_attempts: 12,
            dead_letter_after_attempts: 12,
            enable_ordered_device_commands: true,
            enable_idempotent_ingest: true,
        }
    }

    pub fn backpressure_action(&self, pressure: &AiotRuntimePressure) -> BackpressureAction {
        if pressure.outbox_lag > self.outbox_dead_letter_lag {
            return BackpressureAction::DeadLetterNonCritical;
        }
        if pressure.node_connections > self.max_connections_per_node
            || pressure.tenant_sessions > self.max_sessions_per_tenant
            || pressure.device_inflight > self.max_inflight_per_device
            || pressure.outbox_lag > self.outbox_reject_lag
        {
            return BackpressureAction::Reject;
        }
        if pressure.node_connections >= self.max_connections_per_node * 9 / 10
            || pressure.tenant_sessions >= self.max_sessions_per_tenant * 9 / 10
            || pressure.device_inflight >= self.max_inflight_per_device * 9 / 10
            || pressure.outbox_lag >= self.outbox_warn_lag
        {
            return BackpressureAction::SlowDown;
        }

        BackpressureAction::Accept
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotStorageBundle {
    pub component_name: &'static str,
    pub schema_version: &'static str,
    pub migrations_required: bool,
    pub repository_ports: Vec<&'static str>,
}

impl AiotStorageBundle {
    pub fn standard_sqlx() -> Self {
        Self {
            component_name: "sdkwork-aiot-storage-sqlx",
            schema_version: "0.2.0",
            migrations_required: true,
            repository_ports: vec![
                "ProductRepository",
                "DeviceRepository",
                "DeviceSessionRepository",
                "CommandRepository",
                "TwinRepository",
                "TelemetryRepository",
                "FirmwareRepository",
                "OutboxEventRepository",
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotProtocolBundle {
    pub component_name: &'static str,
    pub protocol_ids: Vec<&'static str>,
    pub plugin_required: bool,
}

impl AiotProtocolBundle {
    pub fn standard() -> Self {
        Self {
            component_name: "sdkwork-aiot-protocol",
            protocol_ids: vec![
                "xiaozhi.websocket",
                "xiaozhi.mqtt_udp",
                "mqtt.v3_1_1",
                "mqtt.v5",
                "coap.lwm2m",
                "matter.bridge",
                "zigbee2mqtt.bridge",
                "lorawan.chirpstack",
                "modbus.bridge",
                "opcua.bridge",
                "esphome.native",
                "tasmota.mqtt",
                "wled.mqtt",
                "raspberrypi.linux_gateway",
                "raspberrypi.pico_mqtt",
                "openbeken.mqtt",
            ],
            plugin_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotHttpRouteBundle {
    pub app_routes: Vec<&'static str>,
    pub backend_routes: Vec<&'static str>,
}

impl AiotHttpRouteBundle {
    pub fn standard() -> Self {
        let plan = RuntimeServicePlan::standard();
        Self {
            app_routes: plan.app_routes,
            backend_routes: plan.backend_routes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotGatewayListenerBundle {
    pub websocket_routes: Vec<&'static str>,
    pub tcp_listeners: Vec<&'static str>,
    pub udp_listeners: Vec<&'static str>,
    pub mqtt_bindings: Vec<&'static str>,
    pub supports_socket: bool,
}

impl AiotGatewayListenerBundle {
    pub fn standard() -> Self {
        Self {
            websocket_routes: vec!["/iot/xiaozhi/ws"],
            tcp_listeners: vec!["tcp.protocol_gateway"],
            udp_listeners: vec!["udp.media"],
            mqtt_bindings: vec!["mqtt.bridge"],
            supports_socket: true,
        }
    }
}

#[derive(Clone)]
pub struct AiotHealthCheck {
    pub component_name: String,
    pub ready: bool,
    pub details: Vec<&'static str>,
    readiness_probe: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl std::fmt::Debug for AiotHealthCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiotHealthCheck")
            .field("component_name", &self.component_name)
            .field("ready", &self.ready)
            .field("details", &self.details)
            .field("has_readiness_probe", &self.readiness_probe.is_some())
            .finish()
    }
}

impl PartialEq for AiotHealthCheck {
    fn eq(&self, other: &Self) -> bool {
        self.component_name == other.component_name
            && self.ready == other.ready
            && self.details == other.details
            && self.readiness_probe.is_some() == other.readiness_probe.is_some()
    }
}

impl Eq for AiotHealthCheck {}

impl AiotHealthCheck {
    pub fn ready(component_name: impl Into<String>) -> Self {
        Self {
            component_name: component_name.into(),
            ready: true,
            details: vec!["runtime_builder", "protocol_registry", "component_manifest"],
            readiness_probe: None,
        }
    }

    pub fn with_readiness_probe(
        mut self,
        probe: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Self {
        self.readiness_probe = Some(Arc::new(probe));
        self
    }

    pub fn and_readiness_probe(mut self, probe: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        let next_probe = Arc::new(probe);
        self.readiness_probe = Some(match self.readiness_probe.take() {
            None => next_probe,
            Some(existing) => Arc::new(move || existing() && next_probe()),
        });
        self
    }

    pub fn is_ready(&self) -> bool {
        self.ready && self.readiness_probe.as_ref().is_none_or(|probe| probe())
    }
}

impl RuntimeServicePlan {
    pub fn standard() -> Self {
        Self {
            gateway_routes: vec![
                "/iot/xiaozhi/ws",
                "/iot/xiaozhi/ota",
                "/iot/xiaozhi/activate",
            ],
            backend_routes: vec![
                "/backend/v3/api/iot/products",
                "/backend/v3/api/iot/hardware_profiles",
                "/backend/v3/api/iot/protocol_profiles",
                "/backend/v3/api/iot/capability_models/{capabilityModelId}",
                "/backend/v3/api/iot/devices",
                "/backend/v3/api/iot/devices/{deviceId}",
                "/backend/v3/api/iot/devices/{deviceId}/credentials",
                "/backend/v3/api/iot/devices/{deviceId}/sessions",
                "/backend/v3/api/iot/devices/{deviceId}/capabilities",
                "/backend/v3/api/iot/devices/{deviceId}/commands",
                "/backend/v3/api/iot/devices/{deviceId}/twin",
                "/backend/v3/api/iot/firmware_artifacts",
                "/backend/v3/api/iot/firmware_rollouts",
                "/backend/v3/api/iot/events",
                "/backend/v3/api/iot/protocol_adapters",
                "/backend/v3/api/iot/runtime/capacity",
            ],
            app_routes: vec![
                "/app/v3/api/iot/devices",
                "/app/v3/api/iot/devices/{deviceId}",
                "/app/v3/api/iot/devices/{deviceId}/commands",
                "/app/v3/api/iot/devices/{deviceId}/twin",
                "/app/v3/api/iot/devices/{deviceId}/events",
            ],
            embedded_mountable: true,
            standalone_startable: true,
            requires_external_iam_context: true,
        }
    }

    pub fn route_prefixes(&self) -> (&'static str, &'static str, &'static str) {
        (
            IOT_APP_API_PREFIX,
            IOT_BACKEND_API_PREFIX,
            IOT_XIAOZHI_BASE_PATH,
        )
    }
}

pub fn standard_aiot_runtime(mode: RuntimeMode) -> Result<AiotRuntime, RuntimeBuildError> {
    let xiaozhi = ProtocolAdapterManifest::new("xiaozhi", env!("CARGO_PKG_VERSION"))
        .with_scope(ProtocolPluginScope::CompatibilityPlugin)
        .with_protocol("xiaozhi.websocket")
        .with_protocol("xiaozhi.mqtt_udp")
        .with_transport(TransportBinding::WebSocket)
        .with_transport(TransportBinding::Http)
        .with_transport(TransportBinding::Mqtt)
        .with_transport(TransportBinding::Udp)
        .with_codec(CodecKind::JsonText)
        .with_codec(CodecKind::JsonRpc)
        .with_codec(CodecKind::BinaryMedia)
        .with_session_policy(SessionPolicy::StatefulDeviceSession)
        .with_capability_bridge("mcp_jsonrpc")
        .with_security_mode(DeviceAuthMode::BearerToken.manifest_name())
        .with_security_mode(DeviceAuthMode::Hmac.manifest_name())
        .with_ota_profile("xiaozhi_ota")
        .with_provisioning_profile("xiaozhi_activation")
        .with_hardware_family("esp32")
        .with_hardware_family("esp32_s3")
        .with_runtime_profile("esp_idf")
        .with_runtime_profile("freertos")
        .with_firmware_profile("xiaozhi_ota");

    AiotRuntime::builder()
        .with_mode(mode)
        .with_component_kind(aiot_component_manifest(), ComponentKind::Runtime)
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-aiot-contract", "iot")
                .with_capability("public_contracts")
                .with_required_feature("openapi_sdkwork_v3"),
            ComponentKind::Contract,
        )
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-iot-device-service", "iot")
                .with_capability("ddd_aggregates")
                .with_capability("domain_services"),
            ComponentKind::DomainCore,
        )
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-aiot-storage", "iot")
                .with_capability("repository_ports")
                .with_capability("schema_contracts"),
            ComponentKind::StoragePort,
        )
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-aiot-security", "iot")
                .with_capability("device_principal")
                .with_capability("device_auth_ports"),
            ComponentKind::SecurityPort,
        )
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-aiot-observability", "iot")
                .with_capability("safe_trace_fields")
                .with_capability("redaction"),
            ComponentKind::Observability,
        )
        .with_component_kind(
            AiotComponentManifest::new("sdkwork-aiot-adapter-xiaozhi", "iot")
                .with_capability("xiaozhi_compatibility_plugin"),
            ComponentKind::ProtocolAdapter,
        )
        .register_protocol_adapter(xiaozhi)
        .register_protocol_route(
            AiotProtocolRoute::new(
                "/iot/xiaozhi/ws",
                "xiaozhi.websocket",
                "xiaozhi",
                TransportBinding::WebSocket,
                AiotProtocolRouteKind::DeviceSession,
            )
            .with_capability_bridge("mcp_jsonrpc"),
        )
        .register_protocol_route(
            AiotProtocolRoute::new(
                "/iot/xiaozhi/ota",
                "xiaozhi.websocket",
                "xiaozhi",
                TransportBinding::Http,
                AiotProtocolRouteKind::OtaMetadata,
            )
            .with_capability_bridge("firmware_ota"),
        )
        .register_protocol_route(
            AiotProtocolRoute::new(
                "/iot/xiaozhi/activate",
                "xiaozhi.websocket",
                "xiaozhi",
                TransportBinding::Http,
                AiotProtocolRouteKind::Provisioning,
            )
            .with_capability_bridge("device_provisioning"),
        )
        .register_protocol_route(
            AiotProtocolRoute::new(
                "/iot/xiaozhi/mqtt",
                "xiaozhi.mqtt_udp",
                "xiaozhi",
                TransportBinding::Mqtt,
                AiotProtocolRouteKind::DeviceSession,
            )
            .with_capability_bridge("mcp_jsonrpc"),
        )
        .register_protocol_route(
            AiotProtocolRoute::new(
                "/iot/xiaozhi/udp",
                "xiaozhi.mqtt_udp",
                "xiaozhi",
                TransportBinding::Udp,
                AiotProtocolRouteKind::BridgeIngress,
            )
            .with_capability_bridge("media_ingest"),
        )
        .build()
}

pub fn standard_sdk_families() -> Vec<SdkFamilyContract> {
    vec![
        SdkFamilyContract {
            name: "app",
            package_name: "@sdkwork/aiot-app-sdk",
            api_prefix: IOT_APP_API_PREFIX,
            openapi_path: "apis/app-api/iot/sdkwork-aiot-app-api.openapi.json",
            sdkgen_path: "sdks/sdkwork-aiot-app-sdk/openapi/sdkwork-aiot-app-sdk.sdkgen.json",
            sdk_manifest_path: "sdks/sdkwork-aiot-app-sdk/sdk-manifest.json",
        },
        SdkFamilyContract {
            name: "backend",
            package_name: "@sdkwork/aiot-backend-sdk",
            api_prefix: IOT_BACKEND_API_PREFIX,
            openapi_path: "apis/backend-api/iot/sdkwork-aiot-backend-api.openapi.json",
            sdkgen_path:
                "sdks/sdkwork-aiot-backend-sdk/openapi/sdkwork-aiot-backend-sdk.sdkgen.json",
            sdk_manifest_path: "sdks/sdkwork-aiot-backend-sdk/sdk-manifest.json",
        },
    ]
}

pub fn standard_aiot_integration_bundle(
    mode: RuntimeMode,
) -> Result<AiotIntegrationBundle, RuntimeBuildError> {
    Ok(AiotIntegrationBundle {
        runtime: standard_aiot_runtime(mode)?,
        component_manifest: aiot_component_manifest(),
        service_plan: RuntimeServicePlan::standard(),
        capacity_policy: AiotRuntimeCapacityPolicy::standard(),
        protocol_catalog: standard_protocol_catalog(),
        sdk_families: standard_sdk_families(),
    })
}

#[cfg(test)]
mod health_check_tests {
    use super::AiotHealthCheck;

    #[test]
    fn and_readiness_probe_requires_all_probes_to_pass() {
        let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ready_for_probe = std::sync::Arc::clone(&ready);
        let health = AiotHealthCheck::ready("probe-chain")
            .with_readiness_probe(move || {
                ready_for_probe.load(std::sync::atomic::Ordering::Relaxed)
            })
            .and_readiness_probe(|| true);
        assert!(health.is_ready());

        ready.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(!health.is_ready());
    }
}
