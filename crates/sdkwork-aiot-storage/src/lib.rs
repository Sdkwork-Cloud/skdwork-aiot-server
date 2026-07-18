use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

mod outbox;
mod pagination;

pub use outbox::{
    AiotOutboxPendingEvent, OutboxEventRepository, OutboxEventRepositoryError,
    OUTBOX_STATUS_CLAIMED, OUTBOX_STATUS_FAILED, OUTBOX_STATUS_PENDING, OUTBOX_STATUS_PUBLISHED,
};
pub use pagination::{
    offset_list_page_info, paginate_bounded_catalog, paginate_vec, AiotOffsetListResult,
    OffsetListPageParams, DEFAULT_LIST_PAGE_SIZE, MAX_LIST_PAGE_SIZE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiotTable {
    pub name: &'static str,
    pub group: &'static str,
}

pub const IOT_DATABASE_PREFIX: &str = "iot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableProfile {
    TenantOwnerEntity,
    TenantEntity,
    RelationEntity,
    RuntimeFact,
    Projection,
    EventLog,
    OutboxEvent,
    InboxEvent,
    AuditLog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotTableContract {
    pub name: &'static str,
    pub group: &'static str,
    pub profile: TableProfile,
    pub system_of_record: bool,
    pub write_owner: &'static str,
    pub required_columns: Vec<&'static str>,
    pub required_indexes: Vec<&'static str>,
}

impl AiotTableContract {
    pub fn new(name: &'static str, group: &'static str, profile: TableProfile) -> Self {
        Self {
            name,
            group,
            profile,
            system_of_record: true,
            write_owner: "sdkwork-iot-device-service",
            required_columns: vec![
                "id",
                "uuid",
                "tenant_id",
                "organization_id",
                "data_scope",
                "created_at",
                "updated_at",
                "version",
                "status",
            ],
            required_indexes: Vec::new(),
        }
    }

    pub fn read_model(mut self) -> Self {
        self.system_of_record = false;
        self
    }

    pub fn with_write_owner(mut self, write_owner: &'static str) -> Self {
        self.write_owner = write_owner;
        self
    }

    pub fn with_column(mut self, column: &'static str) -> Self {
        self.required_columns.push(column);
        self
    }

    pub fn with_index(mut self, index: &'static str) -> Self {
        self.required_indexes.push(index);
        self
    }
}

impl AiotTable {
    pub const fn new(name: &'static str, group: &'static str) -> Self {
        Self { name, group }
    }
}

pub const IOT_TABLES: &[AiotTable] = &[
    AiotTable::new("iot_product", "product_catalog"),
    AiotTable::new("iot_hardware_profile", "hardware_profile"),
    AiotTable::new("iot_protocol_profile", "protocol_profile"),
    AiotTable::new("iot_capability_model", "capability_model"),
    AiotTable::new("iot_capability_definition", "capability_model"),
    AiotTable::new("iot_device", "device_registry"),
    AiotTable::new("iot_device_credential", "device_registry"),
    AiotTable::new("iot_device_binding", "device_registry"),
    AiotTable::new("iot_gateway_child_device", "edge_gateway"),
    AiotTable::new("iot_device_connection", "session_runtime"),
    AiotTable::new("iot_device_session", "session_runtime"),
    AiotTable::new("iot_device_online_lease", "session_runtime"),
    AiotTable::new("iot_command", "command_control"),
    AiotTable::new("iot_command_delivery", "command_control"),
    AiotTable::new("iot_command_result", "command_control"),
    AiotTable::new("iot_device_twin", "device_twin"),
    AiotTable::new("iot_device_twin_property", "device_twin"),
    AiotTable::new("iot_telemetry_latest", "telemetry_event"),
    AiotTable::new("iot_telemetry_event", "telemetry_event"),
    AiotTable::new("iot_device_event", "telemetry_event"),
    AiotTable::new("iot_security_event", "telemetry_event"),
    AiotTable::new("iot_media_resource", "media_resource"),
    AiotTable::new("iot_device_media", "media_resource"),
    AiotTable::new("iot_firmware_artifact", "ota_provisioning"),
    AiotTable::new("iot_firmware_rollout", "ota_provisioning"),
    AiotTable::new("iot_firmware_rollout_target", "ota_provisioning"),
    AiotTable::new("iot_firmware_deployment", "ota_provisioning"),
    AiotTable::new("iot_provisioning_challenge", "ota_provisioning"),
    AiotTable::new("iot_activation_record", "ota_provisioning"),
    AiotTable::new("iot_outbox_event", "eventing"),
    AiotTable::new("iot_inbox_event", "eventing"),
    AiotTable::new("iot_audit_log", "eventing"),
    AiotTable::new("iot_protocol_ingest_record", "protocol_runtime"),
    AiotTable::new("iot_protocol_message_dead_letter", "protocol_runtime"),
];

pub fn standard_protocol_ingest_storage_ports() -> Vec<&'static str> {
    vec![
        "DeviceRepository",
        "DeviceSessionRepository",
        "DeviceOnlineLeaseRepository",
        "DeviceCredentialRepository",
        "TelemetryRepository",
        "DeviceEventRepository",
        "DeviceTwinRepository",
        "CommandDeliveryRepository",
        "CommandResultRepository",
        "FirmwareDeploymentRepository",
        "ProvisioningChallengeRepository",
        "GatewayTopologyRepository",
        "SecurityEventRepository",
        "ProtocolDeadLetterRepository",
        "OutboxEventRepository",
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceRecord {
    pub id: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub device_id: String,
    pub display_name: String,
    pub product_id: String,
    pub client_id: Option<String>,
    pub chip_family: Option<String>,
    pub status: String,
    pub metadata_json: Option<String>,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceCreateCommand {
    pub association: AiotStorageAssociation,
    pub device_id: String,
    pub display_name: String,
    pub product_id: String,
    pub client_id: Option<String>,
    pub chip_family: Option<String>,
}

impl AiotDeviceCreateCommand {
    pub fn new(
        association: AiotStorageAssociation,
        device_id: impl Into<String>,
        display_name: impl Into<String>,
        product_id: impl Into<String>,
    ) -> Self {
        Self {
            association,
            device_id: device_id.into(),
            display_name: display_name.into(),
            product_id: product_id.into(),
            client_id: None,
            chip_family: None,
        }
    }

    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    pub fn with_chip_family(mut self, chip_family: impl Into<String>) -> Self {
        self.chip_family = Some(chip_family.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceUpdateCommand {
    pub association: AiotStorageAssociation,
    pub device_id: String,
    pub display_name: Option<String>,
    pub status: Option<String>,
    pub metadata_json: Option<String>,
}

impl AiotDeviceUpdateCommand {
    pub fn new(association: AiotStorageAssociation, device_id: impl Into<String>) -> Self {
        Self {
            association,
            device_id: device_id.into(),
            display_name: None,
            status: None,
            metadata_json: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn with_metadata_json(mut self, metadata_json: impl Into<String>) -> Self {
        self.metadata_json = Some(metadata_json.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotDeviceRepositoryError {
    DuplicateDeviceId,
    InvalidProductId,
    NotFound,
    PersistenceFailure,
}

pub trait AiotDeviceRepository: Send + Sync {
    fn storage_ready(&self) -> bool {
        true
    }

    fn create_device(
        &self,
        command: AiotDeviceCreateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError>;
    fn get_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Option<AiotDeviceRecord>;
    fn list_devices(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceRecord>, AiotDeviceRepositoryError>;
    /// Rollout/batch targeting: returns device ids only, optionally filtered by product, bounded by limit.
    fn list_device_ids_for_rollout(
        &self,
        association: &AiotStorageAssociation,
        product_id: Option<&str>,
        page_size: Option<i64>,
    ) -> Result<Vec<String>, AiotDeviceRepositoryError>;
    fn update_device(
        &self,
        command: AiotDeviceUpdateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError>;
    fn delete_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<(), AiotDeviceRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotCommandResultRecord {
    pub result_code: Option<String>,
    pub result_payload_json: Option<String>,
    pub result_media_resource_id: Option<String>,
    pub result_object_blob_id: Option<String>,
    pub result_media_json: Option<String>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotCommandRecord {
    pub id: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub command_id: String,
    pub device_id: String,
    pub session_id: Option<String>,
    pub capability_name: String,
    pub command_name: String,
    pub request_payload_json: String,
    pub request_media_resource_id: Option<String>,
    pub request_object_blob_id: Option<String>,
    pub request_media_json: Option<String>,
    pub status: String,
    pub trace_id: Option<String>,
    pub timeout_at: Option<String>,
    pub ack_at: Option<String>,
    pub result_at: Option<String>,
    pub created_at: String,
    pub result: Option<AiotCommandResultRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotCommandCreateCommand {
    pub association: AiotStorageAssociation,
    pub command_id: Option<String>,
    pub device_id: String,
    pub session_id: Option<String>,
    pub capability_name: String,
    pub command_name: String,
    pub request_payload_json: String,
    pub request_media_resource_id: Option<String>,
    pub request_object_blob_id: Option<String>,
    pub request_media_json: Option<String>,
    pub status: String,
    pub trace_id: Option<String>,
    pub timeout_at: Option<String>,
    pub idempotency_key: Option<String>,
}

impl AiotCommandCreateCommand {
    pub fn new(
        association: AiotStorageAssociation,
        device_id: impl Into<String>,
        capability_name: impl Into<String>,
        command_name: impl Into<String>,
    ) -> Self {
        Self {
            association,
            command_id: None,
            device_id: device_id.into(),
            session_id: None,
            capability_name: capability_name.into(),
            command_name: command_name.into(),
            request_payload_json: "{}".to_string(),
            request_media_resource_id: None,
            request_object_blob_id: None,
            request_media_json: None,
            status: "accepted".to_string(),
            trace_id: None,
            timeout_at: None,
            idempotency_key: None,
        }
    }

    pub fn with_command_id(mut self, command_id: impl Into<String>) -> Self {
        self.command_id = Some(command_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_request_payload_json(mut self, request_payload_json: impl Into<String>) -> Self {
        self.request_payload_json = request_payload_json.into();
        self
    }

    pub fn with_request_media(
        mut self,
        request_media_resource_id: Option<String>,
        request_object_blob_id: Option<String>,
        request_media_json: Option<String>,
    ) -> Self {
        self.request_media_resource_id = request_media_resource_id;
        self.request_object_blob_id = request_object_blob_id;
        self.request_media_json = request_media_json;
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_timeout_at(mut self, timeout_at: impl Into<String>) -> Self {
        self.timeout_at = Some(timeout_at.into());
        self
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotCommandRepositoryError {
    DuplicateCommandId,
    PersistenceFailure,
}

pub trait AiotCommandRepository: Send + Sync {
    fn create_command(
        &self,
        command: AiotCommandCreateCommand,
    ) -> Result<AiotCommandRecord, AiotCommandRepositoryError>;
    fn get_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError>;
    fn list_commands(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCommandRecord>, AiotCommandRepositoryError>;
    fn cancel_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceSessionRecord {
    pub session_id: String,
    pub device_id: String,
    pub status: String,
    pub connected_at: Option<String>,
    pub disconnected_at: Option<String>,
    pub transport: String,
    pub protocol_id: String,
    pub adapter_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotCommandDeliveryRecord {
    pub id: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub command_id: String,
    pub session_id: Option<String>,
    pub delivery_state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotCommandDeliveryEnqueueCommand {
    pub association: AiotStorageAssociation,
    pub command_id: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotCommandDeliveryRepositoryError {
    PersistenceFailure,
    CommandNotFound,
}

pub trait AiotCommandDeliveryRepository: Send + Sync {
    fn enqueue_delivery(
        &self,
        command: AiotCommandDeliveryEnqueueCommand,
    ) -> Result<AiotCommandDeliveryRecord, AiotCommandDeliveryRepositoryError>;
    fn list_pending_for_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        limit: i64,
    ) -> Result<Vec<AiotCommandDeliveryRecord>, AiotCommandDeliveryRepositoryError>;
    fn mark_delivered(
        &self,
        association: &AiotStorageAssociation,
        command_id: &str,
    ) -> Result<(), AiotCommandDeliveryRepositoryError>;
}

pub trait AiotDeviceSessionRepository: Send + Sync {
    fn list_sessions(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceSessionRecord>, AiotDeviceRepositoryError>;
    fn disconnect_session(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError>;
    fn is_session_disconnected(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceEventRecord {
    pub id: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub event_id: String,
    pub event_type: String,
    pub event_version: String,
    pub device_id: String,
    pub protocol_id: String,
    pub adapter_id: String,
    pub message_class: String,
    pub semantic_type: String,
    pub transport: String,
    pub direction: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub payload_hash: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_json: Option<String>,
    pub payload_json: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceEventCreateCommand {
    pub association: AiotStorageAssociation,
    pub event_id: Option<String>,
    pub event_type: String,
    pub event_version: String,
    pub device_id: String,
    pub protocol_id: String,
    pub adapter_id: String,
    pub message_class: String,
    pub semantic_type: String,
    pub transport: String,
    pub direction: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub payload_hash: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_json: Option<String>,
    pub payload_json: String,
    pub occurred_at: String,
}

impl AiotDeviceEventCreateCommand {
    pub fn new(
        association: AiotStorageAssociation,
        device_id: impl Into<String>,
        event_type: impl Into<String>,
    ) -> Self {
        Self {
            association,
            event_id: None,
            event_type: event_type.into(),
            event_version: "1".to_string(),
            device_id: device_id.into(),
            protocol_id: "xiaozhi.websocket".to_string(),
            adapter_id: "xiaozhi".to_string(),
            message_class: "mediaFrame".to_string(),
            semantic_type: "audio".to_string(),
            transport: "websocket".to_string(),
            direction: "device_to_cloud".to_string(),
            message_id: None,
            correlation_id: None,
            trace_id: None,
            payload_hash: None,
            media_resource_id: None,
            object_blob_id: None,
            media_json: None,
            payload_json: "{}".to_string(),
            occurred_at: default_timestamp().to_string(),
        }
    }

    pub fn with_event_id(mut self, event_id: impl Into<String>) -> Self {
        self.event_id = Some(event_id.into());
        self
    }

    pub fn with_event_version(mut self, event_version: impl Into<String>) -> Self {
        self.event_version = event_version.into();
        self
    }

    pub fn with_protocol(
        mut self,
        protocol_id: impl Into<String>,
        adapter_id: impl Into<String>,
    ) -> Self {
        self.protocol_id = protocol_id.into();
        self.adapter_id = adapter_id.into();
        self
    }

    pub fn with_message_routing(
        mut self,
        message_class: impl Into<String>,
        semantic_type: impl Into<String>,
        transport: impl Into<String>,
        direction: impl Into<String>,
    ) -> Self {
        self.message_class = message_class.into();
        self.semantic_type = semantic_type.into();
        self.transport = transport.into();
        self.direction = direction.into();
        self
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_payload_hash(mut self, payload_hash: impl Into<String>) -> Self {
        self.payload_hash = Some(payload_hash.into());
        self
    }

    pub fn with_media(
        mut self,
        media_resource_id: Option<String>,
        object_blob_id: Option<String>,
        media_json: Option<String>,
    ) -> Self {
        self.media_resource_id = media_resource_id;
        self.object_blob_id = object_blob_id;
        self.media_json = media_json;
        self
    }

    pub fn with_payload_json(mut self, payload_json: impl Into<String>) -> Self {
        self.payload_json = payload_json.into();
        self
    }

    pub fn with_occurred_at(mut self, occurred_at: impl Into<String>) -> Self {
        self.occurred_at = occurred_at.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotEventRepositoryError {
    PersistenceFailure,
}

pub trait AiotEventRepository: Send + Sync {
    fn record_event(
        &self,
        command: AiotDeviceEventCreateCommand,
    ) -> Result<AiotDeviceEventRecord, AiotEventRepositoryError>;
    fn list_events(
        &self,
        association: &AiotStorageAssociation,
        device_id: Option<&str>,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceEventRecord>, AiotEventRepositoryError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotTwinPropertyUpsertCommand {
    pub association: AiotStorageAssociation,
    pub device_id: String,
    pub property_name: String,
    pub desired_value_json: Option<String>,
    pub reported_value_json: Option<String>,
    pub desired_updated_at: Option<String>,
    pub reported_updated_at: Option<String>,
}

impl AiotTwinPropertyUpsertCommand {
    pub fn new(
        association: AiotStorageAssociation,
        device_id: impl Into<String>,
        property_name: impl Into<String>,
    ) -> Self {
        Self {
            association,
            device_id: device_id.into(),
            property_name: property_name.into(),
            desired_value_json: None,
            reported_value_json: None,
            desired_updated_at: None,
            reported_updated_at: None,
        }
    }

    pub fn with_desired_value_json(mut self, desired_value_json: impl Into<String>) -> Self {
        self.desired_value_json = Some(desired_value_json.into());
        self
    }

    pub fn with_reported_value_json(mut self, reported_value_json: impl Into<String>) -> Self {
        self.reported_value_json = Some(reported_value_json.into());
        self
    }

    pub fn with_desired_updated_at(mut self, desired_updated_at: impl Into<String>) -> Self {
        self.desired_updated_at = Some(desired_updated_at.into());
        self
    }

    pub fn with_reported_updated_at(mut self, reported_updated_at: impl Into<String>) -> Self {
        self.reported_updated_at = Some(reported_updated_at.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotDeviceTwinSnapshot {
    pub tenant_id: i64,
    pub organization_id: i64,
    pub device_id: String,
    pub desired: BTreeMap<String, String>,
    pub reported: BTreeMap<String, String>,
    pub desired_version: i64,
    pub reported_version: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotDeviceTwinRepositoryError {
    PersistenceFailure,
}

pub trait AiotDeviceTwinRepository: Send + Sync {
    fn upsert_twin_property(
        &self,
        command: AiotTwinPropertyUpsertCommand,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError>;
    fn get_twin_snapshot(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError>;
}

#[derive(Debug, Default)]
struct InMemoryAiotDeviceRepositoryState {
    next_device_pk: u64,
    devices: BTreeMap<String, AiotDeviceRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotDeviceRepository {
    state: Arc<Mutex<InMemoryAiotDeviceRepositoryState>>,
}

impl InMemoryAiotDeviceRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotDeviceRepository for InMemoryAiotDeviceRepository {
    fn create_device(
        &self,
        command: AiotDeviceCreateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        if !is_valid_int64_string(&command.product_id) {
            return Err(AiotDeviceRepositoryError::InvalidProductId);
        }

        let mut state = self.state.lock().expect("in-memory device repo poisoned");
        let key = scoped_device_key(&command.association, &command.device_id);
        if state.devices.contains_key(&key) {
            return Err(AiotDeviceRepositoryError::DuplicateDeviceId);
        }

        let id = (state.next_device_pk + 1).to_string();
        state.next_device_pk += 1;
        let record = AiotDeviceRecord {
            id,
            tenant_id: command.association.tenant_id,
            organization_id: command.association.organization_id,
            device_id: command.device_id,
            display_name: command.display_name,
            product_id: command.product_id,
            client_id: command.client_id,
            chip_family: command.chip_family,
            status: "active".to_string(),
            metadata_json: None,
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
        };
        state.devices.insert(key, record.clone());
        Ok(record)
    }

    fn get_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Option<AiotDeviceRecord> {
        self.state
            .lock()
            .expect("in-memory device repo poisoned")
            .devices
            .get(&scoped_device_key(association, device_id))
            .cloned()
    }

    fn list_devices(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceRecord>, AiotDeviceRepositoryError> {
        let items = self
            .state
            .lock()
            .expect("in-memory device repo poisoned")
            .devices
            .values()
            .filter(|device| {
                device.tenant_id == association.tenant_id
                    && device.organization_id == association.organization_id
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(paginate_vec(items, params))
    }

    fn list_device_ids_for_rollout(
        &self,
        association: &AiotStorageAssociation,
        product_id: Option<&str>,
        page_size: Option<i64>,
    ) -> Result<Vec<String>, AiotDeviceRepositoryError> {
        let mut ids = self
            .state
            .lock()
            .expect("in-memory device repo poisoned")
            .devices
            .values()
            .filter(|device| {
                device.tenant_id == association.tenant_id
                    && device.organization_id == association.organization_id
                    && product_id.is_none_or(|product| device.product_id == product)
            })
            .map(|device| device.device_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        if let Some(page_size) = page_size {
            let max = page_size.max(0) as usize;
            ids.truncate(max);
        }
        Ok(ids)
    }

    fn update_device(
        &self,
        command: AiotDeviceUpdateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        let mut state = self.state.lock().expect("in-memory device repo poisoned");
        let key = scoped_device_key(&command.association, &command.device_id);
        let Some(device) = state.devices.get_mut(&key) else {
            return Err(AiotDeviceRepositoryError::NotFound);
        };
        if let Some(display_name) = command.display_name {
            device.display_name = display_name;
        }
        if let Some(status) = command.status {
            device.status = status;
        }
        if command.metadata_json.is_some() {
            device.metadata_json = command.metadata_json;
        }
        Ok(device.clone())
    }

    fn delete_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<(), AiotDeviceRepositoryError> {
        let mut state = self.state.lock().expect("in-memory device repo poisoned");
        let key = scoped_device_key(association, device_id);
        if state.devices.remove(&key).is_some() {
            Ok(())
        } else {
            Err(AiotDeviceRepositoryError::NotFound)
        }
    }
}

#[derive(Debug, Default)]
struct InMemoryAiotCommandRepositoryState {
    next_command_pk: u64,
    commands: BTreeMap<String, AiotCommandRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotCommandRepository {
    state: Arc<Mutex<InMemoryAiotCommandRepositoryState>>,
}

impl InMemoryAiotCommandRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotCommandRepository for InMemoryAiotCommandRepository {
    fn create_command(
        &self,
        command: AiotCommandCreateCommand,
    ) -> Result<AiotCommandRecord, AiotCommandRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory aiot command repo poisoned");

        let command_id = command.command_id.unwrap_or_else(|| {
            format!(
                "cmd-{}-{:04}",
                command.device_id,
                state.next_command_pk.saturating_add(1)
            )
        });
        let scoped_key = scoped_command_key(&command.association, &command_id);
        if state.commands.contains_key(&scoped_key) {
            return Err(AiotCommandRepositoryError::DuplicateCommandId);
        }

        let id = state.next_command_pk.saturating_add(1).to_string();
        state.next_command_pk = state.next_command_pk.saturating_add(1);

        let record = AiotCommandRecord {
            id,
            tenant_id: command.association.tenant_id,
            organization_id: command.association.organization_id,
            command_id,
            device_id: command.device_id,
            session_id: command.session_id,
            capability_name: command.capability_name,
            command_name: command.command_name,
            request_payload_json: command.request_payload_json,
            request_media_resource_id: command.request_media_resource_id,
            request_object_blob_id: command.request_object_blob_id,
            request_media_json: command.request_media_json,
            status: command.status,
            trace_id: command.trace_id,
            timeout_at: command.timeout_at,
            ack_at: None,
            result_at: None,
            created_at: default_timestamp().to_string(),
            result: None,
        };
        state.commands.insert(scoped_key, record.clone());
        Ok(record)
    }

    fn get_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError> {
        let state = self
            .state
            .lock()
            .expect("in-memory aiot command repo poisoned");
        let key = scoped_command_key(association, command_id);
        let Some(record) = state.commands.get(&key) else {
            return Ok(None);
        };
        if record.device_id != device_id {
            return Ok(None);
        }
        Ok(Some(record.clone()))
    }

    fn list_commands(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCommandRecord>, AiotCommandRepositoryError> {
        let mut commands = self
            .state
            .lock()
            .expect("in-memory aiot command repo poisoned")
            .commands
            .values()
            .filter(|record| {
                record.tenant_id == association.tenant_id
                    && record.organization_id == association.organization_id
                    && record.device_id == device_id
            })
            .cloned()
            .collect::<Vec<_>>();
        commands.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(paginate_vec(commands, params))
    }

    fn cancel_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory aiot command repo poisoned");
        let key = scoped_command_key(association, command_id);
        let Some(record) = state.commands.get_mut(&key) else {
            return Ok(None);
        };
        if record.device_id != device_id {
            return Ok(None);
        }
        record.status = "cancelled".to_string();
        Ok(Some(record.clone()))
    }
}

#[derive(Debug, Default)]
struct InMemoryAiotDeviceSessionRepositoryState {
    disconnected_sessions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotDeviceSessionRepository {
    state: Arc<Mutex<InMemoryAiotDeviceSessionRepositoryState>>,
}

impl InMemoryAiotDeviceSessionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotDeviceSessionRepository for InMemoryAiotDeviceSessionRepository {
    fn list_sessions(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceSessionRecord>, AiotDeviceRepositoryError> {
        let state = self.state.lock().expect("in-memory session repo poisoned");
        let session_id = format!("session-{device_id}-primary");
        let key = scoped_device_session_key(association, device_id, &session_id);
        if state.disconnected_sessions.contains_key(&key) {
            return Ok(paginate_vec(Vec::new(), params));
        }
        let session = AiotDeviceSessionRecord {
            session_id,
            device_id: device_id.to_string(),
            status: "connected".to_string(),
            connected_at: Some(default_timestamp().to_string()),
            disconnected_at: None,
            transport: "websocket".to_string(),
            protocol_id: "xiaozhi.websocket".to_string(),
            adapter_id: "xiaozhi".to_string(),
        };
        Ok(paginate_vec(vec![session], params))
    }

    fn disconnect_session(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        let expected_session_id = format!("session-{device_id}-primary");
        if session_id != expected_session_id {
            return Ok(false);
        }
        let mut state = self.state.lock().expect("in-memory session repo poisoned");
        let key = scoped_device_session_key(association, device_id, session_id);
        if state.disconnected_sessions.contains_key(&key) {
            return Ok(false);
        }
        state
            .disconnected_sessions
            .insert(key, default_timestamp().to_string());
        Ok(true)
    }

    fn is_session_disconnected(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        Ok(self
            .state
            .lock()
            .expect("in-memory session repo poisoned")
            .disconnected_sessions
            .contains_key(&scoped_device_session_key(
                association,
                device_id,
                session_id,
            )))
    }
}

#[derive(Debug, Default)]
struct InMemoryAiotEventRepositoryState {
    next_event_pk: u64,
    events: Vec<AiotDeviceEventRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotEventRepository {
    state: Arc<Mutex<InMemoryAiotEventRepositoryState>>,
}

impl InMemoryAiotEventRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotEventRepository for InMemoryAiotEventRepository {
    fn record_event(
        &self,
        command: AiotDeviceEventCreateCommand,
    ) -> Result<AiotDeviceEventRecord, AiotEventRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory aiot event repo poisoned");
        let next_event_pk = state.next_event_pk.saturating_add(1);
        let event_id = command
            .event_id
            .unwrap_or_else(|| format!("evt-{}-{:04}", command.device_id, next_event_pk));

        let record = AiotDeviceEventRecord {
            id: next_event_pk.to_string(),
            tenant_id: command.association.tenant_id,
            organization_id: command.association.organization_id,
            event_id,
            event_type: command.event_type,
            event_version: command.event_version,
            device_id: command.device_id,
            protocol_id: command.protocol_id,
            adapter_id: command.adapter_id,
            message_class: command.message_class,
            semantic_type: command.semantic_type,
            transport: command.transport,
            direction: command.direction,
            message_id: command.message_id,
            correlation_id: command.correlation_id,
            trace_id: command.trace_id,
            payload_hash: command.payload_hash,
            media_resource_id: command.media_resource_id,
            object_blob_id: command.object_blob_id,
            media_json: command.media_json,
            payload_json: command.payload_json,
            occurred_at: command.occurred_at,
        };
        state.next_event_pk = next_event_pk;
        state.events.push(record.clone());
        Ok(record)
    }

    fn list_events(
        &self,
        association: &AiotStorageAssociation,
        device_id: Option<&str>,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceEventRecord>, AiotEventRepositoryError> {
        let mut events = self
            .state
            .lock()
            .expect("in-memory aiot event repo poisoned")
            .events
            .iter()
            .filter(|record| {
                record.tenant_id == association.tenant_id
                    && record.organization_id == association.organization_id
                    && device_id
                        .map(|scoped_device_id| scoped_device_id == record.device_id)
                        .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(paginate_vec(events, params))
    }
}

#[derive(Debug, Default)]
struct InMemoryAiotCommandDeliveryRepositoryState {
    next_pk: u64,
    deliveries: BTreeMap<String, AiotCommandDeliveryRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotCommandDeliveryRepository {
    state: Arc<Mutex<InMemoryAiotCommandDeliveryRepositoryState>>,
}

impl InMemoryAiotCommandDeliveryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotCommandDeliveryRepository for InMemoryAiotCommandDeliveryRepository {
    fn enqueue_delivery(
        &self,
        command: AiotCommandDeliveryEnqueueCommand,
    ) -> Result<AiotCommandDeliveryRecord, AiotCommandDeliveryRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory command delivery repo poisoned");
        state.next_pk = state.next_pk.saturating_add(1);
        let record = AiotCommandDeliveryRecord {
            id: state.next_pk.to_string(),
            tenant_id: command.association.tenant_id,
            organization_id: command.association.organization_id,
            command_id: command.command_id.clone(),
            session_id: command.session_id.clone(),
            delivery_state: "pending".to_string(),
            created_at: default_timestamp().to_string(),
        };
        state.deliveries.insert(command.command_id, record.clone());
        Ok(record)
    }

    fn list_pending_for_device(
        &self,
        association: &AiotStorageAssociation,
        _device_id: &str,
        limit: i64,
    ) -> Result<Vec<AiotCommandDeliveryRecord>, AiotCommandDeliveryRepositoryError> {
        let limit = limit.max(1) as usize;
        Ok(self
            .state
            .lock()
            .expect("in-memory command delivery repo poisoned")
            .deliveries
            .values()
            .filter(|record| {
                record.tenant_id == association.tenant_id
                    && record.organization_id == association.organization_id
                    && record.delivery_state == "pending"
            })
            .take(limit)
            .cloned()
            .collect())
    }

    fn mark_delivered(
        &self,
        association: &AiotStorageAssociation,
        command_id: &str,
    ) -> Result<(), AiotCommandDeliveryRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory command delivery repo poisoned");
        let Some(record) = state.deliveries.get_mut(command_id) else {
            return Err(AiotCommandDeliveryRepositoryError::CommandNotFound);
        };
        if record.tenant_id != association.tenant_id
            || record.organization_id != association.organization_id
        {
            return Err(AiotCommandDeliveryRepositoryError::CommandNotFound);
        }
        record.delivery_state = "delivered".to_string();
        Ok(())
    }
}

#[derive(Debug, Default)]
struct InMemoryAiotTwinRepositoryState {
    twins: BTreeMap<String, AiotDeviceTwinSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAiotDeviceTwinRepository {
    state: Arc<Mutex<InMemoryAiotTwinRepositoryState>>,
}

impl InMemoryAiotDeviceTwinRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AiotDeviceTwinRepository for InMemoryAiotDeviceTwinRepository {
    fn upsert_twin_property(
        &self,
        command: AiotTwinPropertyUpsertCommand,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let mut state = self
            .state
            .lock()
            .expect("in-memory aiot twin repo poisoned");
        let key = scoped_device_key(&command.association, &command.device_id);
        let snapshot = state
            .twins
            .entry(key)
            .or_insert_with(|| empty_twin_snapshot(&command.association, &command.device_id));

        if let Some(desired) = command.desired_value_json {
            snapshot
                .desired
                .insert(command.property_name.clone(), desired);
            snapshot.desired_version = snapshot.desired_version.saturating_add(1);
        }

        if let Some(reported) = command.reported_value_json {
            snapshot
                .reported
                .insert(command.property_name.clone(), reported);
            snapshot.reported_version = snapshot.reported_version.saturating_add(1);
        }

        let desired_updated_at = command
            .desired_updated_at
            .or(command.reported_updated_at)
            .unwrap_or_else(|| default_timestamp().to_string());
        snapshot.updated_at = desired_updated_at;

        Ok(snapshot.clone())
    }

    fn get_twin_snapshot(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let state = self
            .state
            .lock()
            .expect("in-memory aiot twin repo poisoned");
        let key = scoped_device_key(association, device_id);
        Ok(state
            .twins
            .get(&key)
            .cloned()
            .unwrap_or_else(|| empty_twin_snapshot(association, device_id)))
    }
}

fn scoped_device_key(association: &AiotStorageAssociation, device_id: &str) -> String {
    format!(
        "{}:{}:{}",
        association.tenant_id, association.organization_id, device_id
    )
}

fn scoped_command_key(association: &AiotStorageAssociation, command_id: &str) -> String {
    format!(
        "{}:{}:{}",
        association.tenant_id, association.organization_id, command_id
    )
}

fn scoped_device_session_key(
    association: &AiotStorageAssociation,
    device_id: &str,
    session_id: &str,
) -> String {
    format!(
        "{}:{}:{}:{}",
        association.tenant_id, association.organization_id, device_id, session_id
    )
}

fn empty_twin_snapshot(
    association: &AiotStorageAssociation,
    device_id: &str,
) -> AiotDeviceTwinSnapshot {
    AiotDeviceTwinSnapshot {
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        device_id: device_id.to_string(),
        desired: BTreeMap::new(),
        reported: BTreeMap::new(),
        desired_version: 0,
        reported_version: 0,
        updated_at: default_timestamp().to_string(),
    }
}

fn default_timestamp() -> &'static str {
    "2026-06-01T00:00:00Z"
}

fn is_valid_int64_string(value: &str) -> bool {
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return false;
    }

    value.parse::<i64>().is_ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiotStorageWriteKind {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiotStorageWriteRoute {
    pub repository_port: &'static str,
    pub primary_table: &'static str,
    pub transactional: bool,
}

impl AiotStorageWriteKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenSession => "open_session",
            Self::Authenticate => "authenticate",
            Self::KeepAlive => "keep_alive",
            Self::CloseSession => "close_session",
            Self::ProvisionDevice => "provision_device",
            Self::RecordTelemetry => "record_telemetry",
            Self::ApplyDesiredTwin => "apply_desired_twin",
            Self::DispatchCommand => "dispatch_command",
            Self::RecordCommandAck => "record_command_ack",
            Self::RecordCommandResult => "record_command_result",
            Self::ProcessMediaFrame => "process_media_frame",
            Self::EvaluateOta => "evaluate_ota",
            Self::DispatchOta => "dispatch_ota",
            Self::UpdateGatewayTopology => "update_gateway_topology",
            Self::RecordSecurityEvent => "record_security_event",
            Self::RecordDiagnostic => "record_diagnostic",
        }
    }

    pub fn storage_route(&self) -> AiotStorageWriteRoute {
        let (repository_port, primary_table) = match self {
            Self::OpenSession | Self::CloseSession => {
                ("DeviceSessionRepository", "iot_device_session")
            }
            Self::Authenticate => ("DeviceCredentialRepository", "iot_device_credential"),
            Self::KeepAlive => ("DeviceOnlineLeaseRepository", "iot_device_online_lease"),
            Self::ProvisionDevice => (
                "ProvisioningChallengeRepository",
                "iot_provisioning_challenge",
            ),
            Self::RecordTelemetry => ("TelemetryRepository", "iot_telemetry_event"),
            Self::ApplyDesiredTwin => ("DeviceTwinRepository", "iot_device_twin_property"),
            Self::DispatchCommand | Self::RecordCommandAck => {
                ("CommandDeliveryRepository", "iot_command_delivery")
            }
            Self::RecordCommandResult => ("CommandResultRepository", "iot_command_result"),
            Self::ProcessMediaFrame | Self::RecordDiagnostic => {
                ("DeviceEventRepository", "iot_device_event")
            }
            Self::EvaluateOta | Self::DispatchOta => {
                ("FirmwareDeploymentRepository", "iot_firmware_deployment")
            }
            Self::UpdateGatewayTopology => {
                ("GatewayTopologyRepository", "iot_gateway_child_device")
            }
            Self::RecordSecurityEvent => ("SecurityEventRepository", "iot_security_event"),
        };

        AiotStorageWriteRoute {
            repository_port,
            primary_table,
            transactional: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotOutboxWriteIntent {
    pub event_type: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub topic: String,
    pub event_version: String,
    pub payload_json: String,
    pub payload_hash: Option<String>,
    pub initial_status: &'static str,
}

impl AiotOutboxWriteIntent {
    pub fn new(
        event_type: impl Into<String>,
        aggregate_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        topic: impl Into<String>,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            aggregate_type: aggregate_type.into(),
            aggregate_id: aggregate_id.into(),
            topic: topic.into(),
            event_version: "1".to_string(),
            payload_json: "{}".to_string(),
            payload_hash: None,
            initial_status: "pending",
        }
    }

    pub fn with_event_version(mut self, event_version: impl Into<String>) -> Self {
        self.event_version = event_version.into();
        self
    }

    pub fn with_payload_json(mut self, payload_json: impl Into<String>) -> Self {
        self.payload_json = payload_json.into();
        self
    }

    pub fn with_payload_hash(mut self, payload_hash: impl Into<String>) -> Self {
        self.payload_hash = Some(payload_hash.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotStorageAssociation {
    pub tenant_id: i64,
    pub organization_id: i64,
    pub user_id: Option<i64>,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub data_scope: i32,
    pub created_by: Option<i64>,
    pub updated_by: Option<i64>,
    pub deleted_by: Option<i64>,
}

impl AiotStorageAssociation {
    pub fn tenant_org(tenant_id: i64, organization_id: i64) -> Self {
        Self {
            tenant_id,
            organization_id,
            user_id: None,
            owner_type: None,
            owner_id: None,
            data_scope: 0,
            created_by: None,
            updated_by: None,
            deleted_by: None,
        }
    }

    pub fn platform_shared() -> Self {
        Self::tenant_org(0, 0)
    }

    pub fn with_user_id(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_owner(
        mut self,
        owner_type: impl Into<String>,
        owner_id: impl Into<String>,
    ) -> Self {
        self.owner_type = Some(owner_type.into());
        self.owner_id = Some(owner_id.into());
        self
    }

    pub fn with_data_scope(mut self, data_scope: i32) -> Self {
        self.data_scope = data_scope;
        self
    }

    pub fn with_created_by(mut self, created_by: i64) -> Self {
        self.created_by = Some(created_by);
        self
    }

    pub fn with_updated_by(mut self, updated_by: i64) -> Self {
        self.updated_by = Some(updated_by);
        self
    }

    pub fn with_deleted_by(mut self, deleted_by: i64) -> Self {
        self.deleted_by = Some(deleted_by);
        self
    }
}

impl Default for AiotStorageAssociation {
    fn default() -> Self {
        Self::platform_shared()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotProtocolStorageCommand {
    pub association: AiotStorageAssociation,
    pub protocol_id: String,
    pub adapter_id: String,
    pub device_id: String,
    pub kind: AiotStorageWriteKind,
    pub primary_table: &'static str,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_resource_snapshot: Option<String>,
    pub idempotency_key: Option<String>,
    pub requires_transaction: bool,
    pub dead_letter_on_failure: bool,
    pub outbox: Option<AiotOutboxWriteIntent>,
}

impl AiotProtocolStorageCommand {
    pub fn new(
        protocol_id: impl Into<String>,
        adapter_id: impl Into<String>,
        device_id: impl Into<String>,
        kind: AiotStorageWriteKind,
        primary_table: &'static str,
    ) -> Self {
        let protocol_id = protocol_id.into();
        let adapter_id = adapter_id.into();
        let device_id = device_id.into();
        let idempotency_key = Some(format!(
            "{}:{}:{}:{}:{}",
            protocol_id,
            adapter_id,
            device_id,
            kind.as_str(),
            primary_table
        ));

        Self {
            association: AiotStorageAssociation::default(),
            protocol_id,
            adapter_id,
            device_id,
            kind,
            primary_table,
            message_id: None,
            correlation_id: None,
            session_id: None,
            trace_id: None,
            media_resource_id: None,
            object_blob_id: None,
            media_resource_snapshot: None,
            idempotency_key,
            requires_transaction: true,
            dead_letter_on_failure: true,
            outbox: None,
        }
    }

    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    pub fn with_association(mut self, association: AiotStorageAssociation) -> Self {
        self.association = association;
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
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

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_outbox(mut self, outbox: AiotOutboxWriteIntent) -> Self {
        self.outbox = Some(outbox);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotStorageWriteReceipt {
    pub accepted: bool,
    pub duplicate: bool,
    pub write_kind: Option<AiotStorageWriteKind>,
    pub primary_table: Option<String>,
    pub outbox_event_type: Option<String>,
    pub dead_letter_reason: Option<String>,
}

impl AiotStorageWriteReceipt {
    pub fn accepted(
        write_kind: AiotStorageWriteKind,
        primary_table: impl Into<String>,
        outbox_event_type: Option<String>,
    ) -> Self {
        Self {
            accepted: true,
            duplicate: false,
            write_kind: Some(write_kind),
            primary_table: Some(primary_table.into()),
            outbox_event_type,
            dead_letter_reason: None,
        }
    }

    pub fn dead_lettered(reason_code: impl Into<String>) -> Self {
        Self {
            accepted: false,
            duplicate: false,
            write_kind: None,
            primary_table: None,
            outbox_event_type: None,
            dead_letter_reason: Some(reason_code.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotPrimaryWriteRecord {
    pub association: AiotStorageAssociation,
    pub protocol_id: String,
    pub adapter_id: String,
    pub device_id: String,
    pub write_kind: AiotStorageWriteKind,
    pub primary_table: &'static str,
    pub idempotency_key: String,
    pub message_id: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub media_resource_id: Option<String>,
    pub object_blob_id: Option<String>,
    pub media_resource_snapshot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotOutboxEventRecord {
    pub association: AiotStorageAssociation,
    pub event_type: String,
    pub event_version: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub topic: String,
    pub payload_json: String,
    pub payload_hash: Option<String>,
    pub initial_status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InMemoryProtocolIngestSnapshot {
    pub primary_writes: Vec<AiotPrimaryWriteRecord>,
    pub outbox_events: Vec<AiotOutboxEventRecord>,
    pub dead_letters: Vec<AiotProtocolDeadLetterIntent>,
}

#[derive(Debug, Default)]
struct InMemoryProtocolIngestState {
    seen_idempotency_keys: BTreeSet<String>,
    snapshot: InMemoryProtocolIngestSnapshot,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryProtocolIngestUnitOfWork {
    state: Arc<Mutex<InMemoryProtocolIngestState>>,
}

impl InMemoryProtocolIngestUnitOfWork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> InMemoryProtocolIngestSnapshot {
        self.state
            .lock()
            .expect("in-memory uow poisoned")
            .snapshot
            .clone()
    }
}

impl AiotProtocolIngestUnitOfWork for InMemoryProtocolIngestUnitOfWork {
    fn execute_protocol_command(
        &self,
        command: &AiotProtocolStorageCommand,
    ) -> AiotStorageWriteReceipt {
        let mut state = self.state.lock().expect("in-memory uow poisoned");
        let idempotency_key = command.idempotency_key.clone().unwrap_or_else(|| {
            format!(
                "{}:{}:{}:{}:{}",
                command.protocol_id,
                command.adapter_id,
                command.device_id,
                command.kind.as_str(),
                command.primary_table
            )
        });

        if !state.seen_idempotency_keys.insert(idempotency_key.clone()) {
            let mut receipt = AiotStorageWriteReceipt::accepted(
                command.kind,
                command.primary_table,
                command
                    .outbox
                    .as_ref()
                    .map(|outbox| outbox.event_type.clone()),
            );
            receipt.duplicate = true;
            return receipt;
        }

        state.snapshot.primary_writes.push(AiotPrimaryWriteRecord {
            association: command.association.clone(),
            protocol_id: command.protocol_id.clone(),
            adapter_id: command.adapter_id.clone(),
            device_id: command.device_id.clone(),
            write_kind: command.kind,
            primary_table: command.primary_table,
            idempotency_key,
            message_id: command.message_id.clone(),
            correlation_id: command.correlation_id.clone(),
            trace_id: command.trace_id.clone(),
            media_resource_id: command.media_resource_id.clone(),
            object_blob_id: command.object_blob_id.clone(),
            media_resource_snapshot: command.media_resource_snapshot.clone(),
        });

        if let Some(outbox) = &command.outbox {
            state.snapshot.outbox_events.push(AiotOutboxEventRecord {
                association: command.association.clone(),
                event_type: outbox.event_type.clone(),
                event_version: outbox.event_version.clone(),
                aggregate_type: outbox.aggregate_type.clone(),
                aggregate_id: outbox.aggregate_id.clone(),
                topic: outbox.topic.clone(),
                payload_json: outbox.payload_json.clone(),
                payload_hash: outbox.payload_hash.clone(),
                initial_status: outbox.initial_status,
            });
        }

        AiotStorageWriteReceipt::accepted(
            command.kind,
            command.primary_table,
            command
                .outbox
                .as_ref()
                .map(|outbox| outbox.event_type.clone()),
        )
    }

    fn record_dead_letter(&self, intent: &AiotProtocolDeadLetterIntent) -> AiotStorageWriteReceipt {
        self.state
            .lock()
            .expect("in-memory uow poisoned")
            .snapshot
            .dead_letters
            .push(intent.clone());

        AiotStorageWriteReceipt::dead_lettered(intent.reason_code.clone())
    }
}

pub trait AiotProtocolIngestUnitOfWork: Send + Sync {
    fn execute_protocol_command(
        &self,
        command: &AiotProtocolStorageCommand,
    ) -> AiotStorageWriteReceipt;

    fn record_dead_letter(&self, intent: &AiotProtocolDeadLetterIntent) -> AiotStorageWriteReceipt;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotProtocolDeadLetterIntent {
    pub association: AiotStorageAssociation,
    pub protocol_id: String,
    pub adapter_id: String,
    pub device_id: Option<String>,
    pub reason_code: String,
    pub payload_ref: Option<String>,
    pub payload_hash: Option<String>,
    pub raw_payload: Option<String>,
    pub trace_id: Option<String>,
}

impl AiotProtocolDeadLetterIntent {
    pub fn new(
        protocol_id: impl Into<String>,
        adapter_id: impl Into<String>,
        reason_code: impl Into<String>,
        payload_ref: impl Into<String>,
    ) -> Self {
        Self {
            association: AiotStorageAssociation::default(),
            protocol_id: protocol_id.into(),
            adapter_id: adapter_id.into(),
            device_id: None,
            reason_code: reason_code.into(),
            payload_ref: Some(payload_ref.into()),
            payload_hash: None,
            raw_payload: None,
            trace_id: None,
        }
    }

    pub fn from_protocol_error(
        protocol_id: impl Into<String>,
        adapter_id: impl Into<String>,
        reason_code: impl Into<String>,
        payload_ref: impl Into<String>,
    ) -> Self {
        Self::new(protocol_id, adapter_id, reason_code, payload_ref)
    }

    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn with_association(mut self, association: AiotStorageAssociation) -> Self {
        self.association = association;
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_payload_hash(mut self, payload_hash: impl Into<String>) -> Self {
        self.payload_hash = Some(payload_hash.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiotRetryPolicy {
    pub max_attempts: u32,
    pub dead_letter_after_attempts: u32,
    pub initial_backoff_seconds: u64,
    pub max_backoff_seconds: u64,
}

impl AiotRetryPolicy {
    pub fn standard() -> Self {
        Self {
            max_attempts: 12,
            dead_letter_after_attempts: 12,
            initial_backoff_seconds: 1,
            max_backoff_seconds: 300,
        }
    }

    pub fn backoff_seconds(&self, attempt: u32) -> u64 {
        let multiplier = 1_u64.checked_shl(attempt.min(63)).unwrap_or(u64::MAX);
        self.initial_backoff_seconds
            .saturating_mul(multiplier)
            .min(self.max_backoff_seconds)
    }

    pub fn should_dead_letter(&self, attempt_count: u32) -> bool {
        attempt_count >= self.dead_letter_after_attempts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiotStorageFailure {
    pub reason_code: String,
    pub attempt_count: u32,
    pub retryable: bool,
}

impl AiotStorageFailure {
    pub fn retryable(reason_code: impl Into<String>, attempt_count: u32) -> Self {
        Self {
            reason_code: reason_code.into(),
            attempt_count,
            retryable: true,
        }
    }

    pub fn fatal(reason_code: impl Into<String>) -> Self {
        Self {
            reason_code: reason_code.into(),
            attempt_count: 0,
            retryable: false,
        }
    }

    pub fn disposition(&self, policy: &AiotRetryPolicy) -> AiotStorageFailureDisposition {
        if !self.retryable || policy.should_dead_letter(self.attempt_count) {
            return AiotStorageFailureDisposition::DeadLetter {
                reason_code: self.reason_code.clone(),
            };
        }

        AiotStorageFailureDisposition::Retry {
            next_attempt_in_seconds: policy.backoff_seconds(self.attempt_count),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiotStorageFailureDisposition {
    Retry { next_attempt_in_seconds: u64 },
    DeadLetter { reason_code: String },
}

pub fn standard_dead_letter_reason_catalog() -> Vec<&'static str> {
    vec![
        "decode.unsupported_message_type",
        "decode.invalid_frame",
        "auth.failed",
        "auth.replay_detected",
        "storage.write_failed",
        "command.route_unavailable",
        "ota.policy_violation",
        "backpressure.dead_letter_non_critical",
    ]
}

pub fn table_contract(name: &str) -> Option<AiotTableContract> {
    match name {
        "iot_product" => Some(
            AiotTableContract::new(
                "iot_product",
                "product_catalog",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id")
            .with_index("uk_iot_product_tenant_code"),
        ),
        "iot_hardware_profile" => Some(
            AiotTableContract::new(
                "iot_hardware_profile",
                "hardware_profile",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("chip_family")
            .with_column("runtime_profile")
            .with_index("idx_iot_hardware_profile_tenant_chip"),
        ),
        "iot_protocol_profile" => Some(
            AiotTableContract::new(
                "iot_protocol_profile",
                "protocol_profile",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("default_protocol_id")
            .with_index("idx_iot_protocol_profile_tenant_protocol"),
        ),
        "iot_capability_model" => Some(
            AiotTableContract::new(
                "iot_capability_model",
                "capability_model",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id"),
        ),
        "iot_capability_definition" => Some(
            AiotTableContract::new(
                "iot_capability_definition",
                "capability_model",
                TableProfile::TenantEntity,
            )
            .with_column("capability_model_id")
            .with_column("capability_name")
            .with_column("capability_kind")
            .with_column("schema_json")
            .with_index("uk_iot_capability_definition_tenant_model_name"),
        ),
        "iot_device" => Some(
            AiotTableContract::new(
                "iot_device",
                "device_registry",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id")
            .with_column("device_key")
            .with_column("product_id")
            .with_column("device_id")
            .with_column("client_id")
            .with_column("chip_family")
            .with_column("runtime_profile")
            .with_column("firmware_version")
            .with_column("created_by")
            .with_column("updated_by")
            .with_index("uk_iot_device_uuid")
            .with_index("uk_iot_device_tenant_device_key")
            .with_index("uk_iot_device_tenant_product_device_id")
            .with_index("uk_iot_device_tenant_client_id")
            .with_index("idx_iot_device_tenant_product_status")
            .with_index("idx_iot_device_tenant_last_seen"),
        ),
        "iot_device_credential" => Some(
            AiotTableContract::new(
                "iot_device_credential",
                "device_registry",
                TableProfile::TenantEntity,
            )
            .with_column("device_id")
            .with_column("credential_type")
            .with_column("credential_hash")
            .with_column("credential_ref")
            .with_column("expires_at")
            .with_index("idx_iot_device_credential_tenant_device_status"),
        ),
        "iot_device_binding" => Some(
            AiotTableContract::new(
                "iot_device_binding",
                "device_registry",
                TableProfile::RelationEntity,
            )
            .with_column("device_id")
            .with_column("target_type")
            .with_column("target_id")
            .with_index("idx_iot_device_binding_tenant_target"),
        ),
        "iot_gateway_child_device" => Some(
            AiotTableContract::new(
                "iot_gateway_child_device",
                "edge_gateway",
                TableProfile::RelationEntity,
            )
            .with_column("gateway_device_id")
            .with_column("child_device_id")
            .with_column("topology_role")
            .with_index("uk_iot_gateway_child_device_tenant_pair"),
        ),
        "iot_device_connection" => Some(
            AiotTableContract::new(
                "iot_device_connection",
                "session_runtime",
                TableProfile::RuntimeFact,
            )
            .with_column("connection_id")
            .with_column("device_id")
            .with_column("transport")
            .with_column("remote_addr")
            .with_index("idx_iot_device_connection_tenant_device_created"),
        ),
        "iot_device_session" => Some(
            AiotTableContract::new(
                "iot_device_session",
                "session_runtime",
                TableProfile::RuntimeFact,
            )
            .with_column("device_id")
            .with_column("session_id")
            .with_column("protocol_id")
            .with_index("idx_iot_device_session_tenant_device_status"),
        ),
        "iot_device_online_lease" => Some(
            AiotTableContract::new(
                "iot_device_online_lease",
                "session_runtime",
                TableProfile::Projection,
            )
            .read_model()
            .with_write_owner("sdkwork-aiot-service-host")
            .with_column("device_id")
            .with_column("session_id")
            .with_column("node_id")
            .with_column("lease_expires_at")
            .with_index("idx_iot_device_online_lease_tenant_expires"),
        ),
        "iot_command" => Some(
            AiotTableContract::new("iot_command", "command_control", TableProfile::RuntimeFact)
                .with_column("command_id")
                .with_column("device_id")
                .with_column("capability_name")
                .with_column("command_name")
                .with_column("request_media_resource_id")
                .with_column("request_object_blob_id")
                .with_column("request_media_resource_snapshot")
                .with_column("idempotency_key")
                .with_column("trace_id")
                .with_index("idx_iot_command_tenant_device_status_created")
                .with_index("idx_iot_command_tenant_status_timeout")
                .with_index("uk_iot_command_tenant_idempotency_key"),
        ),
        "iot_command_delivery" => Some(
            AiotTableContract::new(
                "iot_command_delivery",
                "command_control",
                TableProfile::RuntimeFact,
            )
            .with_column("command_id")
            .with_column("session_id")
            .with_column("delivery_state")
            .with_index("idx_iot_command_delivery_tenant_session_status"),
        ),
        "iot_command_result" => Some(
            AiotTableContract::new(
                "iot_command_result",
                "command_control",
                TableProfile::RuntimeFact,
            )
            .with_column("command_id")
            .with_column("result_payload")
            .with_column("result_media_resource_id")
            .with_column("result_object_blob_id")
            .with_column("result_media_resource_snapshot")
            .with_column("result_code")
            .with_index("idx_iot_command_result_tenant_command"),
        ),
        "iot_device_twin" => Some(
            AiotTableContract::new("iot_device_twin", "device_twin", TableProfile::Projection)
                .read_model()
                .with_column("device_id")
                .with_column("desired_version")
                .with_column("reported_version")
                .with_index("uk_iot_device_twin_tenant_device"),
        ),
        "iot_device_twin_property" => Some(
            AiotTableContract::new(
                "iot_device_twin_property",
                "device_twin",
                TableProfile::Projection,
            )
            .read_model()
            .with_column("device_id")
            .with_column("property_name")
            .with_column("desired_value")
            .with_column("reported_value")
            .with_index("idx_iot_twin_property_tenant_device_property"),
        ),
        "iot_telemetry_latest" => Some(
            AiotTableContract::new(
                "iot_telemetry_latest",
                "telemetry_event",
                TableProfile::Projection,
            )
            .read_model()
            .with_column("device_id")
            .with_column("metric_key")
            .with_column("metric_value")
            .with_index("idx_iot_telemetry_latest_tenant_device_key"),
        ),
        "iot_telemetry_event" => Some(
            AiotTableContract::new(
                "iot_telemetry_event",
                "telemetry_event",
                TableProfile::EventLog,
            )
            .with_column("device_id")
            .with_column("metric_key")
            .with_column("metric_value")
            .with_column("measured_at")
            .with_index("idx_iot_telemetry_event_tenant_device_time"),
        ),
        "iot_device_event" => Some(
            AiotTableContract::new(
                "iot_device_event",
                "telemetry_event",
                TableProfile::EventLog,
            )
            .with_column("device_id")
            .with_column("event_type")
            .with_column("event_payload")
            .with_column("media_resource_id")
            .with_column("media_resource_snapshot")
            .with_index("idx_iot_device_event_tenant_device_time"),
        ),
        "iot_security_event" => Some(
            AiotTableContract::new(
                "iot_security_event",
                "telemetry_event",
                TableProfile::EventLog,
            )
            .with_column("security_event_type")
            .with_column("severity")
            .with_column("actor_type")
            .with_column("actor_id")
            .with_index("idx_iot_security_event_tenant_time"),
        ),
        "iot_media_resource" => Some(
            AiotTableContract::new(
                "iot_media_resource",
                "media_resource",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id")
            .with_column("media_resource_id")
            .with_column("kind")
            .with_column("source")
            .with_column("object_blob_id")
            .with_column("resource_snapshot")
            .with_index("uk_iot_media_resource_tenant_resource_id")
            .with_index("idx_iot_media_resource_tenant_owner")
            .with_index("idx_iot_media_resource_tenant_object_blob"),
        ),
        "iot_device_media" => Some(
            AiotTableContract::new(
                "iot_device_media",
                "media_resource",
                TableProfile::RelationEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id")
            .with_column("media_role")
            .with_column("media_resource_id")
            .with_column("object_blob_id")
            .with_column("resource_snapshot")
            .with_column("sort_order")
            .with_index("idx_iot_device_media_tenant_owner_role")
            .with_index("idx_iot_device_media_tenant_media"),
        ),
        "iot_firmware_artifact" => Some(
            AiotTableContract::new(
                "iot_firmware_artifact",
                "ota_provisioning",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("media_resource_id")
            .with_column("object_blob_id")
            .with_column("media_resource_snapshot")
            .with_column("sha256")
            .with_column("signature")
            .with_index("uk_iot_firmware_artifact_tenant_media_resource"),
        ),
        "iot_firmware_rollout" => Some(
            AiotTableContract::new(
                "iot_firmware_rollout",
                "ota_provisioning",
                TableProfile::TenantOwnerEntity,
            )
            .with_column("owner_type")
            .with_column("owner_id")
            .with_column("artifact_id")
            .with_column("rollout_policy")
            .with_index("idx_iot_firmware_rollout_tenant_status"),
        ),
        "iot_firmware_rollout_target" => Some(
            AiotTableContract::new(
                "iot_firmware_rollout_target",
                "ota_provisioning",
                TableProfile::RelationEntity,
            )
            .with_column("rollout_id")
            .with_column("target_type")
            .with_column("target_id")
            .with_index("idx_iot_firmware_rollout_target_tenant_rollout"),
        ),
        "iot_firmware_deployment" => Some(
            AiotTableContract::new(
                "iot_firmware_deployment",
                "ota_provisioning",
                TableProfile::RuntimeFact,
            )
            .with_column("rollout_id")
            .with_column("device_id")
            .with_column("deployment_state")
            .with_index("idx_iot_firmware_deployment_tenant_device_status"),
        ),
        "iot_provisioning_challenge" => Some(
            AiotTableContract::new(
                "iot_provisioning_challenge",
                "ota_provisioning",
                TableProfile::RuntimeFact,
            )
            .with_column("challenge_id")
            .with_column("device_hint")
            .with_column("expires_at")
            .with_index("idx_iot_provisioning_challenge_tenant_expires"),
        ),
        "iot_activation_record" => Some(
            AiotTableContract::new(
                "iot_activation_record",
                "ota_provisioning",
                TableProfile::EventLog,
            )
            .with_column("activation_id")
            .with_column("device_id")
            .with_column("activation_profile")
            .with_index("idx_iot_activation_record_tenant_device"),
        ),
        "iot_outbox_event" => Some(
            AiotTableContract::new("iot_outbox_event", "eventing", TableProfile::OutboxEvent)
                .with_column("event_id")
                .with_column("event_type")
                .with_column("event_version")
                .with_column("aggregate_type")
                .with_column("aggregate_id")
                .with_column("payload")
                .with_column("payload_hash")
                .with_column("next_attempt_at")
                .with_column("attempt_count")
                .with_column("trace_id")
                .with_index("idx_iot_outbox_event_status_next_attempt"),
        ),
        "iot_inbox_event" => Some(
            AiotTableContract::new("iot_inbox_event", "eventing", TableProfile::InboxEvent)
                .with_column("source_system")
                .with_column("message_id")
                .with_column("consumer_name")
                .with_column("payload_hash")
                .with_column("error_message")
                .with_column("processed_at")
                .with_index("uk_iot_inbox_event_consumer_message"),
        ),
        "iot_audit_log" => Some(
            AiotTableContract::new("iot_audit_log", "eventing", TableProfile::AuditLog)
                .with_column("operator_id")
                .with_column("action")
                .with_column("target_type")
                .with_column("target_id")
                .with_index("idx_iot_audit_log_tenant_created"),
        ),
        "iot_protocol_ingest_record" => Some(
            AiotTableContract::new(
                "iot_protocol_ingest_record",
                "protocol_runtime",
                TableProfile::InboxEvent,
            )
            .with_write_owner("sdkwork-aiot-storage-sqlx")
            .with_column("protocol_id")
            .with_column("adapter_id")
            .with_column("device_id")
            .with_column("message_id")
            .with_column("correlation_id")
            .with_column("media_resource_id")
            .with_column("object_blob_id")
            .with_column("media_resource_snapshot")
            .with_column("idempotency_key")
            .with_column("trace_id")
            .with_index("uk_iot_protocol_ingest_tenant_idempotency")
            .with_index("idx_iot_protocol_ingest_tenant_message"),
        ),
        "iot_protocol_message_dead_letter" => Some(
            AiotTableContract::new(
                "iot_protocol_message_dead_letter",
                "protocol_runtime",
                TableProfile::EventLog,
            )
            .with_write_owner("sdkwork-aiot-service-host")
            .with_column("protocol_id")
            .with_column("adapter_id")
            .with_column("reason_code")
            .with_column("payload_ref")
            .with_column("payload_hash")
            .with_index("idx_iot_protocol_dead_letter_tenant_created"),
        ),
        _ => None,
    }
}
