use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

mod blocking_device_pool;
mod credential;
mod credential_hash;
mod database_bootstrap;
mod device_database;
mod device_repository_sqlite;
mod dialect_sql;
mod firmware_ota_catalog;
mod framework_bootstrap;
mod outbox;
mod outbox_worker;
mod persisted_entity;
mod postgres_sync;
mod runtime_bridge;
mod row_decode;
mod row_id_allocator;
mod schema;
mod sqlite_sync;

#[cfg(test)]
mod test_env;

pub use blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine, DeviceDbTransaction};
pub use credential::{
    SqliteCredentialCreateCommand, SqliteCredentialRepositoryError, SqliteDeviceCredentialRecord,
    SqliteSqlxCredentialRepository,
};
pub use database_bootstrap::{
    aiot_device_blocking_pool, aiot_device_blocking_pool_from_env, aiot_device_pool_from_env,
    aiot_device_sqlite_memory_config, aiot_device_sqlite_memory_pool,
    device_database_config_is_durable_from_env, resolve_device_database_config,
    resolve_device_database_config_from_env, AIOT_DEVICE_DATABASE_SERVICE_NAME,
};
pub use device_database::{
    open_aiot_device_database, open_aiot_device_database_from_env, AiotDeviceDatabase,
};
pub use device_repository_sqlite::SqliteSqlxDeviceRepository;
pub use firmware_ota_catalog::{
    resolve_firmware_download_url, FirmwareOtaCatalog, FirmwareOtaHint,
    DEFAULT_ROLLOUT_DEVICE_BATCH, ENTITY_FIRMWARE_ARTIFACT, ENTITY_FIRMWARE_DEPLOYMENT,
    MAX_OTA_DEPLOYMENT_SCAN,
};
pub use framework_bootstrap::{
    bootstrap_aiot_database, bootstrap_aiot_database_from_env, connect_aiot_database_pool_from_env,
    connect_and_bootstrap_aiot_database_from_env, AiotDatabaseHost, AiotDatabasePool,
};
pub use outbox::SqliteOutboxEventRepository;
pub use outbox_worker::{
    configured_device_db_path_from_env, device_storage_ready_from_env,
    open_outbox_repository_for_path, open_outbox_repository_for_pool,
    open_outbox_repository_from_env, outbox_dispatcher_enabled_from_env, outbox_lag_count_from_env,
    outbox_lag_ready_threshold_from_env, outbox_readiness_probe, outbox_ready_from_env,
    sqlite_path_ready, start_outbox_dispatcher_worker, DEFAULT_OUTBOX_DISPATCH_INTERVAL_MS,
    DEFAULT_OUTBOX_LAG_READY_THRESHOLD, ENV_DEVICE_DB_PATH, ENV_OUTBOX_DISPATCHER_ENABLED,
    ENV_OUTBOX_DISPATCH_INTERVAL_MS, ENV_OUTBOX_LAG_READY_THRESHOLD,
};
pub use persisted_entity::{
    SqlitePersistedEntityError, SqlitePersistedEntityRecord, SqlitePersistedEntityRepository,
};
pub use postgres_sync::{BlockingPostgresPool, StoragePostgresError};
use schema::ensure_device_schema;
use sdkwork_aiot_storage::{
    paginate_vec, table_contract, AiotCommandCreateCommand, AiotCommandDeliveryEnqueueCommand,
    AiotCommandDeliveryRecord, AiotCommandDeliveryRepository, AiotCommandDeliveryRepositoryError,
    AiotCommandRecord, AiotCommandRepository, AiotCommandRepositoryError, AiotDeviceCreateCommand,
    AiotDeviceEventCreateCommand, AiotDeviceEventRecord, AiotDeviceRecord, AiotDeviceRepository,
    AiotDeviceRepositoryError, AiotDeviceSessionRecord, AiotDeviceSessionRepository,
    AiotDeviceTwinRepository, AiotDeviceTwinRepositoryError, AiotDeviceTwinSnapshot,
    AiotDeviceUpdateCommand, AiotEventRepository, AiotEventRepositoryError, AiotOffsetListResult,
    AiotProtocolDeadLetterIntent, AiotProtocolIngestUnitOfWork, AiotProtocolStorageCommand,
    AiotStorageAssociation, AiotStorageWriteReceipt, AiotTwinPropertyUpsertCommand,
    OffsetListPageParams, OUTBOX_STATUS_PENDING,
};
use sdkwork_database_sqlx::PoolError;
use sdkwork_utils_rust::uuid;
use sqlite_sync::sqlite_connect_url;
pub use sqlite_sync::{BlockingSqlitePool, StorageSqliteError};

pub fn schema_version() -> &'static str {
    "0.2.0"
}

/// Shared in-process SQLite URI so device, credential, and protocol-ingest repositories
/// observe the same schema when no persistent `SDKWORK_AIOT_DEVICE_DB_PATH` is configured.
pub const DEFAULT_SHARED_SQLITE_MEMORY_URI: &str =
    "file:sdkwork-aiot-device-db?mode=memory&cache=shared";

/// Opens the canonical device repository using `sdkwork-database-config` resolution.
pub fn open_device_repository(
    device_db_path: Option<&str>,
) -> Result<SqliteSqlxDeviceRepository, PoolError> {
    open_aiot_device_database(device_db_path)?
        .device_repository()
        .map_err(PoolError::PoolCreation)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlMigration {
    pub version: &'static str,
    pub name: &'static str,
    pub schema_version: &'static str,
    pub sql: &'static str,
}

pub fn migration_catalog() -> Vec<SqlMigration> {
    vec![
        SqlMigration {
            version: "0001",
            name: "aiot_core_schema",
            schema_version: schema_version(),
            sql: initial_migration_sql(),
        },
        SqlMigration {
            version: "0002",
            name: "aiot_admin_entity_schema",
            schema_version: schema_version(),
            sql: admin_entity_migration_sql(),
        },
        SqlMigration {
            version: "0003",
            name: "aiot_row_id_allocator",
            schema_version: schema_version(),
            sql: row_id_allocator_migration_sql(),
        },
        SqlMigration {
            version: "0004",
            name: "aiot_device_credential_active_unique",
            schema_version: schema_version(),
            sql: device_credential_active_unique_migration_sql(),
        },
    ]
}

pub fn device_credential_active_unique_migration_sql() -> &'static str {
    r#"
CREATE UNIQUE INDEX IF NOT EXISTS uk_iot_device_credential_tenant_device_active
    ON iot_device_credential (tenant_id, device_id)
    WHERE status = 1;
"#
}

pub fn row_id_allocator_migration_sql() -> &'static str {
    r#"
CREATE TABLE IF NOT EXISTS iot_row_id_allocator (
    table_name VARCHAR(128) NOT NULL PRIMARY KEY,
    next_id BIGINT NOT NULL
);
"#
}

pub fn admin_entity_migration_sql() -> &'static str {
    r#"
CREATE TABLE IF NOT EXISTS iot_admin_entity (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    entity_kind VARCHAR(64) NOT NULL,
    entity_key VARCHAR(128) NOT NULL,
    payload_json TEXT NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_admin_entity_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_admin_entity_scope_key UNIQUE (tenant_id, organization_id, entity_kind, entity_key)
);

CREATE INDEX IF NOT EXISTS idx_iot_admin_entity_tenant_kind
    ON iot_admin_entity (tenant_id, entity_kind, status);
"#
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlDeviceWriteOperation {
    Create(AiotDeviceRecord),
    Update(AiotDeviceRecord),
    Delete {
        association: AiotStorageAssociation,
        device_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlDeviceRepositoryPlanner {
    dialect: SqlDialect,
}

impl SqlDeviceRepositoryPlanner {
    pub fn standard() -> Self {
        Self {
            dialect: SqlDialect::Postgres,
        }
    }

    pub fn with_dialect(dialect: SqlDialect) -> Self {
        Self { dialect }
    }

    pub fn plan_create_device(
        &self,
        device: &AiotDeviceRecord,
    ) -> Result<SqlStatementBatch, SqlPlanError> {
        let batch = SqlStatementBatch::single(
            "device_create",
            device_create_statement(self.dialect, device)?,
        );
        batch.validate()?;
        Ok(batch)
    }

    pub fn plan_update_device(
        &self,
        device: &AiotDeviceRecord,
    ) -> Result<SqlStatementBatch, SqlPlanError> {
        let batch = SqlStatementBatch::single(
            "device_update",
            device_update_statement(self.dialect, device)?,
        );
        batch.validate()?;
        Ok(batch)
    }

    pub fn plan_delete_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<SqlStatementBatch, SqlPlanError> {
        let batch = SqlStatementBatch::single(
            "device_delete",
            device_delete_statement(self.dialect, association, device_id),
        );
        batch.validate()?;
        Ok(batch)
    }
}

impl Default for SqlDeviceRepositoryPlanner {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Default)]
struct InMemorySqlxDeviceRepositoryState {
    next_device_pk: u64,
    devices: BTreeMap<String, AiotDeviceRecord>,
    next_command_pk: u64,
    commands: BTreeMap<String, AiotCommandRecord>,
    command_idempotency_index: BTreeMap<String, String>,
    next_event_pk: u64,
    events: Vec<AiotDeviceEventRecord>,
    twins: BTreeMap<String, AiotDeviceTwinSnapshot>,
    disconnected_sessions: BTreeSet<String>,
    sessions: Vec<AiotDeviceSessionRecord>,
    next_delivery_pk: u64,
    deliveries: BTreeMap<String, AiotCommandDeliveryRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySqlxDeviceRepository {
    executor: InMemorySqlStatementExecutor,
    planner: SqlDeviceRepositoryPlanner,
    state: Arc<Mutex<InMemorySqlxDeviceRepositoryState>>,
    writes: Arc<Mutex<Vec<SqlDeviceWriteOperation>>>,
}

impl InMemorySqlxDeviceRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> Vec<SqlDeviceWriteOperation> {
        self.writes
            .lock()
            .expect("sqlx device repo writes poisoned")
            .clone()
    }

    pub fn executed_statements(&self) -> Vec<SqlStatementPlan> {
        self.executor.executed_statements()
    }
}

impl AiotDeviceRepository for InMemorySqlxDeviceRepository {
    fn create_device(
        &self,
        command: AiotDeviceCreateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        if !is_valid_int64_string(&command.product_id) {
            return Err(AiotDeviceRepositoryError::InvalidProductId);
        }

        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let key = scoped_device_key(&command.association, &command.device_id);
        if state.devices.contains_key(&key) {
            return Err(AiotDeviceRepositoryError::DuplicateDeviceId);
        }

        let record = AiotDeviceRecord {
            id: (state.next_device_pk + 1).to_string(),
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
        let batch = self
            .planner
            .plan_create_device(&record)
            .map_err(|_| AiotDeviceRepositoryError::PersistenceFailure)?;
        self.executor.execute_batch(batch);
        state.next_device_pk += 1;
        state.devices.insert(key, record.clone());
        self.writes
            .lock()
            .expect("sqlx device repo writes poisoned")
            .push(SqlDeviceWriteOperation::Create(record.clone()));
        Ok(record)
    }

    fn get_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Option<AiotDeviceRecord> {
        self.state
            .lock()
            .expect("sqlx device repo state poisoned")
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
            .expect("sqlx device repo state poisoned")
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
        limit: Option<i64>,
    ) -> Result<Vec<String>, AiotDeviceRepositoryError> {
        let mut ids = self
            .state
            .lock()
            .expect("sqlx device repo state poisoned")
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
        if let Some(limit) = limit {
            ids.truncate(limit.max(0) as usize);
        }
        Ok(ids)
    }

    fn update_device(
        &self,
        command: AiotDeviceUpdateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
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
        let record = device.clone();
        let batch = self
            .planner
            .plan_update_device(&record)
            .map_err(|_| AiotDeviceRepositoryError::PersistenceFailure)?;
        self.executor.execute_batch(batch);
        self.writes
            .lock()
            .expect("sqlx device repo writes poisoned")
            .push(SqlDeviceWriteOperation::Update(record.clone()));
        Ok(record)
    }

    fn delete_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<(), AiotDeviceRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let key = scoped_device_key(association, device_id);
        if state.devices.remove(&key).is_none() {
            return Err(AiotDeviceRepositoryError::NotFound);
        }
        let batch = self
            .planner
            .plan_delete_device(association, device_id)
            .map_err(|_| AiotDeviceRepositoryError::PersistenceFailure)?;
        self.executor.execute_batch(batch);
        self.writes
            .lock()
            .expect("sqlx device repo writes poisoned")
            .push(SqlDeviceWriteOperation::Delete {
                association: association.clone(),
                device_id: device_id.to_string(),
            });
        Ok(())
    }
}

impl AiotCommandRepository for InMemorySqlxDeviceRepository {
    fn create_command(
        &self,
        command: AiotCommandCreateCommand,
    ) -> Result<AiotCommandRecord, AiotCommandRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        if let Some(idempotency_key) = command.idempotency_key.as_deref() {
            let idempotency_scope_key = format!(
                "{}:{}:{idempotency_key}",
                command.association.tenant_id, command.association.organization_id
            );
            if let Some(existing_command_key) =
                state.command_idempotency_index.get(&idempotency_scope_key)
            {
                if let Some(existing) = state.commands.get(existing_command_key) {
                    return Ok(existing.clone());
                }
            }
        }

        let command_id = command.command_id.unwrap_or_else(|| {
            format!(
                "cmd-{}-{:04}",
                command.device_id,
                state.next_command_pk.saturating_add(1)
            )
        });
        let command_key = scoped_command_key(&command.association, &command_id);
        if state.commands.contains_key(&command_key) {
            return Err(AiotCommandRepositoryError::DuplicateCommandId);
        }
        let idempotency_key = command.idempotency_key.clone();

        let record = AiotCommandRecord {
            id: state.next_command_pk.saturating_add(1).to_string(),
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
        state.next_command_pk = state.next_command_pk.saturating_add(1);
        state.commands.insert(command_key, record.clone());
        if let Some(idempotency_key) = idempotency_key {
            let idempotency_scope_key = format!(
                "{}:{}:{idempotency_key}",
                command.association.tenant_id, command.association.organization_id
            );
            let command_key = scoped_command_key(&command.association, &record.command_id);
            state
                .command_idempotency_index
                .insert(idempotency_scope_key, command_key);
        }
        Ok(record)
    }

    fn get_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError> {
        let state = self.state.lock().expect("sqlx device repo state poisoned");
        let key = scoped_command_key(association, command_id);
        let Some(command) = state.commands.get(&key) else {
            return Ok(None);
        };
        if command.device_id != device_id {
            return Ok(None);
        }
        Ok(Some(command.clone()))
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
            .expect("sqlx device repo state poisoned")
            .commands
            .values()
            .filter(|command| {
                command.tenant_id == association.tenant_id
                    && command.organization_id == association.organization_id
                    && command.device_id == device_id
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
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let key = scoped_command_key(association, command_id);
        let Some(command) = state.commands.get_mut(&key) else {
            return Ok(None);
        };
        if command.device_id != device_id {
            return Ok(None);
        }
        command.status = "cancelled".to_string();
        Ok(Some(command.clone()))
    }
}

impl AiotDeviceSessionRepository for InMemorySqlxDeviceRepository {
    fn list_sessions(
        &self,
        _association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceSessionRecord>, AiotDeviceRepositoryError> {
        let sessions = self
            .state
            .lock()
            .expect("sqlx device repo state poisoned")
            .sessions
            .iter()
            .filter(|session| session.device_id == device_id)
            .cloned()
            .collect::<Vec<_>>();
        Ok(paginate_vec(sessions, params))
    }

    fn disconnect_session(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let key = scoped_device_session_key(association, device_id, session_id);
        Ok(state.disconnected_sessions.insert(key))
    }

    fn is_session_disconnected(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        let state = self.state.lock().expect("sqlx device repo state poisoned");
        Ok(state
            .disconnected_sessions
            .contains(&scoped_device_session_key(
                association,
                device_id,
                session_id,
            )))
    }
}

impl AiotEventRepository for InMemorySqlxDeviceRepository {
    fn record_event(
        &self,
        command: AiotDeviceEventCreateCommand,
    ) -> Result<AiotDeviceEventRecord, AiotEventRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let next_event_pk = state.next_event_pk.saturating_add(1);
        let event_id = command
            .event_id
            .unwrap_or_else(|| format!("evt-{}-{:04}", command.device_id, next_event_pk));
        let event = AiotDeviceEventRecord {
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
        state.events.push(event.clone());
        Ok(event)
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
            .expect("sqlx device repo state poisoned")
            .events
            .iter()
            .filter(|event| {
                event.tenant_id == association.tenant_id
                    && event.organization_id == association.organization_id
                    && device_id
                        .map(|scoped_device_id| scoped_device_id == event.device_id)
                        .unwrap_or(true)
            })
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(paginate_vec(events, params))
    }
}

impl AiotCommandDeliveryRepository for InMemorySqlxDeviceRepository {
    fn enqueue_delivery(
        &self,
        command: AiotCommandDeliveryEnqueueCommand,
    ) -> Result<AiotCommandDeliveryRecord, AiotCommandDeliveryRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let command_key = scoped_command_key(&command.association, &command.command_id);
        if !state.commands.contains_key(&command_key) {
            return Err(AiotCommandDeliveryRepositoryError::CommandNotFound);
        }
        state.next_delivery_pk = state.next_delivery_pk.saturating_add(1);
        let record = AiotCommandDeliveryRecord {
            id: state.next_delivery_pk.to_string(),
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
        device_id: &str,
        limit: i64,
    ) -> Result<Vec<AiotCommandDeliveryRecord>, AiotCommandDeliveryRepositoryError> {
        let limit = limit.max(1) as usize;
        let state = self.state.lock().expect("sqlx device repo state poisoned");
        Ok(state
            .deliveries
            .values()
            .filter(|record| {
                record.tenant_id == association.tenant_id
                    && record.organization_id == association.organization_id
                    && record.delivery_state == "pending"
                    && state
                        .commands
                        .get(&scoped_command_key(association, &record.command_id))
                        .is_some_and(|command| command.device_id == device_id)
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
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
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

impl AiotDeviceTwinRepository for InMemorySqlxDeviceRepository {
    fn upsert_twin_property(
        &self,
        command: AiotTwinPropertyUpsertCommand,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let mut state = self.state.lock().expect("sqlx device repo state poisoned");
        let twin_key = scoped_device_key(&command.association, &command.device_id);
        let snapshot = state
            .twins
            .entry(twin_key)
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
        snapshot.updated_at = command
            .desired_updated_at
            .or(command.reported_updated_at)
            .unwrap_or_else(|| default_timestamp().to_string());
        Ok(snapshot.clone())
    }

    fn get_twin_snapshot(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let state = self.state.lock().expect("sqlx device repo state poisoned");
        let twin_key = scoped_device_key(association, device_id);
        Ok(state
            .twins
            .get(&twin_key)
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

pub(crate) fn is_valid_int64_string(value: &str) -> bool {
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return false;
    }

    value.parse::<i64>().is_ok()
}

pub(crate) fn device_status_text(status: i64) -> String {
    match status {
        0 => "inactive".to_string(),
        1 => "active".to_string(),
        2 => "disabled".to_string(),
        3 => "deleted".to_string(),
        _ => "active".to_string(),
    }
}

pub(crate) fn command_status_code(status: &str) -> i64 {
    match status {
        "accepted" => 1,
        "dispatched" => 2,
        "acknowledged" => 3,
        "succeeded" => 4,
        "failed" => 5,
        "cancelled" => 6,
        "timeout" => 7,
        _ => 0,
    }
}

pub(crate) fn command_status_text(status: i64) -> String {
    match status {
        1 => "accepted".to_string(),
        2 => "dispatched".to_string(),
        3 => "acknowledged".to_string(),
        4 => "succeeded".to_string(),
        5 => "failed".to_string(),
        6 => "cancelled".to_string(),
        7 => "timeout".to_string(),
        _ => "pending".to_string(),
    }
}

pub(crate) fn default_timestamp() -> &'static str {
    "2026-06-01T00:00:00Z"
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlBindValue {
    Text(String),
    Int64(i64),
    Null,
}

impl SqlBindValue {
    fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    fn optional_text(value: Option<&str>) -> Self {
        value
            .map(|value| Self::Text(value.to_string()))
            .unwrap_or(Self::Null)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    Postgres,
    Sqlite,
}

impl SqlDialect {
    fn placeholder(&self, index: usize) -> String {
        match self {
            Self::Postgres => format!("${index}"),
            Self::Sqlite => "?".to_string(),
        }
    }

    fn placeholders(&self, count: usize) -> String {
        match self {
            Self::Postgres => (1..=count)
                .map(|index| self.placeholder(index))
                .collect::<Vec<_>>()
                .join(", "),
            Self::Sqlite => vec!["?"; count].join(", "),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlPlanError {
    pub code: String,
    pub table: Option<String>,
    pub column: Option<String>,
    pub statement_kind: Option<&'static str>,
}

impl SqlPlanError {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            table: None,
            column: None,
            statement_kind: None,
        }
    }

    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    pub fn with_statement_kind(mut self, statement_kind: &'static str) -> Self {
        self.statement_kind = Some(statement_kind);
        self
    }

    pub fn with_column(mut self, column: impl Into<String>) -> Self {
        self.column = Some(column.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlStatementPlan {
    pub statement_kind: &'static str,
    pub table: &'static str,
    pub dialect: SqlDialect,
    pub sql: String,
    pub binds: Vec<SqlBindValue>,
}

impl SqlStatementPlan {
    pub fn new(statement_kind: &'static str, table: &'static str, sql: impl Into<String>) -> Self {
        Self {
            statement_kind,
            table,
            dialect: SqlDialect::Postgres,
            sql: sql.into(),
            binds: Vec::new(),
        }
    }

    pub fn with_dialect(mut self, dialect: SqlDialect) -> Self {
        self.dialect = dialect;
        self
    }

    pub fn with_binds(mut self, binds: Vec<SqlBindValue>) -> Self {
        self.binds = binds;
        self
    }

    pub fn placeholder_count(&self) -> usize {
        match self.dialect {
            SqlDialect::Postgres => postgres_placeholder_count(&self.sql),
            SqlDialect::Sqlite => self
                .sql
                .chars()
                .filter(|candidate| *candidate == '?')
                .count(),
        }
    }

    pub fn runtime_prepended_bind_count(&self) -> usize {
        if crate::row_id_allocator::row_id_allocator_table_for_statement(self.statement_kind)
            .is_some()
        {
            1
        } else {
            0
        }
    }

    pub fn validate(&self) -> Result<(), SqlPlanError> {
        let placeholder_count = self.placeholder_count();
        let expected_binds = placeholder_count.saturating_sub(self.runtime_prepended_bind_count());
        if expected_binds != self.binds.len() {
            return Err(SqlPlanError::new("storage.sql.bind_count_mismatch")
                .with_table(self.table)
                .with_statement_kind(self.statement_kind));
        }

        if table_contract(self.table).is_none() {
            return Err(SqlPlanError::new("storage.sql.table.unsupported")
                .with_table(self.table)
                .with_statement_kind(self.statement_kind));
        }

        for column in sql_write_columns(&self.sql) {
            if !initial_migration_declares_column(self.table, &column) {
                return Err(SqlPlanError::new("storage.sql.column.unsupported")
                    .with_table(self.table)
                    .with_column(column)
                    .with_statement_kind(self.statement_kind));
            }
        }

        Ok(())
    }
}

impl SqlStatementPlan {
    fn bound(
        statement_kind: &'static str,
        table: &'static str,
        dialect: SqlDialect,
        sql: impl Into<String>,
        binds: Vec<SqlBindValue>,
    ) -> Self {
        Self::new(statement_kind, table, sql)
            .with_dialect(dialect)
            .with_binds(binds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlStatementBatch {
    pub batch_kind: &'static str,
    pub statements: Vec<SqlStatementPlan>,
}

impl SqlStatementBatch {
    pub fn new(batch_kind: &'static str, statements: Vec<SqlStatementPlan>) -> Self {
        Self {
            batch_kind,
            statements,
        }
    }

    pub fn single(batch_kind: &'static str, statement: SqlStatementPlan) -> Self {
        Self::new(batch_kind, vec![statement])
    }

    pub fn validate(&self) -> Result<(), SqlPlanError> {
        for statement in &self.statements {
            statement.validate()?;
        }

        Ok(())
    }
}

pub trait SqlStatementExecutor: Clone + Send + Sync {
    fn execute_idempotency_guard(&self, key: &str, statement: SqlStatementPlan) -> bool;

    fn execute_batch(&self, batch: SqlStatementBatch);

    fn execute_transaction(&self, transaction: SqlTransactionPlan) -> SqlTransactionOutcome {
        let SqlTransactionPlan {
            idempotency_key,
            guard,
            write_batch,
            ..
        } = transaction;

        if !self.execute_idempotency_guard(&idempotency_key, guard) {
            return SqlTransactionOutcome::Duplicate;
        }

        self.execute_batch(write_batch);
        SqlTransactionOutcome::Committed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlProtocolCommandPlan {
    pub idempotency_key: String,
    pub guard: SqlStatementPlan,
    pub write_batch: SqlStatementBatch,
}

impl SqlProtocolCommandPlan {
    pub fn validate(&self) -> Result<(), SqlPlanError> {
        self.guard.validate()?;
        self.write_batch.validate()?;

        Ok(())
    }

    pub fn into_transaction_plan(self) -> SqlTransactionPlan {
        SqlTransactionPlan::new(
            "protocol_ingest",
            self.idempotency_key,
            self.guard,
            self.write_batch,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlTransactionFailurePolicy {
    RollbackAll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlTransactionOutcome {
    Committed,
    Duplicate,
    RolledBack { reason_code: String },
}

impl SqlTransactionOutcome {
    pub fn rolled_back(reason_code: impl Into<String>) -> Self {
        Self::RolledBack {
            reason_code: reason_code.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlTransactionPlan {
    pub transaction_kind: &'static str,
    pub failure_policy: SqlTransactionFailurePolicy,
    pub idempotency_key: String,
    pub guard: SqlStatementPlan,
    pub write_batch: SqlStatementBatch,
}

impl SqlTransactionPlan {
    pub fn new(
        transaction_kind: &'static str,
        idempotency_key: impl Into<String>,
        guard: SqlStatementPlan,
        write_batch: SqlStatementBatch,
    ) -> Self {
        Self {
            transaction_kind,
            failure_policy: SqlTransactionFailurePolicy::RollbackAll,
            idempotency_key: idempotency_key.into(),
            guard,
            write_batch,
        }
    }

    pub fn ordered_statements(&self) -> Vec<SqlStatementPlan> {
        let mut statements = Vec::with_capacity(1 + self.write_batch.statements.len());
        statements.push(self.guard.clone());
        statements.extend(self.write_batch.statements.iter().cloned());
        statements
    }

    pub fn validate(&self) -> Result<(), SqlPlanError> {
        self.guard.validate()?;
        self.write_batch.validate()?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SqlProtocolIngestPlanner {
    dialect: SqlDialect,
}

impl SqlProtocolIngestPlanner {
    pub fn standard() -> Self {
        Self::for_dialect(SqlDialect::Postgres)
    }

    pub fn for_dialect(dialect: SqlDialect) -> Self {
        Self { dialect }
    }

    pub fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub fn plan_protocol_command(
        &self,
        command: &AiotProtocolStorageCommand,
    ) -> SqlProtocolCommandPlan {
        self.try_plan_protocol_command(command)
            .expect("standard protocol command plan must be valid")
    }

    pub fn try_plan_protocol_command(
        &self,
        command: &AiotProtocolStorageCommand,
    ) -> Result<SqlProtocolCommandPlan, SqlPlanError> {
        if table_contract(command.primary_table).is_none() {
            return Err(SqlPlanError::new("storage.sql.primary_table.unsupported")
                .with_table(command.primary_table));
        }

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
        let guard = idempotency_guard_statement(self.dialect, command, &idempotency_key);
        let mut statements = vec![primary_write_statement(
            self.dialect,
            command,
            &idempotency_key,
        )];
        if command.outbox.is_some() {
            statements.push(outbox_write_statement(self.dialect, command));
        }

        let plan = SqlProtocolCommandPlan {
            idempotency_key,
            guard,
            write_batch: SqlStatementBatch::new("protocol_ingest_write", statements),
        };
        plan.validate()?;

        Ok(plan)
    }

    pub fn plan_dead_letter(&self, intent: &AiotProtocolDeadLetterIntent) -> SqlStatementBatch {
        self.try_plan_dead_letter(intent)
            .expect("standard dead-letter plan must be valid")
    }

    pub fn try_plan_dead_letter(
        &self,
        intent: &AiotProtocolDeadLetterIntent,
    ) -> Result<SqlStatementBatch, SqlPlanError> {
        let batch = SqlStatementBatch::single(
            "dead_letter_write",
            dead_letter_write_statement(self.dialect, intent),
        );
        batch.validate()?;

        Ok(batch)
    }
}

impl Default for SqlProtocolIngestPlanner {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySqlStatementExecutor {
    state: Arc<Mutex<InMemorySqlStatementExecutorState>>,
}

#[derive(Debug, Default)]
struct InMemorySqlStatementExecutorState {
    idempotency_keys: BTreeSet<String>,
    executed_statements: Vec<SqlStatementPlan>,
    executed_batches: Vec<SqlStatementBatch>,
}

impl InMemorySqlStatementExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn claim_idempotency_key(&self, key: &str) -> bool {
        self.state
            .lock()
            .expect("sql statement executor poisoned")
            .idempotency_keys
            .insert(key.to_string())
    }

    pub fn execute(&self, statement: SqlStatementPlan) {
        self.state
            .lock()
            .expect("sql statement executor poisoned")
            .executed_statements
            .push(statement);
    }

    pub fn execute_batch(&self, batch: SqlStatementBatch) {
        let mut state = self.state.lock().expect("sql statement executor poisoned");
        state
            .executed_statements
            .extend(batch.statements.iter().cloned());
        state.executed_batches.push(batch);
    }

    pub fn executed_statements(&self) -> Vec<SqlStatementPlan> {
        self.state
            .lock()
            .expect("sql statement executor poisoned")
            .executed_statements
            .clone()
    }

    pub fn executed_batches(&self) -> Vec<SqlStatementBatch> {
        self.state
            .lock()
            .expect("sql statement executor poisoned")
            .executed_batches
            .clone()
    }
}

impl SqlStatementExecutor for InMemorySqlStatementExecutor {
    fn execute_idempotency_guard(&self, key: &str, statement: SqlStatementPlan) -> bool {
        let mut state = self.state.lock().expect("sql statement executor poisoned");
        state.executed_statements.push(statement.clone());
        state
            .executed_batches
            .push(SqlStatementBatch::single("idempotency_guard", statement));
        state.idempotency_keys.insert(key.to_string())
    }

    fn execute_batch(&self, batch: SqlStatementBatch) {
        InMemorySqlStatementExecutor::execute_batch(self, batch);
    }
}

#[derive(Debug, Clone)]
pub struct SqlxPoolSqlStatementExecutor {
    db: BlockingDevicePool,
}

impl SqlxPoolSqlStatementExecutor {
    pub fn new(db: BlockingDevicePool) -> Self {
        Self { db }
    }

    pub fn new_in_memory() -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url("file:sdkwork-aiot-sql-executor?mode=memory&cache=shared");
        let db = BlockingSqlitePool::connect(&url)?;
        ensure_device_schema(&BlockingDevicePool::Sqlite(db.clone()))?;
        Ok(Self::new(BlockingDevicePool::Sqlite(db)))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url(path.as_ref().to_string_lossy().as_ref());
        let db = BlockingSqlitePool::connect(&url)?;
        ensure_device_schema(&BlockingDevicePool::Sqlite(db.clone()))?;
        Ok(Self::new(BlockingDevicePool::Sqlite(db)))
    }

    pub fn protocol_ingest_unit_of_work(&self) -> SqlxProtocolIngestUnitOfWork<Self> {
        SqlxProtocolIngestUnitOfWork::with_planner(
            self.clone(),
            SqlProtocolIngestPlanner::for_dialect(self.db.engine().dialect()),
        )
    }
}

enum SqlExecutorTransactionError {
    Duplicate,
    Sql,
}

impl SqlExecutorTransactionError {
    fn into_outcome(self) -> SqlTransactionOutcome {
        match self {
            Self::Duplicate => SqlTransactionOutcome::Duplicate,
            Self::Sql => SqlTransactionOutcome::rolled_back("storage.sql.write_batch_failed"),
        }
    }
}

impl From<StorageSqliteError> for SqlExecutorTransactionError {
    fn from(_error: StorageSqliteError) -> Self {
        Self::Sql
    }
}

impl SqlStatementExecutor for SqlxPoolSqlStatementExecutor {
    fn execute_idempotency_guard(&self, _key: &str, statement: SqlStatementPlan) -> bool {
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let statement = crate::row_id_allocator::prepend_allocated_row_id_bind(
                        &mut tx, dialect, statement,
                    )
                    .await
                    .map_err(|_| SqlExecutorTransactionError::Sql)?;
                    let changed = tx
                        .execute_plan(&statement)
                        .await
                        .map_err(|_| SqlExecutorTransactionError::Sql)?;
                    if changed == 0 {
                        return Err(SqlExecutorTransactionError::Duplicate);
                    }
                    Ok(())
                })
            })
            .is_ok()
    }

    fn execute_batch(&self, batch: SqlStatementBatch) {
        if let Err(error) = self.db.execute_statement_batch(batch) {
            eprintln!("protocol ingest execute_batch failed: {error}");
        }
    }

    fn execute_transaction(&self, transaction: SqlTransactionPlan) -> SqlTransactionOutcome {
        match self.db.with_device_transaction(|mut tx, dialect| {
            let guard = transaction.guard.clone();
            let write_batch = transaction.write_batch.clone();
            Box::pin(async move {
                let guard =
                    crate::row_id_allocator::prepend_allocated_row_id_bind(&mut tx, dialect, guard)
                        .await
                        .map_err(|_| SqlExecutorTransactionError::Sql)?;
                let guard_changed = tx
                    .execute_plan(&guard)
                    .await
                    .map_err(|_| SqlExecutorTransactionError::Sql)?;
                if guard_changed == 0 {
                    return Err(SqlExecutorTransactionError::Duplicate);
                }
                for statement in write_batch.statements {
                    let statement = crate::row_id_allocator::prepend_allocated_row_id_bind(
                        &mut tx, dialect, statement,
                    )
                    .await
                    .map_err(|_| SqlExecutorTransactionError::Sql)?;
                    tx.execute_plan(&statement)
                        .await
                        .map_err(|_| SqlExecutorTransactionError::Sql)?;
                }
                Ok(())
            })
        }) {
            Ok(()) => SqlTransactionOutcome::Committed,
            Err(error) => error.into_outcome(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqlxProtocolIngestUnitOfWork<E: SqlStatementExecutor = InMemorySqlStatementExecutor> {
    executor: E,
    planner: SqlProtocolIngestPlanner,
}

impl<E: SqlStatementExecutor> SqlxProtocolIngestUnitOfWork<E> {
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            planner: SqlProtocolIngestPlanner::standard(),
        }
    }

    pub fn with_planner(executor: E, planner: SqlProtocolIngestPlanner) -> Self {
        Self { executor, planner }
    }
}

impl<E: SqlStatementExecutor> AiotProtocolIngestUnitOfWork for SqlxProtocolIngestUnitOfWork<E> {
    fn execute_protocol_command(
        &self,
        command: &AiotProtocolStorageCommand,
    ) -> AiotStorageWriteReceipt {
        let plan = match self.planner.try_plan_protocol_command(command) {
            Ok(plan) => plan,
            Err(error) => return AiotStorageWriteReceipt::dead_lettered(error.code),
        };
        let outcome = self
            .executor
            .execute_transaction(plan.into_transaction_plan());

        match outcome {
            SqlTransactionOutcome::Committed => AiotStorageWriteReceipt::accepted(
                command.kind,
                command.primary_table,
                command
                    .outbox
                    .as_ref()
                    .map(|outbox| outbox.event_type.clone()),
            ),
            SqlTransactionOutcome::Duplicate => {
                let mut receipt = AiotStorageWriteReceipt::accepted(
                    command.kind,
                    command.primary_table,
                    command
                        .outbox
                        .as_ref()
                        .map(|outbox| outbox.event_type.clone()),
                );
                receipt.duplicate = true;
                receipt
            }
            SqlTransactionOutcome::RolledBack { reason_code } => {
                AiotStorageWriteReceipt::dead_lettered(reason_code)
            }
        }
    }

    fn record_dead_letter(&self, intent: &AiotProtocolDeadLetterIntent) -> AiotStorageWriteReceipt {
        let batch = match self.planner.try_plan_dead_letter(intent) {
            Ok(batch) => batch,
            Err(error) => return AiotStorageWriteReceipt::dead_lettered(error.code),
        };
        self.executor.execute_batch(batch);
        AiotStorageWriteReceipt::dead_lettered(intent.reason_code.clone())
    }
}

fn postgres_placeholder_count(sql: &str) -> usize {
    let mut max_placeholder = 0;
    let bytes = sql.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'$' {
            let mut number_index = index + 1;
            let mut value = 0_usize;
            while number_index < bytes.len() && bytes[number_index].is_ascii_digit() {
                value = value
                    .saturating_mul(10)
                    .saturating_add((bytes[number_index] - b'0') as usize);
                number_index += 1;
            }
            if number_index > index + 1 {
                max_placeholder = max_placeholder.max(value);
                index = number_index;
                continue;
            }
        }
        index += 1;
    }

    max_placeholder
}

fn sql_write_columns(sql: &str) -> Vec<String> {
    let trimmed = sql.trim_start();
    let upper = trimmed.to_ascii_uppercase();

    if upper.starts_with("INSERT INTO ") {
        return insert_write_columns(trimmed);
    }

    if upper.starts_with("UPDATE ") {
        return update_write_columns(trimmed);
    }

    Vec::new()
}

fn insert_write_columns(sql: &str) -> Vec<String> {
    let Some(start) = sql.find('(') else {
        return Vec::new();
    };
    let Some(end) = sql[start + 1..].find(')') else {
        return Vec::new();
    };

    comma_separated_identifiers(&sql[start + 1..start + 1 + end])
}

fn update_write_columns(sql: &str) -> Vec<String> {
    let Some(set_start) = find_ascii_case_insensitive(sql, " SET ") else {
        return Vec::new();
    };
    let after_set = set_start + " SET ".len();
    let where_start = find_ascii_case_insensitive(&sql[after_set..], " WHERE ")
        .map(|offset| after_set + offset)
        .unwrap_or(sql.len());

    sql[after_set..where_start]
        .split(',')
        .filter_map(|assignment| assignment.split_once('='))
        .map(|(column, _)| normalize_sql_identifier(column))
        .filter(|column| !column.is_empty())
        .collect()
}

fn comma_separated_identifiers(segment: &str) -> Vec<String> {
    segment
        .split(',')
        .map(normalize_sql_identifier)
        .filter(|column| !column.is_empty())
        .collect()
}

fn normalize_sql_identifier(identifier: &str) -> String {
    sdkwork_utils_rust::trim(identifier)
        .trim_matches('"')
        .trim_matches('`')
        .trim_matches('[')
        .trim_matches(']')
        .to_string()
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_uppercase()
        .find(&needle.to_ascii_uppercase())
}

fn initial_migration_declares_column(table: &str, column: &str) -> bool {
    let Some(definition) = initial_migration_table_definition(table) else {
        return false;
    };

    definition
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with(&format!("{column} ")))
}

fn initial_migration_table_definition(table: &str) -> Option<&'static str> {
    let sql = initial_migration_sql();
    let marker = format!("CREATE TABLE {table}");
    let start = sql.find(&marker)?;
    let rest = &sql[start + marker.len()..];
    let end = rest.find("\nCREATE TABLE ").unwrap_or(rest.len());

    Some(&sql[start..start + marker.len() + end])
}

fn device_create_statement(
    dialect: SqlDialect,
    device: &AiotDeviceRecord,
) -> Result<SqlStatementPlan, SqlPlanError> {
    let id = device
        .id
        .parse::<i64>()
        .map_err(|_| SqlPlanError::new("storage.sql.device.invalid_id").with_table("iot_device"))?;
    let product_id = parse_device_product_id(&device.product_id)?;
    let status = device_status_code(&device.status);
    let owner_id = format!(
        "{}:{}:{}",
        device.tenant_id, device.organization_id, device.device_id
    );
    let statement = SqlStatementPlan::bound(
        "device_create",
        "iot_device",
        dialect,
        format!(
            "INSERT INTO iot_device (id, uuid, tenant_id, organization_id, data_scope, owner_type, owner_id, device_key, product_id, display_name, device_id, client_id, chip_family, lifecycle_state, last_seen_at, metadata, status, created_at, updated_at, version, created_by, updated_by) VALUES ({})",
            dialect.placeholders(22)
        ),
        vec![
            SqlBindValue::Int64(id),
            SqlBindValue::text(format!("iot-device-{id}")),
            SqlBindValue::Int64(device.tenant_id),
            SqlBindValue::Int64(device.organization_id),
            SqlBindValue::Int64(0),
            SqlBindValue::text("device"),
            SqlBindValue::text(owner_id),
            SqlBindValue::text(&device.device_id),
            SqlBindValue::Int64(product_id),
            SqlBindValue::text(&device.display_name),
            SqlBindValue::text(&device.device_id),
            SqlBindValue::optional_text(device.client_id.as_deref()),
            SqlBindValue::optional_text(device.chip_family.as_deref()),
            SqlBindValue::Int64(0),
            SqlBindValue::optional_text(Some(&device.last_seen_at)),
            SqlBindValue::optional_text(device.metadata_json.as_deref()),
            SqlBindValue::Int64(status),
            SqlBindValue::text("2026-01-01T00:00:00Z"),
            SqlBindValue::text("2026-01-01T00:00:00Z"),
            SqlBindValue::Int64(0),
            SqlBindValue::Null,
            SqlBindValue::Null,
        ],
    );
    Ok(statement)
}

fn device_update_statement(
    dialect: SqlDialect,
    device: &AiotDeviceRecord,
) -> Result<SqlStatementPlan, SqlPlanError> {
    let status = device_status_code(&device.status);
    let statement = SqlStatementPlan::bound(
        "device_update",
        "iot_device",
        dialect,
        format!(
            "UPDATE iot_device SET display_name = {}, client_id = {}, chip_family = {}, status = {}, metadata = {}, updated_at = {} WHERE tenant_id = {} AND organization_id = {} AND device_id = {}",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3),
            dialect.placeholder(4),
            dialect.placeholder(5),
            dialect.placeholder(6),
            dialect.placeholder(7),
            dialect.placeholder(8),
            dialect.placeholder(9)
        ),
        vec![
            SqlBindValue::text(&device.display_name),
            SqlBindValue::optional_text(device.client_id.as_deref()),
            SqlBindValue::optional_text(device.chip_family.as_deref()),
            SqlBindValue::Int64(status),
            SqlBindValue::optional_text(device.metadata_json.as_deref()),
            SqlBindValue::text("2026-01-01T00:00:00Z"),
            SqlBindValue::Int64(device.tenant_id),
            SqlBindValue::Int64(device.organization_id),
            SqlBindValue::text(&device.device_id),
        ],
    );
    Ok(statement)
}

fn device_delete_statement(
    dialect: SqlDialect,
    association: &AiotStorageAssociation,
    device_id: &str,
) -> SqlStatementPlan {
    SqlStatementPlan::bound(
        "device_delete",
        "iot_device",
        dialect,
        format!(
            "DELETE FROM iot_device WHERE tenant_id = {} AND organization_id = {} AND device_id = {}",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3)
        ),
        vec![
            SqlBindValue::Int64(association.tenant_id),
            SqlBindValue::Int64(association.organization_id),
            SqlBindValue::text(device_id),
        ],
    )
}

fn device_status_code(status: &str) -> i64 {
    match status {
        "inactive" => 0,
        "active" => 1,
        "disabled" => 2,
        "deleted" => 3,
        _ => 1,
    }
}

fn parse_device_product_id(value: &str) -> Result<i64, SqlPlanError> {
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return Err(
            SqlPlanError::new("storage.sql.device.invalid_product_id").with_table("iot_device")
        );
    }

    value.parse::<i64>().map_err(|_| {
        SqlPlanError::new("storage.sql.device.invalid_product_id").with_table("iot_device")
    })
}

fn idempotency_guard_statement(
    dialect: SqlDialect,
    command: &AiotProtocolStorageCommand,
    idempotency_key: &str,
) -> SqlStatementPlan {
    let created_at = outbox::current_rfc3339_timestamp_for_insert();
    SqlStatementPlan::bound(
        "idempotency_guard",
        "iot_protocol_ingest_record",
        dialect,
        format!(
            "INSERT INTO iot_protocol_ingest_record (id, uuid, tenant_id, organization_id, data_scope, protocol_id, adapter_id, device_id, message_id, correlation_id, media_resource_id, object_blob_id, media_resource_snapshot, idempotency_key, trace_id, status, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}) ON CONFLICT DO NOTHING",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3),
            dialect.placeholder(4),
            dialect.placeholder(5),
            dialect.placeholder(6),
            dialect.placeholder(7),
            dialect.placeholder(8),
            dialect.placeholder(9),
            dialect.placeholder(10),
            dialect.placeholder(11),
            dialect.placeholder(12),
            dialect.placeholder(13),
            dialect.placeholder(14),
            dialect.placeholder(15),
            dialect.placeholder(16),
            dialect.placeholder(17),
            dialect.placeholder(18),
        ),
        vec![
            SqlBindValue::text(uuid()),
            SqlBindValue::Int64(command.association.tenant_id),
            SqlBindValue::Int64(command.association.organization_id),
            SqlBindValue::Int64(command.association.data_scope.into()),
            SqlBindValue::text(&command.protocol_id),
            SqlBindValue::text(&command.adapter_id),
            SqlBindValue::text(&command.device_id),
            SqlBindValue::optional_text(command.message_id.as_deref()),
            SqlBindValue::optional_text(command.correlation_id.as_deref()),
            SqlBindValue::optional_text(command.media_resource_id.as_deref()),
            SqlBindValue::optional_text(command.object_blob_id.as_deref()),
            SqlBindValue::optional_text(command.media_resource_snapshot.as_deref()),
            SqlBindValue::text(idempotency_key),
            SqlBindValue::optional_text(command.trace_id.as_deref()),
            SqlBindValue::Int64(0),
            SqlBindValue::text(&created_at),
            SqlBindValue::text(created_at),
        ],
    )
}

fn primary_write_statement(
    dialect: SqlDialect,
    command: &AiotProtocolStorageCommand,
    idempotency_key: &str,
) -> SqlStatementPlan {
    let placeholders = (1..=11)
        .map(|index| dialect.placeholder(index))
        .collect::<Vec<_>>();

    SqlStatementPlan::bound(
        "primary_write",
        "iot_protocol_ingest_record",
        dialect,
        format!(
            "UPDATE iot_protocol_ingest_record SET status = {}, media_resource_id = {}, object_blob_id = {}, media_resource_snapshot = {} WHERE tenant_id = {} AND organization_id = {} AND data_scope = {} AND protocol_id = {} AND adapter_id = {} AND device_id = {} AND idempotency_key = {}",
            placeholders[0],
            placeholders[1],
            placeholders[2],
            placeholders[3],
            placeholders[4],
            placeholders[5],
            placeholders[6],
            placeholders[7],
            placeholders[8],
            placeholders[9],
            placeholders[10]
        ),
        vec![
            SqlBindValue::Int64(1),
            SqlBindValue::optional_text(command.media_resource_id.as_deref()),
            SqlBindValue::optional_text(command.object_blob_id.as_deref()),
            SqlBindValue::optional_text(command.media_resource_snapshot.as_deref()),
            SqlBindValue::Int64(command.association.tenant_id),
            SqlBindValue::Int64(command.association.organization_id),
            SqlBindValue::Int64(command.association.data_scope.into()),
            SqlBindValue::text(&command.protocol_id),
            SqlBindValue::text(&command.adapter_id),
            SqlBindValue::text(&command.device_id),
            SqlBindValue::text(idempotency_key),
        ],
    )
}

fn outbox_write_statement(
    dialect: SqlDialect,
    command: &AiotProtocolStorageCommand,
) -> SqlStatementPlan {
    let outbox = command.outbox.as_ref().expect("outbox intent");
    let event_id = format!(
        "{}:{}:{}",
        outbox.aggregate_type, outbox.aggregate_id, outbox.event_type
    );
    let created_at = outbox::current_rfc3339_timestamp_for_insert();
    SqlStatementPlan::bound(
        "outbox_write",
        "iot_outbox_event",
        dialect,
        format!(
            "INSERT INTO iot_outbox_event (id, uuid, tenant_id, organization_id, data_scope, event_id, event_type, event_version, aggregate_type, aggregate_id, payload, payload_hash, status, trace_id, attempt_count, created_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3),
            dialect.placeholder(4),
            dialect.placeholder(5),
            dialect.placeholder(6),
            dialect.placeholder(7),
            dialect.placeholder(8),
            dialect.placeholder(9),
            dialect.placeholder(10),
            dialect.placeholder(11),
            dialect.placeholder(12),
            dialect.placeholder(13),
            dialect.placeholder(14),
            dialect.placeholder(15),
            dialect.placeholder(16),
        ),
        vec![
            SqlBindValue::text(uuid()),
            SqlBindValue::Int64(command.association.tenant_id),
            SqlBindValue::Int64(command.association.organization_id),
            SqlBindValue::Int64(command.association.data_scope.into()),
            SqlBindValue::text(event_id),
            SqlBindValue::text(&outbox.event_type),
            SqlBindValue::text(&outbox.event_version),
            SqlBindValue::text(&outbox.aggregate_type),
            SqlBindValue::text(&outbox.aggregate_id),
            SqlBindValue::text(&outbox.payload_json),
            SqlBindValue::optional_text(outbox.payload_hash.as_deref()),
            SqlBindValue::Int64(OUTBOX_STATUS_PENDING),
            SqlBindValue::optional_text(command.trace_id.as_deref()),
            SqlBindValue::Int64(0),
            SqlBindValue::text(created_at),
        ],
    )
}

fn dead_letter_write_statement(
    dialect: SqlDialect,
    intent: &AiotProtocolDeadLetterIntent,
) -> SqlStatementPlan {
    let created_at = outbox::current_rfc3339_timestamp_for_insert();
    SqlStatementPlan::bound(
        "dead_letter_write",
        "iot_protocol_message_dead_letter",
        dialect,
        format!(
            "INSERT INTO iot_protocol_message_dead_letter (id, uuid, tenant_id, organization_id, data_scope, protocol_id, adapter_id, device_id, reason_code, payload_ref, payload_hash, trace_id, status, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            dialect.placeholder(1),
            dialect.placeholder(2),
            dialect.placeholder(3),
            dialect.placeholder(4),
            dialect.placeholder(5),
            dialect.placeholder(6),
            dialect.placeholder(7),
            dialect.placeholder(8),
            dialect.placeholder(9),
            dialect.placeholder(10),
            dialect.placeholder(11),
            dialect.placeholder(12),
            dialect.placeholder(13),
            dialect.placeholder(14),
            dialect.placeholder(15),
        ),
        vec![
            SqlBindValue::text(uuid()),
            SqlBindValue::Int64(intent.association.tenant_id),
            SqlBindValue::Int64(intent.association.organization_id),
            SqlBindValue::Int64(intent.association.data_scope.into()),
            SqlBindValue::text(&intent.protocol_id),
            SqlBindValue::text(&intent.adapter_id),
            SqlBindValue::optional_text(intent.device_id.as_deref()),
            SqlBindValue::text(&intent.reason_code),
            SqlBindValue::optional_text(intent.payload_ref.as_deref()),
            SqlBindValue::optional_text(intent.payload_hash.as_deref()),
            SqlBindValue::optional_text(intent.trace_id.as_deref()),
            SqlBindValue::Int64(0),
            SqlBindValue::text(&created_at),
            SqlBindValue::text(created_at),
        ],
    )
}

pub fn initial_migration_sql() -> &'static str {
    r#"
CREATE TABLE iot_product (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    product_key VARCHAR(128) NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_product_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_product_tenant_key UNIQUE (tenant_id, product_key)
);

CREATE TABLE iot_hardware_profile (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    profile_key VARCHAR(128) NOT NULL,
    chip_family VARCHAR(64) NOT NULL,
    runtime_profile VARCHAR(64) NOT NULL,
    connectivity_profile VARCHAR(64) NOT NULL,
    security_profile VARCHAR(64),
    ota_profile VARCHAR(64),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_hardware_profile_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_hardware_profile_tenant_key UNIQUE (tenant_id, profile_key)
);

CREATE INDEX idx_iot_hardware_profile_tenant_chip
    ON iot_hardware_profile (tenant_id, chip_family, runtime_profile);

CREATE TABLE iot_protocol_profile (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    profile_key VARCHAR(128) NOT NULL,
    default_protocol_id VARCHAR(128) NOT NULL,
    allowed_transports TEXT NOT NULL,
    allowed_message_classes TEXT NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_protocol_profile_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_protocol_profile_tenant_key UNIQUE (tenant_id, profile_key)
);

CREATE INDEX idx_iot_protocol_profile_tenant_protocol
    ON iot_protocol_profile (tenant_id, default_protocol_id, status);

CREATE TABLE iot_capability_model (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    model_key VARCHAR(128) NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_capability_model_tenant_key UNIQUE (tenant_id, model_key)
);

CREATE TABLE iot_capability_definition (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    capability_model_id BIGINT NOT NULL,
    capability_name VARCHAR(128) NOT NULL,
    capability_kind VARCHAR(32) NOT NULL,
    schema_json TEXT NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_capability_definition_tenant_model_name
        UNIQUE (tenant_id, capability_model_id, capability_name)
);

CREATE TABLE iot_device (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    device_key VARCHAR(128) NOT NULL,
    product_id BIGINT NOT NULL,
    hardware_profile_id BIGINT,
    protocol_profile_id BIGINT,
    display_name VARCHAR(200) NOT NULL,
    device_id VARCHAR(128) NOT NULL,
    client_id VARCHAR(128),
    serial_number VARCHAR(128),
    mac_address VARCHAR(128),
    chip_family VARCHAR(64),
    runtime_profile VARCHAR(64),
    firmware_version VARCHAR(64),
    auth_state INTEGER NOT NULL DEFAULT 0,
    lifecycle_state INTEGER NOT NULL DEFAULT 0,
    last_seen_at TIMESTAMP,
    metadata TEXT,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    deleted_at TIMESTAMP,
    deleted_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_device_uuid UNIQUE (uuid),
    CONSTRAINT uk_iot_device_tenant_device_key UNIQUE (tenant_id, device_key),
    CONSTRAINT uk_iot_device_tenant_product_device_id UNIQUE (tenant_id, product_id, device_id),
    CONSTRAINT uk_iot_device_tenant_client_id UNIQUE (tenant_id, client_id)
);

CREATE INDEX idx_iot_device_tenant_product_status
    ON iot_device (tenant_id, product_id, status);

CREATE INDEX idx_iot_device_tenant_last_seen
    ON iot_device (tenant_id, last_seen_at);

CREATE TABLE iot_device_credential (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    credential_type VARCHAR(64) NOT NULL,
    credential_hash VARCHAR(256),
    credential_ref VARCHAR(512),
    expires_at TIMESTAMP,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_device_credential_tenant_device_status
    ON iot_device_credential (tenant_id, device_id, status);

CREATE TABLE iot_device_binding (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    binding_type VARCHAR(64) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(128) NOT NULL,
    role VARCHAR(64),
    status INTEGER NOT NULL,
    bound_at TIMESTAMP NOT NULL,
    bound_by BIGINT,
    expires_at TIMESTAMP,
    metadata TEXT,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_device_binding_tenant_target
    ON iot_device_binding (tenant_id, target_type, target_id, status);

CREATE TABLE iot_gateway_child_device (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    gateway_device_id VARCHAR(128) NOT NULL,
    child_device_id VARCHAR(128) NOT NULL,
    topology_role VARCHAR(64) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_gateway_child_device_tenant_pair
        UNIQUE (tenant_id, gateway_device_id, child_device_id)
);

CREATE TABLE iot_device_connection (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    connection_id VARCHAR(128) NOT NULL,
    device_id VARCHAR(128),
    transport VARCHAR(64) NOT NULL,
    remote_addr VARCHAR(256),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_device_connection_tenant_connection UNIQUE (tenant_id, connection_id)
);

CREATE INDEX idx_iot_device_connection_tenant_device_created
    ON iot_device_connection (tenant_id, device_id, created_at);

CREATE TABLE iot_device_session (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    session_id VARCHAR(128) NOT NULL,
    connection_id VARCHAR(128) NOT NULL,
    protocol_id VARCHAR(128) NOT NULL,
    adapter_id VARCHAR(128) NOT NULL,
    node_id VARCHAR(128),
    status INTEGER NOT NULL,
    connected_at TIMESTAMP NOT NULL,
    last_seen_at TIMESTAMP,
    disconnected_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_device_session_tenant_session UNIQUE (tenant_id, session_id)
);

CREATE INDEX idx_iot_device_session_tenant_device_status
    ON iot_device_session (tenant_id, device_id, status);

CREATE TABLE iot_device_online_lease (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    session_id VARCHAR(128) NOT NULL,
    node_id VARCHAR(128) NOT NULL,
    lease_expires_at TIMESTAMP NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_device_online_lease_tenant_device UNIQUE (tenant_id, device_id)
);

CREATE INDEX idx_iot_device_online_lease_tenant_expires
    ON iot_device_online_lease (tenant_id, lease_expires_at);

CREATE TABLE iot_command (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    command_id VARCHAR(128) NOT NULL,
    device_id VARCHAR(128) NOT NULL,
    session_id VARCHAR(128),
    capability_name VARCHAR(128) NOT NULL,
    command_name VARCHAR(128) NOT NULL,
    request_payload TEXT NOT NULL,
    request_media_resource_id VARCHAR(128),
    request_object_blob_id VARCHAR(128),
    request_media_resource_snapshot TEXT,
    status INTEGER NOT NULL,
    idempotency_key VARCHAR(128),
    timeout_at TIMESTAMP,
    ack_at TIMESTAMP,
    result_at TIMESTAMP,
    trace_id VARCHAR(128),
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_command_tenant_command_id UNIQUE (tenant_id, command_id),
    CONSTRAINT uk_iot_command_tenant_idempotency_key
        UNIQUE (tenant_id, organization_id, idempotency_key)
);

CREATE INDEX idx_iot_command_tenant_device_status_created
    ON iot_command (tenant_id, device_id, status, created_at);

CREATE INDEX idx_iot_command_tenant_status_timeout
    ON iot_command (tenant_id, status, timeout_at);

CREATE TABLE iot_command_delivery (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    command_id VARCHAR(128) NOT NULL,
    session_id VARCHAR(128),
    delivery_state VARCHAR(64) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_command_delivery_tenant_session_status
    ON iot_command_delivery (tenant_id, session_id, status);

CREATE TABLE iot_command_result (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    command_id VARCHAR(128) NOT NULL,
    result_payload TEXT,
    result_media_resource_id VARCHAR(128),
    result_object_blob_id VARCHAR(128),
    result_media_resource_snapshot TEXT,
    result_code VARCHAR(128),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_command_result_tenant_command
    ON iot_command_result (tenant_id, command_id);

CREATE TABLE iot_device_twin (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    desired_version BIGINT NOT NULL DEFAULT 0,
    reported_version BIGINT NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_device_twin_tenant_device UNIQUE (tenant_id, device_id)
);

CREATE TABLE iot_device_twin_property (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    property_name VARCHAR(128) NOT NULL,
    desired_value TEXT,
    desired_version BIGINT NOT NULL DEFAULT 0,
    desired_updated_at TIMESTAMP,
    reported_value TEXT,
    reported_version BIGINT NOT NULL DEFAULT 0,
    reported_updated_at TIMESTAMP,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_twin_property_tenant_device_property
        UNIQUE (tenant_id, device_id, property_name)
);

CREATE INDEX idx_iot_twin_property_tenant_device_property
    ON iot_device_twin_property (tenant_id, device_id, property_name);

CREATE TABLE iot_telemetry_latest (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    metric_key VARCHAR(128) NOT NULL,
    metric_value TEXT NOT NULL,
    metric_type VARCHAR(32) NOT NULL,
    measured_at TIMESTAMP NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_telemetry_latest_tenant_device_key
        UNIQUE (tenant_id, device_id, metric_key)
);

CREATE INDEX idx_iot_telemetry_latest_tenant_device_key
    ON iot_telemetry_latest (tenant_id, device_id, metric_key);

CREATE TABLE iot_telemetry_event (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    metric_key VARCHAR(128) NOT NULL,
    metric_value TEXT NOT NULL,
    measured_at TIMESTAMP NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_telemetry_event_tenant_device_time
    ON iot_telemetry_event (tenant_id, device_id, measured_at);

CREATE TABLE iot_device_event (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    device_id VARCHAR(128) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    event_payload TEXT NOT NULL,
    media_resource_id VARCHAR(128),
    object_blob_id VARCHAR(128),
    media_resource_snapshot TEXT,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_device_event_tenant_device_time
    ON iot_device_event (tenant_id, device_id, created_at);

CREATE TABLE iot_security_event (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    security_event_type VARCHAR(128) NOT NULL,
    severity VARCHAR(64) NOT NULL,
    actor_type VARCHAR(64),
    actor_id VARCHAR(128),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    trace_id VARCHAR(128),
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_security_event_tenant_time
    ON iot_security_event (tenant_id, created_at);

CREATE TABLE iot_media_resource (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    media_resource_id VARCHAR(128) NOT NULL,
    kind VARCHAR(32) NOT NULL,
    source VARCHAR(32) NOT NULL,
    object_blob_id VARCHAR(128),
    resource_snapshot TEXT,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_media_resource_tenant_resource_id
        UNIQUE (tenant_id, media_resource_id)
);

CREATE INDEX idx_iot_media_resource_tenant_owner
    ON iot_media_resource (tenant_id, owner_type, owner_id, status);

CREATE INDEX idx_iot_media_resource_tenant_object_blob
    ON iot_media_resource (tenant_id, object_blob_id);

CREATE TABLE iot_device_media (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    media_role VARCHAR(64) NOT NULL,
    media_resource_id VARCHAR(128) NOT NULL,
    object_blob_id VARCHAR(128),
    resource_snapshot TEXT,
    alt_text VARCHAR(512),
    sort_order INTEGER NOT NULL DEFAULT 0,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_device_media_tenant_owner_role
    ON iot_device_media (tenant_id, owner_type, owner_id, media_role, sort_order);

CREATE INDEX idx_iot_device_media_tenant_media
    ON iot_device_media (tenant_id, media_resource_id);

CREATE TABLE iot_firmware_artifact (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    version_name VARCHAR(128) NOT NULL,
    media_resource_id VARCHAR(128) NOT NULL,
    object_blob_id VARCHAR(128),
    media_resource_snapshot TEXT,
    file_name VARCHAR(256),
    size_bytes BIGINT NOT NULL,
    sha256 VARCHAR(128) NOT NULL,
    signature TEXT,
    signature_algorithm VARCHAR(64),
    target_chip_family VARCHAR(64),
    target_runtime_profile VARCHAR(64),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_firmware_artifact_tenant_media_resource
        UNIQUE (tenant_id, media_resource_id)
);

CREATE TABLE iot_firmware_rollout (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    owner_type VARCHAR(32) NOT NULL,
    owner_id VARCHAR(128) NOT NULL,
    artifact_id BIGINT NOT NULL,
    rollout_policy TEXT NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    created_by BIGINT,
    updated_by BIGINT,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_firmware_rollout_tenant_status
    ON iot_firmware_rollout (tenant_id, status);

CREATE TABLE iot_firmware_rollout_target (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    rollout_id BIGINT NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(128) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_firmware_rollout_target_tenant_rollout
    ON iot_firmware_rollout_target (tenant_id, rollout_id);

CREATE TABLE iot_firmware_deployment (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    rollout_id BIGINT,
    device_id VARCHAR(128) NOT NULL,
    deployment_state VARCHAR(64) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_firmware_deployment_tenant_device_status
    ON iot_firmware_deployment (tenant_id, device_id, status);

CREATE TABLE iot_provisioning_challenge (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    challenge_id VARCHAR(128) NOT NULL,
    device_hint VARCHAR(128),
    expires_at TIMESTAMP NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_provisioning_challenge_tenant_id UNIQUE (tenant_id, challenge_id)
);

CREATE INDEX idx_iot_provisioning_challenge_tenant_expires
    ON iot_provisioning_challenge (tenant_id, expires_at);

CREATE TABLE iot_activation_record (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    activation_id VARCHAR(128) NOT NULL,
    device_id VARCHAR(128),
    activation_profile VARCHAR(128) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_activation_record_tenant_device
    ON iot_activation_record (tenant_id, device_id);

CREATE TABLE iot_protocol_message_dead_letter (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    protocol_id VARCHAR(128) NOT NULL,
    adapter_id VARCHAR(128) NOT NULL,
    device_id VARCHAR(128),
    reason_code VARCHAR(128) NOT NULL,
    payload_ref VARCHAR(512),
    payload_hash VARCHAR(128),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    trace_id VARCHAR(128),
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_protocol_dead_letter_tenant_created
    ON iot_protocol_message_dead_letter (tenant_id, created_at);

CREATE TABLE iot_outbox_event (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    event_id VARCHAR(128) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    event_version VARCHAR(16) NOT NULL DEFAULT '1',
    aggregate_type VARCHAR(128) NOT NULL,
    aggregate_id VARCHAR(128) NOT NULL,
    payload TEXT NOT NULL,
    payload_hash VARCHAR(128),
    status INTEGER NOT NULL,
    next_attempt_at TIMESTAMP,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL,
    published_at TIMESTAMP,
    trace_id VARCHAR(128),
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_outbox_event_tenant_event_id UNIQUE (tenant_id, event_id)
);

CREATE INDEX idx_iot_outbox_event_status_next_attempt
    ON iot_outbox_event (status, next_attempt_at);

CREATE TABLE iot_inbox_event (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    source_system VARCHAR(128) NOT NULL,
    message_id VARCHAR(128) NOT NULL,
    consumer_name VARCHAR(128) NOT NULL,
    payload_hash VARCHAR(128),
    error_message VARCHAR(1000),
    processed_at TIMESTAMP,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_inbox_event_consumer_message
        UNIQUE (source_system, message_id, consumer_name)
);

CREATE TABLE iot_audit_log (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    operator_id BIGINT,
    action VARCHAR(128) NOT NULL,
    target_type VARCHAR(128) NOT NULL,
    target_id VARCHAR(128) NOT NULL,
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    trace_id VARCHAR(128),
    PRIMARY KEY (id)
);

CREATE INDEX idx_iot_audit_log_tenant_created
    ON iot_audit_log (tenant_id, created_at);

CREATE TABLE iot_protocol_ingest_record (
    id BIGINT NOT NULL,
    uuid VARCHAR(64) NOT NULL,
    tenant_id BIGINT NOT NULL,
    organization_id BIGINT NOT NULL DEFAULT 0,
    data_scope INTEGER NOT NULL DEFAULT 0,
    protocol_id VARCHAR(128) NOT NULL,
    adapter_id VARCHAR(128) NOT NULL,
    device_id VARCHAR(128),
    message_id VARCHAR(128),
    correlation_id VARCHAR(128),
    media_resource_id VARCHAR(128),
    object_blob_id VARCHAR(128),
    media_resource_snapshot TEXT,
    idempotency_key VARCHAR(256) NOT NULL,
    trace_id VARCHAR(128),
    status INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    version BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    CONSTRAINT uk_iot_protocol_ingest_tenant_idempotency
        UNIQUE (tenant_id, idempotency_key)
);

CREATE INDEX idx_iot_protocol_ingest_tenant_message
    ON iot_protocol_ingest_record (tenant_id, protocol_id, message_id);
"#
}

#[cfg(test)]
mod protocol_ingest_sqlite_tests {
    use sdkwork_aiot_storage::{
        AiotOutboxWriteIntent, AiotProtocolStorageCommand, AiotStorageWriteKind,
    };

    use sdkwork_aiot_storage::AiotProtocolIngestUnitOfWork;

    use crate::schema::ensure_device_schema;
    use crate::sqlite_sync::sqlite_connect_url;
    use crate::{
        BlockingDevicePool, SqlDialect, SqlProtocolIngestPlanner, SqlxPoolSqlStatementExecutor,
    };

    #[test]
    fn protocol_ingest_statements_execute_on_sqlite() {
        let command = AiotProtocolStorageCommand::new(
            "xiaozhi.websocket",
            "xiaozhi",
            "device-outbox-001",
            AiotStorageWriteKind::OpenSession,
            "iot_device_session",
        )
        .with_session_id("session-outbox-001")
        .with_idempotency_key("outbox-idem-001")
        .with_outbox(AiotOutboxWriteIntent::new(
            "iot.device.session.started",
            "device_session",
            "session-outbox-001",
            "iot.protocol.ingested",
        ));
        SqlProtocolIngestPlanner::for_dialect(SqlDialect::Sqlite)
            .try_plan_protocol_command(&command)
            .expect("plan must validate with runtime row-id binds");
        let url = sqlite_connect_url("file:protocol-ingest-sqlite-test?mode=memory&cache=shared");
        let db = crate::sqlite_sync::BlockingSqlitePool::connect(&url).expect("connect");
        let pool = BlockingDevicePool::Sqlite(db);
        ensure_device_schema(&pool).expect("schema");

        let executor = SqlxPoolSqlStatementExecutor::new(pool);
        let receipt = executor
            .protocol_ingest_unit_of_work()
            .execute_protocol_command(&command);
        assert!(
            receipt.accepted,
            "protocol ingest must commit: {:?}",
            receipt
        );
    }
}
