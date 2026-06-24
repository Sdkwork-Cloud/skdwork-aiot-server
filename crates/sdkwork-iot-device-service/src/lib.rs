use std::collections::BTreeMap;

use sdkwork_aiot_contract::AiotOwnershipRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub product_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareClass {
    Unspecified,
    Mcu,
    LinuxSbc,
    EdgeGateway,
    IndustrialController,
    CameraDevice,
    AudioDevice,
    CellularModule,
    BridgeAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareProfile {
    pub profile_id: String,
    pub chip_family: String,
    pub hardware_class: HardwareClass,
    pub hardware_classes: Vec<HardwareClass>,
    pub runtime_profiles: Vec<String>,
    pub connectivity_profiles: Vec<String>,
    pub security_profiles: Vec<String>,
    pub ota_profiles: Vec<String>,
}

impl HardwareProfile {
    pub fn new(profile_id: impl Into<String>, chip_family: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            chip_family: chip_family.into(),
            hardware_class: HardwareClass::Unspecified,
            hardware_classes: Vec::new(),
            runtime_profiles: Vec::new(),
            connectivity_profiles: Vec::new(),
            security_profiles: Vec::new(),
            ota_profiles: Vec::new(),
        }
    }

    pub fn with_hardware_class(mut self, hardware_class: HardwareClass) -> Self {
        if self.hardware_class == HardwareClass::Unspecified {
            self.hardware_class = hardware_class;
        }
        if !self.hardware_classes.contains(&hardware_class) {
            self.hardware_classes.push(hardware_class);
        }
        self
    }

    pub fn with_runtime(mut self, runtime_profile: impl Into<String>) -> Self {
        self.runtime_profiles.push(runtime_profile.into());
        self
    }

    pub fn with_connectivity(mut self, connectivity_profile: impl Into<String>) -> Self {
        self.connectivity_profiles.push(connectivity_profile.into());
        self
    }

    pub fn with_security_profile(mut self, security_profile: impl Into<String>) -> Self {
        self.security_profiles.push(security_profile.into());
        self
    }

    pub fn with_ota_profile(mut self, ota_profile: impl Into<String>) -> Self {
        self.ota_profiles.push(ota_profile.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolProfile {
    pub profile_id: String,
    pub default_protocol_id: String,
    pub allowed_transports: Vec<String>,
    pub allowed_message_classes: Vec<String>,
}

impl ProtocolProfile {
    pub fn new(profile_id: impl Into<String>, default_protocol_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            default_protocol_id: default_protocol_id.into(),
            allowed_transports: Vec::new(),
            allowed_message_classes: Vec::new(),
        }
    }

    pub fn allow_transport(mut self, transport: impl Into<String>) -> Self {
        self.allowed_transports.push(transport.into());
        self
    }

    pub fn allow_message_class(mut self, message_class: impl Into<String>) -> Self {
        self.allowed_message_classes.push(message_class.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    Property,
    Command,
    Event,
    Telemetry,
    Media,
    Ota,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub name: String,
    pub kind: CapabilityKind,
    pub commands: Vec<String>,
    pub events: Vec<String>,
    pub protocol_mappings: BTreeMap<String, String>,
}

impl CapabilityDefinition {
    pub fn new(name: impl Into<String>, kind: CapabilityKind) -> Self {
        Self {
            name: name.into(),
            kind,
            commands: Vec::new(),
            events: Vec::new(),
            protocol_mappings: BTreeMap::new(),
        }
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.commands.push(command.into());
        self
    }

    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.events.push(event.into());
        self
    }

    pub fn with_protocol_mapping(
        mut self,
        protocol_id: impl Into<String>,
        mapped_name: impl Into<String>,
    ) -> Self {
        self.protocol_mappings
            .insert(protocol_id.into(), mapped_name.into());
        self
    }
}

impl Product {
    pub fn new(product_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            product_id: product_id.into(),
            display_name: display_name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub device_id: String,
    pub device_key: String,
    pub product_id: String,
    pub owner: AiotOwnershipRef,
}

impl Device {
    pub fn new(
        device_id: impl Into<String>,
        device_key: impl Into<String>,
        product_id: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            device_key: device_key.into(),
            product_id: product_id.into(),
            owner: AiotOwnershipRef::tenant(""),
        }
    }

    pub fn with_owner(mut self, owner: AiotOwnershipRef) -> Self {
        self.owner = owner;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Created,
    Dispatched,
    Acknowledged,
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCommand {
    pub command_id: String,
    pub device_id: String,
    pub capability_name: String,
    pub command_name: String,
    pub status: CommandStatus,
    pub session_id: Option<String>,
    pub result_payload: Option<String>,
}

impl DeviceCommand {
    pub fn new(
        command_id: impl Into<String>,
        device_id: impl Into<String>,
        capability_name: impl Into<String>,
        command_name: impl Into<String>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            device_id: device_id.into(),
            capability_name: capability_name.into(),
            command_name: command_name.into(),
            status: CommandStatus::Created,
            session_id: None,
            result_payload: None,
        }
    }

    pub fn mark_dispatched(mut self, session_id: impl Into<String>) -> Self {
        self.status = CommandStatus::Dispatched;
        self.session_id = Some(session_id.into());
        self
    }

    pub fn mark_acknowledged(mut self) -> Self {
        self.status = CommandStatus::Acknowledged;
        self
    }

    pub fn mark_succeeded(mut self, result_payload: impl Into<String>) -> Self {
        self.status = CommandStatus::Succeeded;
        self.result_payload = Some(result_payload.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolIngestAction {
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
pub struct ProtocolIngestRecord {
    pub protocol_id: String,
    pub plugin_id: String,
    pub device_id: String,
    pub client_id: Option<String>,
    pub session_id: Option<String>,
    pub action: ProtocolIngestAction,
    pub pipeline: String,
    pub trace_id: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_resource_snapshot: Option<String>,
}

impl ProtocolIngestRecord {
    pub fn new(
        protocol_id: impl Into<String>,
        plugin_id: impl Into<String>,
        device_id: impl Into<String>,
        action: ProtocolIngestAction,
        pipeline: impl Into<String>,
    ) -> Self {
        Self {
            protocol_id: protocol_id.into(),
            plugin_id: plugin_id.into(),
            device_id: device_id.into(),
            client_id: None,
            session_id: None,
            action,
            pipeline: pipeline.into(),
            trace_id: None,
            media_resource_id: None,
            object_blob_id: None,
            media_resource_snapshot: None,
        }
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_media_reference(
        mut self,
        media_resource_id: impl Into<String>,
        object_blob_id: Option<String>,
        media_resource_snapshot: Option<String>,
    ) -> Self {
        self.media_resource_id = Some(media_resource_id.into());
        self.object_blob_id = object_blob_id;
        self.media_resource_snapshot = media_resource_snapshot;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainEventKind {
    DeviceSessionOpened,
    DeviceAuthenticated,
    DeviceHeartbeatObserved,
    DeviceSessionClosed,
    DeviceProvisioningRequested,
    TelemetryRecorded,
    TwinUpdateRequested,
    CommandDispatchRequested,
    CommandAcknowledged,
    CommandResultRecorded,
    MediaFrameReceived,
    OtaCheckRequested,
    OtaDeploymentCompleted,
    GatewayTopologyUpdated,
    SecurityEventRecorded,
    DiagnosticRecorded,
}

impl DomainEventKind {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::DeviceSessionOpened => "iot.device.session.started",
            Self::DeviceAuthenticated => "iot.device.authenticated",
            Self::DeviceHeartbeatObserved => "iot.device.heartbeat.observed",
            Self::DeviceSessionClosed => "iot.device.session.ended",
            Self::DeviceProvisioningRequested => "iot.device.provisioning.requested",
            Self::TelemetryRecorded => "iot.telemetry.received",
            Self::TwinUpdateRequested => "iot.twin.update.requested",
            Self::CommandDispatchRequested => "iot.command.dispatchRequested",
            Self::CommandAcknowledged => "iot.command.acknowledged",
            Self::CommandResultRecorded => "iot.command.resultRecorded",
            Self::MediaFrameReceived => "iot.media.frameReceived",
            Self::OtaCheckRequested => "iot.firmware.otaCheckRequested",
            Self::OtaDeploymentCompleted => "iot.firmware.deployment.completed",
            Self::GatewayTopologyUpdated => "iot.gateway.topology.updated",
            Self::SecurityEventRecorded => "iot.security.eventRecorded",
            Self::DiagnosticRecorded => "iot.diagnostic.recorded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolIngestPlan {
    pub event_kind: DomainEventKind,
    pub primary_table: &'static str,
    pub outbox_topic: &'static str,
    pub emit_outbox_event: bool,
}

pub fn protocol_ingest_plan(record: &ProtocolIngestRecord) -> ProtocolIngestPlan {
    let (event_kind, primary_table, emit_outbox_event) = match record.action {
        ProtocolIngestAction::OpenSession => (
            DomainEventKind::DeviceSessionOpened,
            "iot_device_session",
            true,
        ),
        ProtocolIngestAction::Authenticate => (
            DomainEventKind::DeviceAuthenticated,
            "iot_device_credential",
            true,
        ),
        ProtocolIngestAction::KeepAlive => (
            DomainEventKind::DeviceHeartbeatObserved,
            "iot_device_online_lease",
            false,
        ),
        ProtocolIngestAction::CloseSession => (
            DomainEventKind::DeviceSessionClosed,
            "iot_device_session",
            true,
        ),
        ProtocolIngestAction::ProvisionDevice => (
            DomainEventKind::DeviceProvisioningRequested,
            "iot_provisioning_challenge",
            true,
        ),
        ProtocolIngestAction::RecordTelemetry => (
            DomainEventKind::TelemetryRecorded,
            "iot_telemetry_event",
            true,
        ),
        ProtocolIngestAction::ApplyDesiredTwin => (
            DomainEventKind::TwinUpdateRequested,
            "iot_device_twin_property",
            true,
        ),
        ProtocolIngestAction::DispatchCommand => (
            DomainEventKind::CommandDispatchRequested,
            "iot_command_delivery",
            true,
        ),
        ProtocolIngestAction::RecordCommandAck => (
            DomainEventKind::CommandAcknowledged,
            "iot_command_delivery",
            true,
        ),
        ProtocolIngestAction::RecordCommandResult => (
            DomainEventKind::CommandResultRecorded,
            "iot_command_result",
            true,
        ),
        ProtocolIngestAction::ProcessMediaFrame => (
            DomainEventKind::MediaFrameReceived,
            "iot_device_event",
            true,
        ),
        ProtocolIngestAction::EvaluateOta => (
            DomainEventKind::OtaCheckRequested,
            "iot_firmware_deployment",
            true,
        ),
        ProtocolIngestAction::DispatchOta => (
            DomainEventKind::OtaDeploymentCompleted,
            "iot_firmware_deployment",
            true,
        ),
        ProtocolIngestAction::UpdateGatewayTopology => (
            DomainEventKind::GatewayTopologyUpdated,
            "iot_gateway_child_device",
            true,
        ),
        ProtocolIngestAction::RecordSecurityEvent => (
            DomainEventKind::SecurityEventRecorded,
            "iot_security_event",
            true,
        ),
        ProtocolIngestAction::RecordDiagnostic => (
            DomainEventKind::DiagnosticRecorded,
            "iot_device_event",
            true,
        ),
    };

    ProtocolIngestPlan {
        event_kind,
        primary_table,
        outbox_topic: "iot.protocol.ingested",
        emit_outbox_event,
    }
}
