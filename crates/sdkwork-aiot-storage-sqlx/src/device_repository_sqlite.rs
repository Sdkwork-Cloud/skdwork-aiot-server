use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use sdkwork_aiot_storage::{
    AiotCommandCreateCommand, AiotCommandDeliveryEnqueueCommand, AiotCommandDeliveryRecord,
    AiotCommandDeliveryRepository, AiotCommandDeliveryRepositoryError, AiotCommandRecord,
    AiotCommandRepository, AiotCommandRepositoryError, AiotCommandResultRecord,
    AiotDeviceCreateCommand, AiotDeviceEventCreateCommand, AiotDeviceEventRecord, AiotDeviceRecord,
    AiotDeviceRepository, AiotDeviceRepositoryError, AiotDeviceSessionRecord,
    AiotDeviceSessionRepository, AiotDeviceTwinRepository, AiotDeviceTwinRepositoryError,
    AiotDeviceTwinSnapshot, AiotDeviceUpdateCommand, AiotEventRepository, AiotEventRepositoryError,
    AiotOffsetListResult, AiotStorageAssociation, AiotTwinPropertyUpsertCommand,
    OffsetListPageParams, OUTBOX_STATUS_PENDING,
};
use sdkwork_utils_rust::uuid;
use serde_json::Value as JsonValue;
use sqlx::Row;

use crate::blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine, DeviceDbTransaction};
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::schema::ensure_device_schema;
use crate::sqlite_sync::{sqlite_connect_url, BlockingSqlitePool};
use crate::{
    command_status_code, command_status_text, default_timestamp, device_status_text,
    is_valid_int64_string, SqlDeviceRepositoryPlanner, SqlDialect, SqlStatementBatch,
};

type CommandIdempotencyCache = HashMap<(i64, i64, String), String>;
type TwinPropertyState = (i64, Option<String>, i64, Option<String>, i64);

enum SqliteRepoTxError {
    Device(AiotDeviceRepositoryError),
    Command(AiotCommandRepositoryError),
    Delivery(AiotCommandDeliveryRepositoryError),
    Event(AiotEventRepositoryError),
    Twin(AiotDeviceTwinRepositoryError),
    Storage,
}

impl From<sqlx::Error> for SqliteRepoTxError {
    fn from(_error: sqlx::Error) -> Self {
        Self::Storage
    }
}

fn map_device_sql_error(_: sqlx::Error) -> AiotDeviceRepositoryError {
    AiotDeviceRepositoryError::PersistenceFailure
}

fn map_command_sql_error(_: sqlx::Error) -> AiotCommandRepositoryError {
    AiotCommandRepositoryError::PersistenceFailure
}

fn map_delivery_sql_error(_: sqlx::Error) -> AiotCommandDeliveryRepositoryError {
    AiotCommandDeliveryRepositoryError::PersistenceFailure
}

fn map_event_sql_error(_: sqlx::Error) -> AiotEventRepositoryError {
    AiotEventRepositoryError::PersistenceFailure
}

impl SqliteRepoTxError {
    fn into_device(self) -> AiotDeviceRepositoryError {
        match self {
            Self::Device(error) => error,
            Self::Command(_)
            | Self::Delivery(_)
            | Self::Event(_)
            | Self::Twin(_)
            | Self::Storage => AiotDeviceRepositoryError::PersistenceFailure,
        }
    }

    fn into_command(self) -> AiotCommandRepositoryError {
        match self {
            Self::Command(error) => error,
            Self::Device(_)
            | Self::Delivery(_)
            | Self::Event(_)
            | Self::Twin(_)
            | Self::Storage => AiotCommandRepositoryError::PersistenceFailure,
        }
    }

    fn into_delivery(self) -> AiotCommandDeliveryRepositoryError {
        match self {
            Self::Delivery(error) => error,
            Self::Device(_) | Self::Command(_) | Self::Event(_) | Self::Twin(_) | Self::Storage => {
                AiotCommandDeliveryRepositoryError::PersistenceFailure
            }
        }
    }

    fn into_event(self) -> AiotEventRepositoryError {
        match self {
            Self::Event(error) => error,
            Self::Device(_)
            | Self::Command(_)
            | Self::Delivery(_)
            | Self::Twin(_)
            | Self::Storage => AiotEventRepositoryError::PersistenceFailure,
        }
    }

    fn into_twin(self) -> AiotDeviceTwinRepositoryError {
        match self {
            Self::Twin(error) => error,
            Self::Device(_)
            | Self::Command(_)
            | Self::Delivery(_)
            | Self::Event(_)
            | Self::Storage => AiotDeviceTwinRepositoryError::PersistenceFailure,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqliteSqlxDeviceRepository {
    db: BlockingDevicePool,
    planner: SqlDeviceRepositoryPlanner,
    command_idempotency_cache: Arc<Mutex<CommandIdempotencyCache>>,
}

impl SqliteSqlxDeviceRepository {
    pub fn new_in_memory() -> Result<Self, sqlx::Error> {
        Self::open("file:sdkwork-aiot-device-repo?mode=memory&cache=shared")
    }

    pub fn from_blocking_pool(db: BlockingDevicePool) -> Result<Self, sqlx::Error> {
        ensure_device_schema(&db)?;
        let dialect = db.dialect();
        Ok(Self {
            db,
            planner: SqlDeviceRepositoryPlanner::with_dialect(dialect),
            command_idempotency_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn open(path_or_uri: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url(path_or_uri.as_ref().to_string_lossy().as_ref());
        let db = BlockingSqlitePool::connect(&url)?;
        Self::from_blocking_pool(BlockingDevicePool::Sqlite(db))
    }

    fn execute_batch(&self, batch: SqlStatementBatch) -> Result<(), sqlx::Error> {
        self.db.execute_statement_batch(batch)
    }

    pub fn get_command_by_id(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let command_id = command_id.to_string();
        self.db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT id, command_id, device_id, session_id, capability_name, command_name, request_payload, request_media_resource_id, request_object_blob_id, request_media_resource_snapshot, status, timeout_at, ack_at, result_at, trace_id, created_at FROM iot_command WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND command_id = ?4 LIMIT 1",
                );
                let mut command = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&command_id)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        row.as_ref()
                            .map(|row| row_to_command_record(row, &association))
                            .transpose()?
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&command_id)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        row.as_ref()
                            .map(|row| row_to_command_record_postgres(row, &association))
                            .transpose()?
                    }
                };
                if let Some(command) = command.as_mut() {
                    command.result = command_result_for(
                        &pool,
                        association.tenant_id,
                        association.organization_id,
                        &command.command_id,
                    )
                    .await?;
                }
                Ok(command)
            })
            .map_err(map_command_sql_error)
    }
}

impl AiotDeviceRepository for SqliteSqlxDeviceRepository {
    fn storage_ready(&self) -> bool {
        self.db
            .run_owned(|pool| async move {
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query_scalar::<_, i64>("SELECT 1")
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query_scalar::<_, i64>("SELECT 1")
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await
                    }
                }
            })
            .is_ok()
    }

    fn create_device(
        &self,
        command: AiotDeviceCreateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        if !is_valid_int64_string(&command.product_id) {
            return Err(AiotDeviceRepositoryError::InvalidProductId);
        }

        let planner = self.planner;
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let exists_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT COUNT(1) FROM iot_device WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
                    );
                    let exists: i64 = match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query_scalar(&exists_sql)
                                .bind(command.association.tenant_id)
                                .bind(command.association.organization_id)
                                .bind(&command.device_id)
                                .fetch_one(&mut **connection)
                                .await
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query_scalar(&exists_sql)
                                .bind(command.association.tenant_id)
                                .bind(command.association.organization_id)
                                .bind(&command.device_id)
                                .fetch_one(&mut **connection)
                                .await
                        }
                    }
                    .map_err(|_| SqliteRepoTxError::Device(AiotDeviceRepositoryError::PersistenceFailure))?;
                    if exists > 0 {
                        return Err(SqliteRepoTxError::Device(
                            AiotDeviceRepositoryError::DuplicateDeviceId,
                        ));
                    }

                    let max_id_sql =
                        adapt_sqlite_placeholders(dialect, "SELECT COALESCE(MAX(id), 0) FROM iot_device");
                    let max_id: i64 = match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                        }
                    }
                    .map_err(|_| SqliteRepoTxError::Device(AiotDeviceRepositoryError::PersistenceFailure))?;
                    let next_id = max_id + 1;

                    let record = AiotDeviceRecord {
                        id: next_id.to_string(),
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
                    let batch = planner
                        .plan_create_device(&record)
                        .map_err(|_| SqliteRepoTxError::Device(AiotDeviceRepositoryError::PersistenceFailure))?;
                    for statement in batch.statements {
                        tx.execute_plan(&statement)
                            .await
                            .map_err(|_| SqliteRepoTxError::Device(AiotDeviceRepositoryError::PersistenceFailure))?;
                    }
                    Ok(record)
                })
            })
            .map_err(SqliteRepoTxError::into_device)
    }

    fn get_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Option<AiotDeviceRecord> {
        let association = association.clone();
        let device_id = device_id.to_string();
        self.db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT id, tenant_id, organization_id, device_id, display_name, product_id, client_id, chip_family, status, metadata, last_seen_at FROM iot_device WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 LIMIT 1",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        row.as_ref().map(row_to_device_record).transpose()
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        row.as_ref()
                            .map(row_to_device_record_postgres)
                            .transpose()
                    }
                }
            })
            .ok()
            .flatten()
    }

    fn list_devices(
        &self,
        association: &AiotStorageAssociation,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceRecord>, AiotDeviceRepositoryError> {
        let association = association.clone();
        self.db
            .run_owned(|pool| async move {
                let count_sql = pool.adapt_sql(
                    "SELECT COUNT(1) FROM iot_device WHERE tenant_id = ?1 AND organization_id = ?2",
                );
                let list_sql = pool.adapt_sql(
                    "SELECT id, tenant_id, organization_id, device_id, display_name, product_id, client_id, chip_family, status, metadata, last_seen_at FROM iot_device WHERE tenant_id = ?1 AND organization_id = ?2 ORDER BY id ASC LIMIT ?3 OFFSET ?4",
                );
                let limit = params.page_size.max(1);
                let offset = params.offset.max(0);
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<AiotOffsetListResult<AiotDeviceRecord>, sqlx::Error>(AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_device_record(row).ok())
                                .collect(),
                            total,
                        })
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok(AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_device_record_postgres(row).ok())
                                .collect(),
                            total,
                        })
                    }
                }
            })
            .map_err(map_device_sql_error)
    }

    fn list_device_ids_for_rollout(
        &self,
        association: &AiotStorageAssociation,
        product_id: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<String>, AiotDeviceRepositoryError> {
        let association = association.clone();
        let product_id = product_id.map(str::to_string);
        self.db
            .run_owned(|pool| async move {
                let product_filter = product_id
                    .as_deref()
                    .map(|value| value.parse::<i64>())
                    .transpose()
                    .map_err(|_| sqlx::Error::RowNotFound)?;
                let mut sql = String::from(
                    "SELECT device_id FROM iot_device WHERE tenant_id = ?1 AND organization_id = ?2",
                );
                if product_filter.is_some() {
                    sql.push_str(" AND product_id = ?3");
                }
                sql.push_str(" ORDER BY id ASC");
                if limit.is_some() {
                    sql.push_str(if product_filter.is_some() {
                        " LIMIT ?4"
                    } else {
                        " LIMIT ?3"
                    });
                }
                let sql = pool.adapt_sql(&sql);
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let mut query = sqlx::query_scalar::<_, String>(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id);
                        if let Some(product_id) = product_filter {
                            query = query.bind(product_id);
                        }
                        if let Some(limit) = limit {
                            query = query.bind(limit.max(1));
                        }
                        query
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let mut query = sqlx::query_scalar::<_, String>(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id);
                        if let Some(product_id) = product_filter {
                            query = query.bind(product_id);
                        }
                        if let Some(limit) = limit {
                            query = query.bind(limit.max(1));
                        }
                        query
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await
                    }
                }
            })
            .map_err(map_device_sql_error)
    }

    fn update_device(
        &self,
        command: AiotDeviceUpdateCommand,
    ) -> Result<AiotDeviceRecord, AiotDeviceRepositoryError> {
        let Some(mut existing) = self.get_device(&command.association, &command.device_id) else {
            return Err(AiotDeviceRepositoryError::NotFound);
        };
        if let Some(display_name) = command.display_name {
            existing.display_name = display_name;
        }
        if let Some(status) = command.status {
            existing.status = status;
        }
        if command.metadata_json.is_some() {
            existing.metadata_json = command.metadata_json;
        }
        let batch = self
            .planner
            .plan_update_device(&existing)
            .map_err(|_| AiotDeviceRepositoryError::PersistenceFailure)?;
        self.execute_batch(batch).map_err(map_device_sql_error)?;
        Ok(existing)
    }

    fn delete_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<(), AiotDeviceRepositoryError> {
        if self.get_device(association, device_id).is_none() {
            return Err(AiotDeviceRepositoryError::NotFound);
        }
        let batch = self
            .planner
            .plan_delete_device(association, device_id)
            .map_err(|_| AiotDeviceRepositoryError::PersistenceFailure)?;
        self.execute_batch(batch).map_err(map_device_sql_error)?;
        Ok(())
    }
}

impl AiotCommandRepository for SqliteSqlxDeviceRepository {
    fn create_command(
        &self,
        command: AiotCommandCreateCommand,
    ) -> Result<AiotCommandRecord, AiotCommandRepositoryError> {
        if let Some(idempotency_key) = command.idempotency_key.as_deref() {
            let cache_key = (
                command.association.tenant_id,
                command.association.organization_id,
                idempotency_key.to_string(),
            );
            if let Some(existing_command_id) = self
                .command_idempotency_cache
                .lock()
                .expect("sqlite command idempotency cache poisoned")
                .get(&cache_key)
                .cloned()
            {
                if let Some(existing) = self.get_command_by_id(
                    &command.association,
                    &command.device_id,
                    &existing_command_id,
                )? {
                    return Ok(existing);
                }
            }
        }

        let request_media_snapshot = command.request_media_json.clone();
        let status_code = command_status_code(&command.status);
        let created_at = default_timestamp().to_string();
        let trace_id = command.trace_id.clone();
        let idempotency_key = command.idempotency_key.clone();
        let association = command.association.clone();
        let device_id = command.device_id.clone();
        let session_id = command.session_id.clone();
        let capability_name = command.capability_name.clone();
        let command_name = command.command_name.clone();
        let request_payload_json = command.request_payload_json.clone();
        let request_media_resource_id = command.request_media_resource_id.clone();
        let request_object_blob_id = command.request_object_blob_id.clone();
        let timeout_at = command.timeout_at.clone();
        let status = command.status.clone();

        let (next_id, command_id) = self.db.with_device_transaction(|mut tx, dialect| {
            let command = command;
            Box::pin(async move {
                let created_at = default_timestamp().to_string();
                let max_id_sql =
                    adapt_sqlite_placeholders(dialect, "SELECT COALESCE(MAX(id), 0) FROM iot_command");
                let next_id: i64 = match &mut tx {
                    DeviceDbTransaction::Sqlite(connection) => {
                        sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                    }
                    DeviceDbTransaction::Postgres(connection) => {
                        sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                    }
                }
                .map_err(|_| SqliteRepoTxError::Command(AiotCommandRepositoryError::PersistenceFailure))?;

                let command_id = command.command_id.unwrap_or_else(|| {
                    format!("cmd-{}-{:04}", command.device_id, next_id + 1)
                });

                let duplicate_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT COUNT(1) FROM iot_command WHERE tenant_id = ?1 AND command_id = ?2",
                );
                let duplicate_count: i64 = match &mut tx {
                    DeviceDbTransaction::Sqlite(connection) => {
                        sqlx::query_scalar(&duplicate_sql)
                            .bind(command.association.tenant_id)
                            .bind(&command_id)
                            .fetch_one(&mut **connection)
                            .await
                    }
                    DeviceDbTransaction::Postgres(connection) => {
                        sqlx::query_scalar(&duplicate_sql)
                            .bind(command.association.tenant_id)
                            .bind(&command_id)
                            .fetch_one(&mut **connection)
                            .await
                    }
                }
                .map_err(|_| SqliteRepoTxError::Command(AiotCommandRepositoryError::PersistenceFailure))?;
                if duplicate_count > 0 {
                    return Err(SqliteRepoTxError::Command(
                        AiotCommandRepositoryError::DuplicateCommandId,
                    ));
                }

                let insert_sql = adapt_sqlite_placeholders(
                    dialect,
                    "INSERT INTO iot_command (id, uuid, tenant_id, organization_id, data_scope, command_id, device_id, session_id, capability_name, command_name, request_payload, request_media_resource_id, request_object_blob_id, request_media_resource_snapshot, status, idempotency_key, timeout_at, ack_at, result_at, trace_id, created_at, updated_at, version, created_by, updated_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, NULL, NULL, ?18, ?19, ?20, 0, ?21, ?22)",
                );
                match &mut tx {
                    DeviceDbTransaction::Sqlite(connection) => {
                        sqlx::query(&insert_sql)
                            .bind(next_id + 1)
                            .bind(format!("cmd-uuid-{}", next_id + 1))
                            .bind(command.association.tenant_id)
                            .bind(command.association.organization_id)
                            .bind(command.association.data_scope as i64)
                            .bind(&command_id)
                            .bind(&command.device_id)
                            .bind(command.session_id.as_deref())
                            .bind(&command.capability_name)
                            .bind(&command.command_name)
                            .bind(&command.request_payload_json)
                            .bind(command.request_media_resource_id.as_deref())
                            .bind(command.request_object_blob_id.as_deref())
                            .bind(command.request_media_json.as_deref())
                            .bind(status_code)
                            .bind(command.idempotency_key.as_deref())
                            .bind(command.timeout_at.as_deref())
                            .bind(command.trace_id.as_deref())
                            .bind(&created_at)
                            .bind(&created_at)
                            .bind(command.association.created_by)
                            .bind(command.association.updated_by)
                            .execute(&mut **connection)
                            .await?;
                    }
                    DeviceDbTransaction::Postgres(connection) => {
                        sqlx::query(&insert_sql)
                            .bind(next_id + 1)
                            .bind(format!("cmd-uuid-{}", next_id + 1))
                            .bind(command.association.tenant_id)
                            .bind(command.association.organization_id)
                            .bind(command.association.data_scope as i64)
                            .bind(&command_id)
                            .bind(&command.device_id)
                            .bind(command.session_id.as_deref())
                            .bind(&command.capability_name)
                            .bind(&command.command_name)
                            .bind(&command.request_payload_json)
                            .bind(command.request_media_resource_id.as_deref())
                            .bind(command.request_object_blob_id.as_deref())
                            .bind(command.request_media_json.as_deref())
                            .bind(status_code)
                            .bind(command.idempotency_key.as_deref())
                            .bind(command.timeout_at.as_deref())
                            .bind(command.trace_id.as_deref())
                            .bind(&created_at)
                            .bind(&created_at)
                            .bind(command.association.created_by)
                            .bind(command.association.updated_by)
                            .execute(&mut **connection)
                            .await?;
                    }
                }

                insert_command_delivery_and_outbox(
                    &mut tx,
                    dialect,
                    &command.association,
                    &command_id,
                    command.session_id.as_deref(),
                    &command.device_id,
                    &command.capability_name,
                    &command.command_name,
                    command.trace_id.as_deref(),
                    &created_at,
                )
                .await?;

                Ok((next_id, command_id))
            })
        })
        .map_err(SqliteRepoTxError::into_command)?;

        let command_id_for_cache = command_id.clone();
        if let Some(idempotency_key) = idempotency_key {
            self.command_idempotency_cache
                .lock()
                .expect("sqlite command idempotency cache poisoned")
                .insert(
                    (
                        association.tenant_id,
                        association.organization_id,
                        idempotency_key,
                    ),
                    command_id_for_cache,
                );
        }

        Ok(AiotCommandRecord {
            id: (next_id + 1).to_string(),
            tenant_id: association.tenant_id,
            organization_id: association.organization_id,
            command_id,
            device_id,
            session_id,
            capability_name,
            command_name,
            request_payload_json,
            request_media_resource_id,
            request_object_blob_id,
            request_media_json: request_media_snapshot.clone(),
            status,
            trace_id: trace_id.clone(),
            timeout_at,
            ack_at: None,
            result_at: None,
            created_at: created_at.clone(),
            result: None,
        })
    }

    fn list_commands(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotCommandRecord>, AiotCommandRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let limit = params.page_size.max(1);
        let offset = params.offset.max(0);
        self.db
            .run_owned(|pool| async move {
                let count_sql = pool.adapt_sql(
                    "SELECT COUNT(1) FROM iot_command WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
                );
                let list_sql = pool.adapt_sql(
                    "SELECT id, command_id, device_id, session_id, capability_name, command_name, request_payload, request_media_resource_id, request_object_blob_id, request_media_resource_snapshot, status, timeout_at, ack_at, result_at, trace_id, created_at FROM iot_command WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 ORDER BY id ASC LIMIT ?4 OFFSET ?5",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let mut commands = rows
                            .iter()
                            .map(|row| row_to_command_record(row, &association))
                            .collect::<Result<Vec<_>, _>>()?;
                        for command in &mut commands {
                            command.result = command_result_for(
                                &pool,
                                association.tenant_id,
                                association.organization_id,
                                &command.command_id,
                            )
                            .await?;
                        }
                        Ok(AiotOffsetListResult { items: commands, total })
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let mut commands = rows
                            .iter()
                            .map(|row| row_to_command_record_postgres(row, &association))
                            .collect::<Result<Vec<_>, _>>()?;
                        for command in &mut commands {
                            command.result = command_result_for(
                                &pool,
                                association.tenant_id,
                                association.organization_id,
                                &command.command_id,
                            )
                            .await?;
                        }
                        Ok(AiotOffsetListResult { items: commands, total })
                    }
                }
            })
            .map_err(map_command_sql_error)
    }

    fn cancel_command(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        command_id: &str,
    ) -> Result<Option<AiotCommandRecord>, AiotCommandRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let command_id = command_id.to_string();
        let scoped_device_id = device_id.clone();
        let scoped_command_id = command_id.clone();
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let select_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT id, status FROM iot_command WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND command_id = ?4 LIMIT 1",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            let existing = sqlx::query(&select_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&scoped_device_id)
                                .bind(&scoped_command_id)
                                .fetch_optional(&mut **connection)
                                .await?;
                            if let Some(row) = existing {
                                let id: i64 = row.try_get("id").map_err(|_| {
                                    SqliteRepoTxError::Command(
                                        AiotCommandRepositoryError::PersistenceFailure,
                                    )
                                })?;
                                let current_status_code: i64 = row.try_get("status").map_err(
                                    |_| {
                                        SqliteRepoTxError::Command(
                                            AiotCommandRepositoryError::PersistenceFailure,
                                        )
                                    },
                                )?;
                                if current_status_code != command_status_code("cancelled") {
                                    let now = default_timestamp().to_string();
                                    let update_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "UPDATE iot_command SET status = ?1, updated_at = ?2 WHERE id = ?3",
                                    );
                                    sqlx::query(&update_sql)
                                        .bind(command_status_code("cancelled"))
                                        .bind(&now)
                                        .bind(id)
                                        .execute(&mut **connection)
                                        .await?;
                                }
                            }
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            let existing = sqlx::query(&select_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&scoped_device_id)
                                .bind(&scoped_command_id)
                                .fetch_optional(&mut **connection)
                                .await?;
                            if let Some(row) = existing {
                                let id: i64 = row.try_get("id").map_err(|_| {
                                    SqliteRepoTxError::Command(
                                        AiotCommandRepositoryError::PersistenceFailure,
                                    )
                                })?;
                                let current_status_code: i64 = row.try_get("status").map_err(
                                    |_| {
                                        SqliteRepoTxError::Command(
                                            AiotCommandRepositoryError::PersistenceFailure,
                                        )
                                    },
                                )?;
                                if current_status_code != command_status_code("cancelled") {
                                    let now = default_timestamp().to_string();
                                    let update_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "UPDATE iot_command SET status = ?1, updated_at = ?2 WHERE id = ?3",
                                    );
                                    sqlx::query(&update_sql)
                                        .bind(command_status_code("cancelled"))
                                        .bind(&now)
                                        .bind(id)
                                        .execute(&mut **connection)
                                        .await?;
                                }
                            }
                        }
                    }
                    Ok(())
                })
            })
        .map_err(SqliteRepoTxError::into_command)?;

        self.get_command_by_id(&association, &device_id, &command_id)
    }
}

impl AiotCommandDeliveryRepository for SqliteSqlxDeviceRepository {
    fn enqueue_delivery(
        &self,
        command: AiotCommandDeliveryEnqueueCommand,
    ) -> Result<AiotCommandDeliveryRecord, AiotCommandDeliveryRepositoryError> {
        let created_at = default_timestamp().to_string();
        let association = command.association.clone();
        let command_id = command.command_id.clone();
        let session_id = command.session_id.clone();
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let exists_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT COUNT(1) FROM iot_command WHERE tenant_id = ?1 AND organization_id = ?2 AND command_id = ?3",
                    );
                    let exists: i64 = match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query_scalar(&exists_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&command_id)
                                .fetch_one(&mut **connection)
                                .await?
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query_scalar(&exists_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&command_id)
                                .fetch_one(&mut **connection)
                                .await?
                        }
                    };
                    if exists == 0 {
                        return Err(SqliteRepoTxError::Delivery(
                            AiotCommandDeliveryRepositoryError::CommandNotFound,
                        ));
                    }

                    let (delivery_id, _) = insert_command_delivery_row(
                        &mut tx,
                        dialect,
                        &association,
                        &command_id,
                        session_id.as_deref(),
                        &created_at,
                    )
                    .await?;

                    Ok(AiotCommandDeliveryRecord {
                        id: delivery_id.to_string(),
                        tenant_id: association.tenant_id,
                        organization_id: association.organization_id,
                        command_id,
                        session_id,
                        delivery_state: "pending".to_string(),
                        created_at,
                    })
                })
            })
            .map_err(SqliteRepoTxError::into_delivery)
    }

    fn list_pending_for_device(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        limit: i64,
    ) -> Result<Vec<AiotCommandDeliveryRecord>, AiotCommandDeliveryRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let limit = limit.max(1);
        self.db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT d.id, d.command_id, d.session_id, d.delivery_state, d.created_at
                     FROM iot_command_delivery d
                     INNER JOIN iot_command c
                       ON c.tenant_id = d.tenant_id
                      AND c.organization_id = d.organization_id
                      AND c.command_id = d.command_id
                     WHERE d.tenant_id = ?1
                       AND d.organization_id = ?2
                       AND c.device_id = ?3
                       AND d.delivery_state = 'pending'
                       AND d.status = 1
                     ORDER BY d.id ASC
                     LIMIT ?4",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let rows = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok(rows
                            .iter()
                            .filter_map(|row| {
                                row_to_command_delivery_record(row, &association).ok()
                            })
                            .collect())
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let rows = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok(rows
                            .iter()
                            .filter_map(|row| {
                                row_to_command_delivery_record_postgres(row, &association).ok()
                            })
                            .collect())
                    }
                }
            })
            .map_err(map_delivery_sql_error)
    }

    fn mark_delivered(
        &self,
        association: &AiotStorageAssociation,
        command_id: &str,
    ) -> Result<(), AiotCommandDeliveryRepositoryError> {
        let association = association.clone();
        let command_id = command_id.to_string();
        let now = default_timestamp().to_string();
        let updated = self
            .db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "UPDATE iot_command_delivery
                     SET delivery_state = 'delivered', updated_at = ?1
                     WHERE tenant_id = ?2 AND organization_id = ?3 AND command_id = ?4 AND status = 1",
                );
                let rows = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query(&sql)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&command_id)
                            .execute(pool.sqlite_pool().expect("sqlite pool"))
                            .await?
                            .rows_affected()
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query(&sql)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&command_id)
                            .execute(pool.postgres_pool().expect("postgres pool"))
                            .await?
                            .rows_affected()
                    }
                };
                Ok(rows)
            })
            .map_err(map_delivery_sql_error)?;
        if updated == 0 {
            return Err(AiotCommandDeliveryRepositoryError::CommandNotFound);
        }
        Ok(())
    }
}

impl AiotDeviceSessionRepository for SqliteSqlxDeviceRepository {
    fn list_sessions(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceSessionRecord>, AiotDeviceRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let limit = params.page_size.max(1);
        let offset = params.offset.max(0);
        self.db
            .run_owned(|pool| async move {
                let count_sql = pool.adapt_sql(
                    "SELECT COUNT(1) FROM iot_device_session WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
                );
                let list_sql = pool.adapt_sql(
                    "SELECT session_id, device_id, status, connected_at, disconnected_at, protocol_id, adapter_id FROM iot_device_session WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 ORDER BY id ASC LIMIT ?4 OFFSET ?5",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok(AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_device_session_record(row).ok())
                                .collect(),
                            total,
                        })
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok(AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_device_session_record_postgres(row).ok())
                                .collect(),
                            total,
                        })
                    }
                }
            })
            .map_err(map_device_sql_error)
    }

    fn disconnect_session(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let session_id = session_id.to_string();
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let now = default_timestamp().to_string();
                    let disconnected_status = 2_i64;

                    let select_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT status FROM iot_device_session WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND session_id = ?4 LIMIT 1",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            let existing_status = sqlx::query(&select_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&device_id)
                                .bind(&session_id)
                                .fetch_optional(&mut **connection)
                                .await?
                                .and_then(|row| row.try_get::<i64, _>("status").ok());
                            match existing_status {
                                Some(status) if status == disconnected_status => Ok(false),
                                Some(_) => {
                                    let update_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "UPDATE iot_device_session SET status = ?1, disconnected_at = ?2, updated_at = ?2 WHERE tenant_id = ?3 AND organization_id = ?4 AND device_id = ?5 AND session_id = ?6",
                                    );
                                    sqlx::query(&update_sql)
                                        .bind(disconnected_status)
                                        .bind(&now)
                                        .bind(association.tenant_id)
                                        .bind(association.organization_id)
                                        .bind(&device_id)
                                        .bind(&session_id)
                                        .execute(&mut **connection)
                                        .await?;
                                    Ok(true)
                                }
                                None => {
                                    let max_id_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "SELECT COALESCE(MAX(id), 0) FROM iot_device_session",
                                    );
                                    let next_id: i64 = sqlx::query_scalar(&max_id_sql)
                                        .fetch_one(&mut **connection)
                                        .await?;
                                    let session_uuid = format!("session-{session_id}");
                                    let connection_id = format!("connection-{session_id}");
                                    let insert_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "INSERT INTO iot_device_session (id, uuid, tenant_id, organization_id, data_scope, device_id, session_id, connection_id, protocol_id, adapter_id, node_id, status, connected_at, last_seen_at, disconnected_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'xiaozhi.websocket', 'xiaozhi', NULL, ?9, ?10, ?10, ?10, ?10, ?10, 0)",
                                    );
                                    sqlx::query(&insert_sql)
                                        .bind(next_id + 1)
                                        .bind(&session_uuid)
                                        .bind(association.tenant_id)
                                        .bind(association.organization_id)
                                        .bind(association.data_scope as i64)
                                        .bind(&device_id)
                                        .bind(&session_id)
                                        .bind(&connection_id)
                                        .bind(disconnected_status)
                                        .bind(&now)
                                        .execute(&mut **connection)
                                        .await?;
                                    Ok(true)
                                }
                            }
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            let existing_status = sqlx::query(&select_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&device_id)
                                .bind(&session_id)
                                .fetch_optional(&mut **connection)
                                .await?
                                .and_then(|row| row.try_get::<i64, _>("status").ok());
                            match existing_status {
                                Some(status) if status == disconnected_status => Ok(false),
                                Some(_) => {
                                    let update_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "UPDATE iot_device_session SET status = ?1, disconnected_at = ?2, updated_at = ?2 WHERE tenant_id = ?3 AND organization_id = ?4 AND device_id = ?5 AND session_id = ?6",
                                    );
                                    sqlx::query(&update_sql)
                                        .bind(disconnected_status)
                                        .bind(&now)
                                        .bind(association.tenant_id)
                                        .bind(association.organization_id)
                                        .bind(&device_id)
                                        .bind(&session_id)
                                        .execute(&mut **connection)
                                        .await?;
                                    Ok(true)
                                }
                                None => {
                                    let max_id_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "SELECT COALESCE(MAX(id), 0) FROM iot_device_session",
                                    );
                                    let next_id: i64 = sqlx::query_scalar(&max_id_sql)
                                        .fetch_one(&mut **connection)
                                        .await?;
                                    let session_uuid = format!("session-{session_id}");
                                    let connection_id = format!("connection-{session_id}");
                                    let insert_sql = adapt_sqlite_placeholders(
                                        dialect,
                                        "INSERT INTO iot_device_session (id, uuid, tenant_id, organization_id, data_scope, device_id, session_id, connection_id, protocol_id, adapter_id, node_id, status, connected_at, last_seen_at, disconnected_at, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'xiaozhi.websocket', 'xiaozhi', NULL, ?9, ?10, ?10, ?10, ?10, ?10, 0)",
                                    );
                                    sqlx::query(&insert_sql)
                                        .bind(next_id + 1)
                                        .bind(&session_uuid)
                                        .bind(association.tenant_id)
                                        .bind(association.organization_id)
                                        .bind(association.data_scope as i64)
                                        .bind(&device_id)
                                        .bind(&session_id)
                                        .bind(&connection_id)
                                        .bind(disconnected_status)
                                        .bind(&now)
                                        .execute(&mut **connection)
                                        .await?;
                                    Ok(true)
                                }
                            }
                        }
                    }
                })
            })
            .map_err(SqliteRepoTxError::into_device)
    }

    fn is_session_disconnected(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        session_id: &str,
    ) -> Result<bool, AiotDeviceRepositoryError> {
        let disconnected_status = 2_i64;
        let association = association.clone();
        let device_id = device_id.to_string();
        let session_id = session_id.to_string();
        let status = self
            .db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT status FROM iot_device_session WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND session_id = ?4 LIMIT 1",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&session_id)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<Option<i64>, sqlx::Error>(
                            row.and_then(|row| row.try_get::<i64, _>("status").ok()),
                        )
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&session_id)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<Option<i64>, sqlx::Error>(
                            row.and_then(|row| row.try_get::<i64, _>("status").ok()),
                        )
                    }
                }
            })
            .map_err(map_device_sql_error)?;
        Ok(status == Some(disconnected_status))
    }
}

impl AiotEventRepository for SqliteSqlxDeviceRepository {
    fn record_event(
        &self,
        command: AiotDeviceEventCreateCommand,
    ) -> Result<AiotDeviceEventRecord, AiotEventRepositoryError> {
        let media_snapshot = command.media_json.clone();
        let occurred_at = command.occurred_at.clone();
        let envelope_payload = serde_json::json!({
            "eventVersion": command.event_version,
            "protocolId": command.protocol_id,
            "adapterId": command.adapter_id,
            "messageClass": command.message_class,
            "semanticType": command.semantic_type,
            "transport": command.transport,
            "direction": command.direction,
            "messageId": command.message_id,
            "correlationId": command.correlation_id,
            "traceId": command.trace_id,
            "payloadHash": command.payload_hash,
            "occurredAt": occurred_at,
            "payload": serde_json::from_str::<JsonValue>(&command.payload_json).unwrap_or_else(|_| JsonValue::String(command.payload_json.clone()))
        });
        let event_payload_json = envelope_payload.to_string();
        let tenant_id = command.association.tenant_id;
        let organization_id = command.association.organization_id;
        let event_type = command.event_type.clone();
        let event_version = command.event_version.clone();
        let device_id = command.device_id.clone();
        let protocol_id = command.protocol_id.clone();
        let adapter_id = command.adapter_id.clone();
        let message_class = command.message_class.clone();
        let semantic_type = command.semantic_type.clone();
        let transport = command.transport.clone();
        let direction = command.direction.clone();
        let message_id = command.message_id.clone();
        let correlation_id = command.correlation_id.clone();
        let trace_id = command.trace_id.clone();
        let payload_hash = command.payload_hash.clone();
        let media_resource_id = command.media_resource_id.clone();
        let object_blob_id = command.object_blob_id.clone();
        let payload_json = command.payload_json.clone();

        let media_json = media_snapshot.clone();
        let occurred_at_value = occurred_at.clone();
        let (next_id, event_id) = self.db.with_device_transaction(|mut tx, dialect| {
            let command = command;
            let event_payload_json = event_payload_json;
            let media_snapshot = media_snapshot;
            let occurred_at = occurred_at;
            Box::pin(async move {
                let max_id_sql =
                    adapt_sqlite_placeholders(dialect, "SELECT COALESCE(MAX(id), 0) FROM iot_device_event");
                let next_id: i64 = match &mut tx {
                    DeviceDbTransaction::Sqlite(connection) => {
                        sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                    }
                    DeviceDbTransaction::Postgres(connection) => {
                        sqlx::query_scalar(&max_id_sql).fetch_one(&mut **connection).await
                    }
                }
                .map_err(|_| SqliteRepoTxError::Event(AiotEventRepositoryError::PersistenceFailure))?;

                let event_id = command
                    .event_id
                    .unwrap_or_else(|| format!("evt-{}-{:04}", command.device_id, next_id + 1));

                let insert_sql = adapt_sqlite_placeholders(
                    dialect,
                    "INSERT INTO iot_device_event (id, uuid, tenant_id, organization_id, data_scope, device_id, event_type, event_payload, media_resource_id, object_blob_id, media_resource_snapshot, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?13, 0)",
                );
                match &mut tx {
                    DeviceDbTransaction::Sqlite(connection) => {
                        sqlx::query(&insert_sql)
                            .bind(next_id + 1)
                            .bind(&event_id)
                            .bind(command.association.tenant_id)
                            .bind(command.association.organization_id)
                            .bind(command.association.data_scope as i64)
                            .bind(&command.device_id)
                            .bind(&command.event_type)
                            .bind(&event_payload_json)
                            .bind(command.media_resource_id.as_deref())
                            .bind(command.object_blob_id.as_deref())
                            .bind(media_snapshot.as_deref())
                            .bind(&occurred_at)
                            .bind(&occurred_at)
                            .execute(&mut **connection)
                            .await?;
                    }
                    DeviceDbTransaction::Postgres(connection) => {
                        sqlx::query(&insert_sql)
                            .bind(next_id + 1)
                            .bind(&event_id)
                            .bind(command.association.tenant_id)
                            .bind(command.association.organization_id)
                            .bind(command.association.data_scope as i64)
                            .bind(&command.device_id)
                            .bind(&command.event_type)
                            .bind(&event_payload_json)
                            .bind(command.media_resource_id.as_deref())
                            .bind(command.object_blob_id.as_deref())
                            .bind(media_snapshot.as_deref())
                            .bind(&occurred_at)
                            .bind(&occurred_at)
                            .execute(&mut **connection)
                            .await?;
                    }
                }

                Ok((next_id, event_id))
            })
        })
        .map_err(SqliteRepoTxError::into_event)?;

        Ok(AiotDeviceEventRecord {
            id: (next_id + 1).to_string(),
            tenant_id,
            organization_id,
            event_id,
            event_type,
            event_version,
            device_id,
            protocol_id,
            adapter_id,
            message_class,
            semantic_type,
            transport,
            direction,
            message_id,
            correlation_id,
            trace_id,
            payload_hash,
            media_resource_id,
            object_blob_id,
            media_json,
            payload_json,
            occurred_at: occurred_at_value,
        })
    }

    fn list_events(
        &self,
        association: &AiotStorageAssociation,
        device_id: Option<&str>,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<AiotDeviceEventRecord>, AiotEventRepositoryError> {
        let association = association.clone();
        let scoped_device_id = device_id.map(str::to_string);
        let limit = params.page_size.max(1);
        let offset = params.offset.max(0);
        self.db
            .run_owned(|pool| async move {
                if let Some(device_id) = scoped_device_id.as_deref() {
                    let count_sql = pool.adapt_sql(
                        "SELECT COUNT(1) FROM iot_device_event WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
                    );
                    let list_sql = pool.adapt_sql(
                        "SELECT id, uuid, device_id, event_type, event_payload, media_resource_id, object_blob_id, media_resource_snapshot, created_at FROM iot_device_event WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 ORDER BY id ASC LIMIT ?4 OFFSET ?5",
                    );
                    match pool.engine() {
                        DeviceDatabaseEngine::Sqlite => {
                            let total: i64 = sqlx::query_scalar(&count_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(device_id)
                                .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            let rows = sqlx::query(&list_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(device_id)
                                .bind(limit)
                                .bind(offset)
                                .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            Ok(AiotOffsetListResult {
                                items: rows
                                    .iter()
                                    .map(|row| row_to_device_event_record(row, &association))
                                    .collect::<Result<Vec<_>, _>>()?,
                                total,
                            })
                        }
                        DeviceDatabaseEngine::Postgres => {
                            let total: i64 = sqlx::query_scalar(&count_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(device_id)
                                .fetch_one(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            let rows = sqlx::query(&list_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(device_id)
                                .bind(limit)
                                .bind(offset)
                                .fetch_all(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            Ok(AiotOffsetListResult {
                                items: rows
                                    .iter()
                                    .map(|row| {
                                        row_to_device_event_record_postgres(row, &association)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                total,
                            })
                        }
                    }
                } else {
                    let count_sql = pool.adapt_sql(
                        "SELECT COUNT(1) FROM iot_device_event WHERE tenant_id = ?1 AND organization_id = ?2",
                    );
                    let list_sql = pool.adapt_sql(
                        "SELECT id, uuid, device_id, event_type, event_payload, media_resource_id, object_blob_id, media_resource_snapshot, created_at FROM iot_device_event WHERE tenant_id = ?1 AND organization_id = ?2 ORDER BY id ASC LIMIT ?3 OFFSET ?4",
                    );
                    match pool.engine() {
                        DeviceDatabaseEngine::Sqlite => {
                            let total: i64 = sqlx::query_scalar(&count_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            let rows = sqlx::query(&list_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(limit)
                                .bind(offset)
                                .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                                .await?;
                            Ok(AiotOffsetListResult {
                                items: rows
                                    .iter()
                                    .map(|row| row_to_device_event_record(row, &association))
                                    .collect::<Result<Vec<_>, _>>()?,
                                total,
                            })
                        }
                        DeviceDatabaseEngine::Postgres => {
                            let total: i64 = sqlx::query_scalar(&count_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .fetch_one(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            let rows = sqlx::query(&list_sql)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(limit)
                                .bind(offset)
                                .fetch_all(pool.postgres_pool().expect("postgres pool"))
                                .await?;
                            Ok(AiotOffsetListResult {
                                items: rows
                                    .iter()
                                    .map(|row| {
                                        row_to_device_event_record_postgres(row, &association)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                total,
                            })
                        }
                    }
                }
            })
            .map_err(map_event_sql_error)
    }
}

impl AiotDeviceTwinRepository for SqliteSqlxDeviceRepository {
    fn upsert_twin_property(
        &self,
        command: AiotTwinPropertyUpsertCommand,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let updated_at = command
            .desired_updated_at
            .clone()
            .or(command.reported_updated_at.clone())
            .unwrap_or_else(|| default_timestamp().to_string());
        let desired_updated_at = command
            .desired_updated_at
            .clone()
            .unwrap_or_else(|| default_timestamp().to_string());
        let reported_updated_at = command
            .reported_updated_at
            .clone()
            .unwrap_or_else(|| default_timestamp().to_string());
        let association = command.association.clone();
        let device_id = command.device_id.clone();

        self.db
            .with_device_transaction(|mut tx, dialect| {
                let command = command;
                let updated_at = updated_at;
                let desired_updated_at = desired_updated_at;
                let reported_updated_at = reported_updated_at;
                Box::pin(async move {
                    ensure_twin_root_row(
                        &mut tx,
                        dialect,
                        &command.association,
                        &command.device_id,
                        &updated_at,
                    )
                    .await
                    .map_err(|_| SqliteRepoTxError::Twin(AiotDeviceTwinRepositoryError::PersistenceFailure))?;

                    let select_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT id, desired_value, desired_version, reported_value, reported_version FROM iot_device_twin_property WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND property_name = ?4 LIMIT 1",
                    );
                    let property_state: Option<TwinPropertyState> = match &mut tx {
                            DeviceDbTransaction::Sqlite(connection) => {
                                let row = sqlx::query(&select_sql)
                                    .bind(command.association.tenant_id)
                                    .bind(command.association.organization_id)
                                    .bind(&command.device_id)
                                    .bind(&command.property_name)
                                    .fetch_optional(&mut **connection)
                                    .await
                                    .map_err(|_| {
                                        SqliteRepoTxError::Twin(
                                            AiotDeviceTwinRepositoryError::PersistenceFailure,
                                        )
                                    })?;
                                match row {
                                    None =>                                 Ok::<Option<TwinPropertyState>, SqliteRepoTxError>(None),
                                    Some(row) => Ok(Some((
                                        row.try_get::<i64, _>("id").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                        row.try_get::<Option<String>, _>("desired_value")
                                            .map_err(|_| {
                                                SqliteRepoTxError::Twin(
                                                    AiotDeviceTwinRepositoryError::PersistenceFailure,
                                                )
                                            })?,
                                        row.try_get::<i64, _>("desired_version").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                        row.try_get::<Option<String>, _>("reported_value")
                                            .map_err(|_| {
                                                SqliteRepoTxError::Twin(
                                                    AiotDeviceTwinRepositoryError::PersistenceFailure,
                                                )
                                            })?,
                                        row.try_get::<i64, _>("reported_version").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                    ))),
                                }
                            }
                            DeviceDbTransaction::Postgres(connection) => {
                                let row = sqlx::query(&select_sql)
                                    .bind(command.association.tenant_id)
                                    .bind(command.association.organization_id)
                                    .bind(&command.device_id)
                                    .bind(&command.property_name)
                                    .fetch_optional(&mut **connection)
                                    .await
                                    .map_err(|_| {
                                        SqliteRepoTxError::Twin(
                                            AiotDeviceTwinRepositoryError::PersistenceFailure,
                                        )
                                    })?;
                                match row {
                                    None =>                                 Ok::<Option<TwinPropertyState>, SqliteRepoTxError>(None),
                                    Some(row) => Ok(Some((
                                        row.try_get::<i64, _>("id").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                        row.try_get::<Option<String>, _>("desired_value")
                                            .map_err(|_| {
                                                SqliteRepoTxError::Twin(
                                                    AiotDeviceTwinRepositoryError::PersistenceFailure,
                                                )
                                            })?,
                                        row.try_get::<i64, _>("desired_version").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                        row.try_get::<Option<String>, _>("reported_value")
                                            .map_err(|_| {
                                                SqliteRepoTxError::Twin(
                                                    AiotDeviceTwinRepositoryError::PersistenceFailure,
                                                )
                                            })?,
                                        row.try_get::<i64, _>("reported_version").map_err(|_| {
                                            SqliteRepoTxError::Twin(
                                                AiotDeviceTwinRepositoryError::PersistenceFailure,
                                            )
                                        })?,
                                    ))),
                                }
                            }
                        }?;

                    if let Some((
                        id,
                        existing_desired,
                        existing_desired_version,
                        existing_reported,
                        existing_reported_version,
                    )) = property_state
                    {

                        let desired_value = command.desired_value_json.clone().or(existing_desired);
                        let reported_value = command.reported_value_json.clone().or(existing_reported);
                        let desired_version = if command.desired_value_json.is_some() {
                            existing_desired_version.saturating_add(1)
                        } else {
                            existing_desired_version
                        };
                        let reported_version = if command.reported_value_json.is_some() {
                            existing_reported_version.saturating_add(1)
                        } else {
                            existing_reported_version
                        };
                        let update_sql = adapt_sqlite_placeholders(
                            dialect,
                            "UPDATE iot_device_twin_property SET desired_value = ?1, desired_version = ?2, desired_updated_at = ?3, reported_value = ?4, reported_version = ?5, reported_updated_at = ?6, updated_at = ?7 WHERE id = ?8",
                        );
                        match &mut tx {
                            DeviceDbTransaction::Sqlite(connection) => {
                                sqlx::query(&update_sql)
                                    .bind(desired_value.as_deref())
                                    .bind(desired_version)
                                    .bind(&desired_updated_at)
                                    .bind(reported_value.as_deref())
                                    .bind(reported_version)
                                    .bind(&reported_updated_at)
                                    .bind(&updated_at)
                                    .bind(id)
                                    .execute(&mut **connection)
                                    .await?;
                            }
                            DeviceDbTransaction::Postgres(connection) => {
                                sqlx::query(&update_sql)
                                    .bind(desired_value.as_deref())
                                    .bind(desired_version)
                                    .bind(&desired_updated_at)
                                    .bind(reported_value.as_deref())
                                    .bind(reported_version)
                                    .bind(&reported_updated_at)
                                    .bind(&updated_at)
                                    .bind(id)
                                    .execute(&mut **connection)
                                    .await?;
                            }
                        }
                    } else {
                        let max_id_sql = adapt_sqlite_placeholders(
                            dialect,
                            "SELECT COALESCE(MAX(id), 0) FROM iot_device_twin_property",
                        );
                        let next_property_id: i64 = match &mut tx {
                            DeviceDbTransaction::Sqlite(connection) => {
                                sqlx::query_scalar(&max_id_sql)
                                    .fetch_one(&mut **connection)
                                    .await
                            }
                            DeviceDbTransaction::Postgres(connection) => {
                                sqlx::query_scalar(&max_id_sql)
                                    .fetch_one(&mut **connection)
                                    .await
                            }
                        }
                        .map_err(|_| SqliteRepoTxError::Twin(AiotDeviceTwinRepositoryError::PersistenceFailure))?;
                        let insert_sql = adapt_sqlite_placeholders(
                            dialect,
                            "INSERT INTO iot_device_twin_property (id, uuid, tenant_id, organization_id, data_scope, device_id, property_name, desired_value, desired_version, desired_updated_at, reported_value, reported_version, reported_updated_at, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1, ?14, ?15, 0)",
                        );
                        match &mut tx {
                            DeviceDbTransaction::Sqlite(connection) => {
                                sqlx::query(&insert_sql)
                                    .bind(next_property_id + 1)
                                    .bind(format!(
                                        "twin-prop-{}-{}",
                                        command.device_id, command.property_name
                                    ))
                                    .bind(command.association.tenant_id)
                                    .bind(command.association.organization_id)
                                    .bind(command.association.data_scope as i64)
                                    .bind(&command.device_id)
                                    .bind(&command.property_name)
                                    .bind(command.desired_value_json.as_deref())
                                    .bind(if command.desired_value_json.is_some() {
                                        1
                                    } else {
                                        0
                                    })
                                    .bind(if command.desired_value_json.is_some() {
                                        Some(desired_updated_at.as_str())
                                    } else {
                                        None::<&str>
                                    })
                                    .bind(command.reported_value_json.as_deref())
                                    .bind(if command.reported_value_json.is_some() {
                                        1
                                    } else {
                                        0
                                    })
                                    .bind(if command.reported_value_json.is_some() {
                                        Some(reported_updated_at.as_str())
                                    } else {
                                        None::<&str>
                                    })
                                    .bind(&updated_at)
                                    .bind(&updated_at)
                                    .execute(&mut **connection)
                                    .await?;
                            }
                            DeviceDbTransaction::Postgres(connection) => {
                                sqlx::query(&insert_sql)
                                    .bind(next_property_id + 1)
                                    .bind(format!(
                                        "twin-prop-{}-{}",
                                        command.device_id, command.property_name
                                    ))
                                    .bind(command.association.tenant_id)
                                    .bind(command.association.organization_id)
                                    .bind(command.association.data_scope as i64)
                                    .bind(&command.device_id)
                                    .bind(&command.property_name)
                                    .bind(command.desired_value_json.as_deref())
                                    .bind(if command.desired_value_json.is_some() {
                                        1
                                    } else {
                                        0
                                    })
                                    .bind(if command.desired_value_json.is_some() {
                                        Some(desired_updated_at.as_str())
                                    } else {
                                        None::<&str>
                                    })
                                    .bind(command.reported_value_json.as_deref())
                                    .bind(if command.reported_value_json.is_some() {
                                        1
                                    } else {
                                        0
                                    })
                                    .bind(if command.reported_value_json.is_some() {
                                        Some(reported_updated_at.as_str())
                                    } else {
                                        None::<&str>
                                    })
                                    .bind(&updated_at)
                                    .bind(&updated_at)
                                    .execute(&mut **connection)
                                    .await?;
                            }
                        }
                    }

                    recompute_twin_versions(
                        &mut tx,
                        dialect,
                        &command.association,
                        &command.device_id,
                        &updated_at,
                    )
                    .await
                    .map_err(|_| SqliteRepoTxError::Twin(AiotDeviceTwinRepositoryError::PersistenceFailure))?;
                    Ok(())
                })
            })
            .map_err(SqliteRepoTxError::into_twin)?;

        self.get_twin_snapshot(&association, &device_id)
    }

    fn get_twin_snapshot(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
    ) -> Result<AiotDeviceTwinSnapshot, AiotDeviceTwinRepositoryError> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let association_for_properties = association.clone();
        let device_id_for_properties = device_id.clone();

        let (desired, reported) = self
            .db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT property_name, desired_value, reported_value FROM iot_device_twin_property WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 ORDER BY id ASC",
                );
                let mut desired = BTreeMap::new();
                let mut reported = BTreeMap::new();
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let rows = sqlx::query(&sql)
                            .bind(association_for_properties.tenant_id)
                            .bind(association_for_properties.organization_id)
                            .bind(&device_id_for_properties)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        for row in rows {
                            let property_name: String = row.try_get("property_name")?;
                            let desired_value: Option<String> = row.try_get("desired_value")?;
                            let reported_value: Option<String> = row.try_get("reported_value")?;
                            if let Some(desired_value) = desired_value {
                                desired.insert(property_name.clone(), desired_value);
                            }
                            if let Some(reported_value) = reported_value {
                                reported.insert(property_name, reported_value);
                            }
                        }
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let rows = sqlx::query(&sql)
                            .bind(association_for_properties.tenant_id)
                            .bind(association_for_properties.organization_id)
                            .bind(&device_id_for_properties)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        for row in rows {
                            let property_name: String = row.try_get("property_name")?;
                            let desired_value: Option<String> = row.try_get("desired_value")?;
                            let reported_value: Option<String> = row.try_get("reported_value")?;
                            if let Some(desired_value) = desired_value {
                                desired.insert(property_name.clone(), desired_value);
                            }
                            if let Some(reported_value) = reported_value {
                                reported.insert(property_name, reported_value);
                            }
                        }
                    }
                }
                Ok::<(BTreeMap<String, String>, BTreeMap<String, String>), sqlx::Error>((
                    desired, reported,
                ))
            })
            .map_err(|_| AiotDeviceTwinRepositoryError::PersistenceFailure)?;

        let association_for_twin = association.clone();
        let device_id_for_twin = device_id.clone();
        let (desired_version, reported_version, updated_at) = self
            .db
            .run_owned(|pool| async move {
                let sql = pool.adapt_sql(
                    "SELECT desired_version, reported_version, updated_at FROM iot_device_twin WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 LIMIT 1",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&sql)
                            .bind(association_for_twin.tenant_id)
                            .bind(association_for_twin.organization_id)
                            .bind(&device_id_for_twin)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<Option<(i64, i64, Option<String>)>, sqlx::Error>(row.and_then(|row| {
                            let desired_version: i64 = row.try_get("desired_version").ok()?;
                            let reported_version: i64 = row.try_get("reported_version").ok()?;
                            let updated_at: Option<String> = row.try_get("updated_at").ok()?;
                            Some((desired_version, reported_version, updated_at))
                        }))
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&sql)
                            .bind(association_for_twin.tenant_id)
                            .bind(association_for_twin.organization_id)
                            .bind(&device_id_for_twin)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<Option<(i64, i64, Option<String>)>, sqlx::Error>(row.and_then(|row| {
                            let desired_version: i64 = row.try_get("desired_version").ok()?;
                            let reported_version: i64 = row.try_get("reported_version").ok()?;
                            let updated_at: Option<String> = row.try_get("updated_at").ok()?;
                            Some((desired_version, reported_version, updated_at))
                        }))
                    }
                }
            })
            .map_err(|_| AiotDeviceTwinRepositoryError::PersistenceFailure)?
            .unwrap_or((0, 0, Some(default_timestamp().to_string())));

        Ok(AiotDeviceTwinSnapshot {
            tenant_id: association.tenant_id,
            organization_id: association.organization_id,
            device_id,
            desired,
            reported,
            desired_version,
            reported_version,
            updated_at: updated_at.unwrap_or_else(|| default_timestamp().to_string()),
        })
    }
}

fn row_to_device_record(row: &sqlx::sqlite::SqliteRow) -> Result<AiotDeviceRecord, sqlx::Error> {
    let id: i64 = row.try_get("id")?;
    let tenant_id: i64 = row.try_get("tenant_id")?;
    let organization_id: i64 = row.try_get("organization_id")?;
    let device_id: String = row.try_get("device_id")?;
    let display_name: String = row.try_get("display_name")?;
    let product_id: i64 = row.try_get("product_id")?;
    let client_id: Option<String> = row.try_get("client_id")?;
    let chip_family: Option<String> = row.try_get("chip_family")?;
    let status: i64 = row.try_get("status")?;
    let metadata_json: Option<String> = row.try_get("metadata")?;
    let last_seen_at: Option<String> = row.try_get("last_seen_at")?;
    Ok(AiotDeviceRecord {
        id: id.to_string(),
        tenant_id,
        organization_id,
        device_id,
        display_name,
        product_id: product_id.to_string(),
        client_id,
        chip_family,
        status: device_status_text(status),
        metadata_json,
        last_seen_at: last_seen_at.unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string()),
    })
}

fn row_to_device_record_postgres(
    row: &sqlx::postgres::PgRow,
) -> Result<AiotDeviceRecord, sqlx::Error> {
    let id: i64 = row.try_get("id")?;
    let tenant_id: i64 = row.try_get("tenant_id")?;
    let organization_id: i64 = row.try_get("organization_id")?;
    let device_id: String = row.try_get("device_id")?;
    let display_name: String = row.try_get("display_name")?;
    let product_id: i64 = row.try_get("product_id")?;
    let client_id: Option<String> = row.try_get("client_id")?;
    let chip_family: Option<String> = row.try_get("chip_family")?;
    let status: i64 = row.try_get("status")?;
    let metadata_json: Option<String> = row.try_get("metadata")?;
    let last_seen_at: Option<String> = row.try_get("last_seen_at")?;
    Ok(AiotDeviceRecord {
        id: id.to_string(),
        tenant_id,
        organization_id,
        device_id,
        display_name,
        product_id: product_id.to_string(),
        client_id,
        chip_family,
        status: device_status_text(status),
        metadata_json,
        last_seen_at: last_seen_at.unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string()),
    })
}

fn row_to_command_record(
    row: &sqlx::sqlite::SqliteRow,
    association: &AiotStorageAssociation,
) -> Result<AiotCommandRecord, sqlx::Error> {
    let id: i64 = row.try_get("id")?;
    let command_id: String = row.try_get("command_id")?;
    let device_id: String = row.try_get("device_id")?;
    let session_id: Option<String> = row.try_get("session_id")?;
    let capability_name: String = row.try_get("capability_name")?;
    let command_name: String = row.try_get("command_name")?;
    let request_payload_json: String = row.try_get("request_payload")?;
    let request_media_resource_id: Option<String> = row.try_get("request_media_resource_id")?;
    let request_object_blob_id: Option<String> = row.try_get("request_object_blob_id")?;
    let request_media_json: Option<String> = row.try_get("request_media_resource_snapshot")?;
    let status_code: i64 = row.try_get("status")?;
    let timeout_at: Option<String> = row.try_get("timeout_at")?;
    let ack_at: Option<String> = row.try_get("ack_at")?;
    let result_at: Option<String> = row.try_get("result_at")?;
    let trace_id: Option<String> = row.try_get("trace_id")?;
    let created_at: Option<String> = row.try_get("created_at")?;
    Ok(AiotCommandRecord {
        id: id.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        command_id,
        device_id,
        session_id,
        capability_name,
        command_name,
        request_payload_json,
        request_media_resource_id,
        request_object_blob_id,
        request_media_json,
        status: command_status_text(status_code),
        trace_id,
        timeout_at,
        ack_at,
        result_at,
        created_at: created_at.unwrap_or_else(|| default_timestamp().to_string()),
        result: None,
    })
}

fn row_to_command_record_postgres(
    row: &sqlx::postgres::PgRow,
    association: &AiotStorageAssociation,
) -> Result<AiotCommandRecord, sqlx::Error> {
    let id: i64 = row.try_get("id")?;
    let command_id: String = row.try_get("command_id")?;
    let device_id: String = row.try_get("device_id")?;
    let session_id: Option<String> = row.try_get("session_id")?;
    let capability_name: String = row.try_get("capability_name")?;
    let command_name: String = row.try_get("command_name")?;
    let request_payload_json: String = row.try_get("request_payload")?;
    let request_media_resource_id: Option<String> = row.try_get("request_media_resource_id")?;
    let request_object_blob_id: Option<String> = row.try_get("request_object_blob_id")?;
    let request_media_json: Option<String> = row.try_get("request_media_resource_snapshot")?;
    let status_code: i64 = row.try_get("status")?;
    let timeout_at: Option<String> = row.try_get("timeout_at")?;
    let ack_at: Option<String> = row.try_get("ack_at")?;
    let result_at: Option<String> = row.try_get("result_at")?;
    let trace_id: Option<String> = row.try_get("trace_id")?;
    let created_at: Option<String> = row.try_get("created_at")?;
    Ok(AiotCommandRecord {
        id: id.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        command_id,
        device_id,
        session_id,
        capability_name,
        command_name,
        request_payload_json,
        request_media_resource_id,
        request_object_blob_id,
        request_media_json,
        status: command_status_text(status_code),
        trace_id,
        timeout_at,
        ack_at,
        result_at,
        created_at: created_at.unwrap_or_else(|| default_timestamp().to_string()),
        result: None,
    })
}

async fn command_result_for(
    pool: &BlockingDevicePool,
    tenant_id: i64,
    organization_id: i64,
    command_id: &str,
) -> Result<Option<AiotCommandResultRecord>, sqlx::Error> {
    let sql = pool.adapt_sql(
        "SELECT result_code, result_payload, result_media_resource_id, result_object_blob_id, result_media_resource_snapshot, updated_at FROM iot_command_result WHERE tenant_id = ?1 AND organization_id = ?2 AND command_id = ?3 ORDER BY id DESC LIMIT 1",
    );
    match pool.engine() {
        DeviceDatabaseEngine::Sqlite => {
            let row = sqlx::query(&sql)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(command_id)
                .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                .await?;
            Ok(row.map(|row| AiotCommandResultRecord {
                result_code: row.try_get("result_code").ok(),
                result_payload_json: row.try_get("result_payload").ok(),
                result_media_resource_id: row.try_get("result_media_resource_id").ok(),
                result_object_blob_id: row.try_get("result_object_blob_id").ok(),
                result_media_json: row.try_get("result_media_resource_snapshot").ok(),
                occurred_at: row.try_get("updated_at").ok(),
            }))
        }
        DeviceDatabaseEngine::Postgres => {
            let row = sqlx::query(&sql)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(command_id)
                .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                .await?;
            Ok(row.map(|row| AiotCommandResultRecord {
                result_code: row.try_get("result_code").ok(),
                result_payload_json: row.try_get("result_payload").ok(),
                result_media_resource_id: row.try_get("result_media_resource_id").ok(),
                result_object_blob_id: row.try_get("result_object_blob_id").ok(),
                result_media_json: row.try_get("result_media_resource_snapshot").ok(),
                occurred_at: row.try_get("updated_at").ok(),
            }))
        }
    }
}

fn row_to_device_event_record(
    row: &sqlx::sqlite::SqliteRow,
    association: &AiotStorageAssociation,
) -> Result<AiotDeviceEventRecord, sqlx::Error> {
    device_event_record_from_parts(
        row.try_get("id")?,
        row.try_get("uuid")?,
        row.try_get("device_id")?,
        row.try_get("event_type")?,
        row.try_get("event_payload")?,
        row.try_get("media_resource_id")?,
        row.try_get("object_blob_id")?,
        row.try_get("media_resource_snapshot")?,
        row.try_get("created_at")?,
        association,
    )
}

fn row_to_device_event_record_postgres(
    row: &sqlx::postgres::PgRow,
    association: &AiotStorageAssociation,
) -> Result<AiotDeviceEventRecord, sqlx::Error> {
    device_event_record_from_parts(
        row.try_get("id")?,
        row.try_get("uuid")?,
        row.try_get("device_id")?,
        row.try_get("event_type")?,
        row.try_get("event_payload")?,
        row.try_get("media_resource_id")?,
        row.try_get("object_blob_id")?,
        row.try_get("media_resource_snapshot")?,
        row.try_get("created_at")?,
        association,
    )
}

#[allow(clippy::too_many_arguments)]
fn device_event_record_from_parts(
    id: i64,
    event_id: String,
    device_id: String,
    event_type: String,
    event_payload_json: String,
    media_resource_id: Option<String>,
    object_blob_id: Option<String>,
    media_json: Option<String>,
    created_at: Option<String>,
    association: &AiotStorageAssociation,
) -> Result<AiotDeviceEventRecord, sqlx::Error> {
    let parsed_payload = serde_json::from_str::<JsonValue>(&event_payload_json).ok();

    let envelope = parsed_payload.as_ref().and_then(JsonValue::as_object);
    let payload_json = envelope
        .and_then(|payload| payload.get("payload"))
        .map(JsonValue::to_string)
        .unwrap_or(event_payload_json);

    let event_version = envelope
        .and_then(|payload| payload.get("eventVersion"))
        .and_then(JsonValue::as_str)
        .unwrap_or("1")
        .to_string();
    let protocol_id = envelope
        .and_then(|payload| payload.get("protocolId"))
        .and_then(JsonValue::as_str)
        .unwrap_or("xiaozhi.websocket")
        .to_string();
    let adapter_id = envelope
        .and_then(|payload| payload.get("adapterId"))
        .and_then(JsonValue::as_str)
        .unwrap_or("xiaozhi")
        .to_string();
    let message_class = envelope
        .and_then(|payload| payload.get("messageClass"))
        .and_then(JsonValue::as_str)
        .unwrap_or("mediaFrame")
        .to_string();
    let semantic_type = envelope
        .and_then(|payload| payload.get("semanticType"))
        .and_then(JsonValue::as_str)
        .unwrap_or("audio")
        .to_string();
    let transport = envelope
        .and_then(|payload| payload.get("transport"))
        .and_then(JsonValue::as_str)
        .unwrap_or("websocket")
        .to_string();
    let direction = envelope
        .and_then(|payload| payload.get("direction"))
        .and_then(JsonValue::as_str)
        .unwrap_or("device_to_cloud")
        .to_string();
    let message_id = envelope
        .and_then(|payload| payload.get("messageId"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let correlation_id = envelope
        .and_then(|payload| payload.get("correlationId"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let trace_id = envelope
        .and_then(|payload| payload.get("traceId"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let payload_hash = envelope
        .and_then(|payload| payload.get("payloadHash"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);
    let occurred_at = envelope
        .and_then(|payload| payload.get("occurredAt"))
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .or(created_at)
        .unwrap_or_else(|| default_timestamp().to_string());

    Ok(AiotDeviceEventRecord {
        id: id.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        event_id,
        event_type,
        event_version,
        device_id,
        protocol_id,
        adapter_id,
        message_class,
        semantic_type,
        transport,
        direction,
        message_id,
        correlation_id,
        trace_id,
        payload_hash,
        media_resource_id,
        object_blob_id,
        media_json,
        payload_json,
        occurred_at,
    })
}

async fn ensure_twin_root_row(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    association: &AiotStorageAssociation,
    device_id: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    let exists_sql = adapt_sqlite_placeholders(
        dialect,
        "SELECT COUNT(1) FROM iot_device_twin WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
    );
    let existing: i64 = match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query_scalar(&exists_sql)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .fetch_one(&mut **connection)
                .await?
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query_scalar(&exists_sql)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .fetch_one(&mut **connection)
                .await?
        }
    };
    if existing > 0 {
        return Ok(());
    }

    let max_id_sql =
        adapt_sqlite_placeholders(dialect, "SELECT COALESCE(MAX(id), 0) FROM iot_device_twin");
    let next_twin_id: i64 = match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query_scalar(&max_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query_scalar(&max_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
    };
    let insert_sql = adapt_sqlite_placeholders(
        dialect,
        "INSERT INTO iot_device_twin (id, uuid, tenant_id, organization_id, data_scope, device_id, desired_version, reported_version, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 1, ?7, ?8, 0)",
    );
    match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query(&insert_sql)
                .bind(next_twin_id + 1)
                .bind(format!("twin-{device_id}"))
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(device_id)
                .bind(updated_at)
                .bind(updated_at)
                .execute(&mut **connection)
                .await?;
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query(&insert_sql)
                .bind(next_twin_id + 1)
                .bind(format!("twin-{device_id}"))
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(device_id)
                .bind(updated_at)
                .bind(updated_at)
                .execute(&mut **connection)
                .await?;
        }
    }
    Ok(())
}

async fn recompute_twin_versions(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    association: &AiotStorageAssociation,
    device_id: &str,
    updated_at: &str,
) -> Result<(), sqlx::Error> {
    let select_sql = adapt_sqlite_placeholders(
        dialect,
        "SELECT COALESCE(MAX(desired_version), 0) AS desired_version, COALESCE(MAX(reported_version), 0) AS reported_version FROM iot_device_twin_property WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
    );
    let (desired_version, reported_version) = match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            let row = sqlx::query(&select_sql)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .fetch_one(&mut **connection)
                .await?;
            (
                row.try_get::<i64, _>("desired_version")?,
                row.try_get::<i64, _>("reported_version")?,
            )
        }
        DeviceDbTransaction::Postgres(connection) => {
            let row = sqlx::query(&select_sql)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .fetch_one(&mut **connection)
                .await?;
            (
                row.try_get::<i64, _>("desired_version")?,
                row.try_get::<i64, _>("reported_version")?,
            )
        }
    };

    let update_sql = adapt_sqlite_placeholders(
        dialect,
        "UPDATE iot_device_twin SET desired_version = ?1, reported_version = ?2, updated_at = ?3 WHERE tenant_id = ?4 AND organization_id = ?5 AND device_id = ?6",
    );
    match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query(&update_sql)
                .bind(desired_version)
                .bind(reported_version)
                .bind(updated_at)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .execute(&mut **connection)
                .await?;
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query(&update_sql)
                .bind(desired_version)
                .bind(reported_version)
                .bind(updated_at)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(device_id)
                .execute(&mut **connection)
                .await?;
        }
    }
    Ok(())
}

async fn insert_command_delivery_row(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    association: &AiotStorageAssociation,
    command_id: &str,
    session_id: Option<&str>,
    created_at: &str,
) -> Result<(i64, String), sqlx::Error> {
    let max_id_sql = adapt_sqlite_placeholders(
        dialect,
        "SELECT COALESCE(MAX(id), 0) FROM iot_command_delivery",
    );
    let next_id: i64 = match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query_scalar(&max_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query_scalar(&max_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
    };
    let delivery_id = next_id + 1;
    let delivery_uuid = format!("cmd-delivery-{delivery_id}");
    let insert_sql = adapt_sqlite_placeholders(
        dialect,
        "INSERT INTO iot_command_delivery (id, uuid, tenant_id, organization_id, data_scope, command_id, session_id, delivery_state, status, created_at, updated_at, version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 1, ?8, ?8, 0)",
    );
    match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query(&insert_sql)
                .bind(delivery_id)
                .bind(&delivery_uuid)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(command_id)
                .bind(session_id)
                .bind(created_at)
                .execute(&mut **connection)
                .await?;
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query(&insert_sql)
                .bind(delivery_id)
                .bind(&delivery_uuid)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(command_id)
                .bind(session_id)
                .bind(created_at)
                .execute(&mut **connection)
                .await?;
        }
    }
    Ok((delivery_id, delivery_uuid))
}

#[allow(clippy::too_many_arguments)]
async fn insert_command_delivery_and_outbox(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    association: &AiotStorageAssociation,
    command_id: &str,
    session_id: Option<&str>,
    device_id: &str,
    capability_name: &str,
    command_name: &str,
    trace_id: Option<&str>,
    created_at: &str,
) -> Result<(), sqlx::Error> {
    insert_command_delivery_row(tx, dialect, association, command_id, session_id, created_at)
        .await?;

    let max_outbox_id_sql =
        adapt_sqlite_placeholders(dialect, "SELECT COALESCE(MAX(id), 0) FROM iot_outbox_event");
    let next_outbox_id: i64 = match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query_scalar(&max_outbox_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query_scalar(&max_outbox_id_sql)
                .fetch_one(&mut **connection)
                .await?
        }
    };
    let outbox_id = next_outbox_id + 1;
    let event_type = "iot.command.dispatchRequested";
    let aggregate_type = "device_command";
    let event_id = format!("{aggregate_type}:{command_id}:{event_type}");
    let payload_json = serde_json::json!({
        "eventVersion": "1",
        "commandId": command_id,
        "deviceId": device_id,
        "sessionId": session_id,
        "capabilityName": capability_name,
        "commandName": command_name,
        "traceId": trace_id,
    })
    .to_string();
    let outbox_uuid = uuid();
    let insert_outbox_sql = adapt_sqlite_placeholders(
        dialect,
        "INSERT INTO iot_outbox_event (id, uuid, tenant_id, organization_id, data_scope, event_id, event_type, event_version, aggregate_type, aggregate_id, payload, payload_hash, status, trace_id, attempt_count, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '1', ?8, ?9, ?10, NULL, ?11, ?12, 0, ?13)",
    );
    match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query(&insert_outbox_sql)
                .bind(outbox_id)
                .bind(&outbox_uuid)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(&event_id)
                .bind(event_type)
                .bind(aggregate_type)
                .bind(command_id)
                .bind(&payload_json)
                .bind(OUTBOX_STATUS_PENDING)
                .bind(trace_id)
                .bind(created_at)
                .execute(&mut **connection)
                .await?;
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query(&insert_outbox_sql)
                .bind(outbox_id)
                .bind(&outbox_uuid)
                .bind(association.tenant_id)
                .bind(association.organization_id)
                .bind(association.data_scope as i64)
                .bind(&event_id)
                .bind(event_type)
                .bind(aggregate_type)
                .bind(command_id)
                .bind(&payload_json)
                .bind(OUTBOX_STATUS_PENDING)
                .bind(trace_id)
                .bind(created_at)
                .execute(&mut **connection)
                .await?;
        }
    }
    Ok(())
}

fn session_status_text(status: i64) -> String {
    match status {
        1 => "connected".to_string(),
        2 => "disconnected".to_string(),
        _ => "unknown".to_string(),
    }
}

fn transport_from_protocol_id(protocol_id: &str) -> String {
    if protocol_id.contains("websocket") {
        "websocket".to_string()
    } else if protocol_id.contains("mqtt") {
        "mqtt".to_string()
    } else if let Some((_prefix, transport)) = protocol_id.rsplit_once('.') {
        transport.to_string()
    } else {
        protocol_id.to_string()
    }
}

fn row_to_device_session_record(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<AiotDeviceSessionRecord, sqlx::Error> {
    let protocol_id: String = row.try_get("protocol_id")?;
    Ok(AiotDeviceSessionRecord {
        session_id: row.try_get("session_id")?,
        device_id: row.try_get("device_id")?,
        status: session_status_text(row.try_get::<i64, _>("status")?),
        connected_at: row.try_get("connected_at").ok(),
        disconnected_at: row.try_get("disconnected_at").ok(),
        transport: transport_from_protocol_id(&protocol_id),
        protocol_id,
        adapter_id: row.try_get("adapter_id")?,
    })
}

fn row_to_device_session_record_postgres(
    row: &sqlx::postgres::PgRow,
) -> Result<AiotDeviceSessionRecord, sqlx::Error> {
    let protocol_id: String = row.try_get("protocol_id")?;
    Ok(AiotDeviceSessionRecord {
        session_id: row.try_get("session_id")?,
        device_id: row.try_get("device_id")?,
        status: session_status_text(row.try_get::<i64, _>("status")?),
        connected_at: row.try_get("connected_at").ok(),
        disconnected_at: row.try_get("disconnected_at").ok(),
        transport: transport_from_protocol_id(&protocol_id),
        protocol_id,
        adapter_id: row.try_get("adapter_id")?,
    })
}

fn row_to_command_delivery_record(
    row: &sqlx::sqlite::SqliteRow,
    association: &AiotStorageAssociation,
) -> Result<AiotCommandDeliveryRecord, sqlx::Error> {
    Ok(AiotCommandDeliveryRecord {
        id: row.try_get::<i64, _>("id")?.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        command_id: row.try_get("command_id")?,
        session_id: row.try_get("session_id").ok(),
        delivery_state: row.try_get("delivery_state")?,
        created_at: row
            .try_get::<Option<String>, _>("created_at")?
            .unwrap_or_else(|| default_timestamp().to_string()),
    })
}

fn row_to_command_delivery_record_postgres(
    row: &sqlx::postgres::PgRow,
    association: &AiotStorageAssociation,
) -> Result<AiotCommandDeliveryRecord, sqlx::Error> {
    Ok(AiotCommandDeliveryRecord {
        id: row.try_get::<i64, _>("id")?.to_string(),
        tenant_id: association.tenant_id,
        organization_id: association.organization_id,
        command_id: row.try_get("command_id")?,
        session_id: row.try_get("session_id").ok(),
        delivery_state: row.try_get("delivery_state")?,
        created_at: row
            .try_get::<Option<String>, _>("created_at")?
            .unwrap_or_else(|| default_timestamp().to_string()),
    })
}
