use std::time::{SystemTime, UNIX_EPOCH};

use sdkwork_aiot_storage::{AiotOffsetListResult, AiotStorageAssociation, OffsetListPageParams};
use sqlx::Row;

use crate::blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine, DeviceDbTransaction};
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::schema::ensure_device_schema;
use crate::sqlite_sync::{sqlite_connect_url, BlockingSqlitePool};

const ENTITY_STATUS_ACTIVE: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlitePersistedEntityRecord {
    pub entity_kind: String,
    pub entity_key: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlitePersistedEntityError {
    PersistenceFailure,
    NotFound,
    Duplicate,
}

impl From<sqlx::Error> for SqlitePersistedEntityError {
    fn from(_: sqlx::Error) -> Self {
        Self::PersistenceFailure
    }
}

#[allow(dead_code)]
enum PersistedEntityTxError {
    Repo(SqlitePersistedEntityError),
    Sql(sqlx::Error),
}

impl From<sqlx::Error> for PersistedEntityTxError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sql(error)
    }
}

impl PersistedEntityTxError {
    fn into_repo(self) -> SqlitePersistedEntityError {
        match self {
            Self::Repo(error) => error,
            Self::Sql(_) => SqlitePersistedEntityError::PersistenceFailure,
        }
    }
}

pub struct SqlitePersistedEntityRepository {
    db: BlockingDevicePool,
}

impl SqlitePersistedEntityRepository {
    pub fn new_in_memory() -> Result<Self, sqlx::Error> {
        Self::open("file:sdkwork-aiot-admin-entity?mode=memory&cache=shared")
    }

    pub fn from_blocking_pool(db: BlockingDevicePool) -> Result<Self, sqlx::Error> {
        ensure_device_schema(&db)?;
        Ok(Self { db })
    }

    pub fn open(path_or_uri: impl AsRef<std::path::Path>) -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url(path_or_uri.as_ref().to_string_lossy().as_ref());
        let sqlite = BlockingSqlitePool::connect(&url)?;
        Self::from_blocking_pool(BlockingDevicePool::Sqlite(sqlite))
    }

    pub(crate) fn blocking_pool(&self) -> BlockingDevicePool {
        self.db.clone()
    }

    pub fn upsert_entity(
        &self,
        association: &AiotStorageAssociation,
        entity_kind: &str,
        entity_key: &str,
        payload_json: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        let now = current_timestamp();
        let association = association.clone();
        let entity_kind = entity_kind.to_string();
        let entity_key = entity_key.to_string();
        let payload_json = payload_json.to_string();
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let update_sql = adapt_sqlite_placeholders(
                        dialect,
                        "UPDATE iot_admin_entity
                         SET payload_json = ?1, updated_at = ?2, status = ?3
                         WHERE tenant_id = ?4 AND organization_id = ?5 AND entity_kind = ?6 AND entity_key = ?7",
                    );
                    let updated = match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query(&update_sql)
                                .bind(&payload_json)
                                .bind(&now)
                                .bind(ENTITY_STATUS_ACTIVE)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&entity_kind)
                                .bind(&entity_key)
                                .execute(&mut **connection)
                                .await?
                                .rows_affected()
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query(&update_sql)
                                .bind(&payload_json)
                                .bind(&now)
                                .bind(ENTITY_STATUS_ACTIVE)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(&entity_kind)
                                .bind(&entity_key)
                                .execute(&mut **connection)
                                .await?
                                .rows_affected()
                        }
                    };
                    if updated > 0 {
                        return Ok(());
                    }

                    let next_id_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT COALESCE(MAX(id), 0) + 1 FROM iot_admin_entity",
                    );
                    let next_id: i64 = match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query_scalar(&next_id_sql)
                                .fetch_one(&mut **connection)
                                .await?
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query_scalar(&next_id_sql)
                                .fetch_one(&mut **connection)
                                .await?
                        }
                    };
                    let uuid = format!("admin-entity-{next_id:08}");
                    let insert_sql = adapt_sqlite_placeholders(
                        dialect,
                        "INSERT INTO iot_admin_entity (
                            id, uuid, tenant_id, organization_id, data_scope, entity_kind, entity_key,
                            payload_json, status, created_at, updated_at, version
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query(&insert_sql)
                                .bind(next_id)
                                .bind(&uuid)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(association.data_scope)
                                .bind(&entity_kind)
                                .bind(&entity_key)
                                .bind(&payload_json)
                                .bind(ENTITY_STATUS_ACTIVE)
                                .bind(&now)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query(&insert_sql)
                                .bind(next_id)
                                .bind(&uuid)
                                .bind(association.tenant_id)
                                .bind(association.organization_id)
                                .bind(association.data_scope)
                                .bind(&entity_kind)
                                .bind(&entity_key)
                                .bind(&payload_json)
                                .bind(ENTITY_STATUS_ACTIVE)
                                .bind(&now)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                    }
                    Ok(())
                })
            })
            .map_err(PersistedEntityTxError::into_repo)
    }

    pub fn get_entity(
        &self,
        association: &AiotStorageAssociation,
        entity_kind: &str,
        entity_key: &str,
    ) -> Option<SqlitePersistedEntityRecord> {
        let association = association.clone();
        let entity_kind = entity_kind.to_string();
        let entity_key = entity_key.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
            let sql = adapt_sqlite_placeholders(
                dialect,
                "SELECT entity_kind, entity_key, payload_json
                 FROM iot_admin_entity
                 WHERE tenant_id = ?1 AND organization_id = ?2 AND entity_kind = ?3 AND entity_key = ?4 AND status = ?5",
            );
            match pool.engine() {
                DeviceDatabaseEngine::Sqlite => {
                    let row = sqlx::query(&sql)
                        .bind(association.tenant_id)
                        .bind(association.organization_id)
                        .bind(&entity_kind)
                        .bind(&entity_key)
                        .bind(ENTITY_STATUS_ACTIVE)
                        .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                        .await?;
                    row.as_ref().map(row_to_entity_record).transpose()
                }
                DeviceDatabaseEngine::Postgres => {
                    let row = sqlx::query(&sql)
                        .bind(association.tenant_id)
                        .bind(association.organization_id)
                        .bind(&entity_kind)
                        .bind(&entity_key)
                        .bind(ENTITY_STATUS_ACTIVE)
                        .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                        .await?;
                    row.as_ref()
                        .map(row_to_entity_record_postgres)
                        .transpose()
                }
            }
        })
        .ok()
        .flatten()
    }

    pub fn list_entities_page(
        &self,
        association: &AiotStorageAssociation,
        entity_kind: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<SqlitePersistedEntityRecord>, SqlitePersistedEntityError> {
        let association = association.clone();
        let entity_kind = entity_kind.to_string();
        let limit = params.page_size.max(1);
        let offset = params.offset.max(0);
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let count_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT COUNT(1) FROM iot_admin_entity
                     WHERE tenant_id = ?1 AND organization_id = ?2 AND entity_kind = ?3 AND status = ?4",
                );
                let list_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT entity_kind, entity_key, payload_json
                     FROM iot_admin_entity
                     WHERE tenant_id = ?1 AND organization_id = ?2 AND entity_kind = ?3 AND status = ?4
                     ORDER BY id ASC
                     LIMIT ?5 OFFSET ?6",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<AiotOffsetListResult<SqlitePersistedEntityRecord>, sqlx::Error>(
                            AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_entity_record(row).ok())
                                .collect(),
                            total,
                        })
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let total: i64 = sqlx::query_scalar(&count_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let rows = sqlx::query(&list_sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .bind(limit)
                            .bind(offset)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<AiotOffsetListResult<SqlitePersistedEntityRecord>, sqlx::Error>(
                            AiotOffsetListResult {
                            items: rows
                                .iter()
                                .filter_map(|row| row_to_entity_record_postgres(row).ok())
                                .collect(),
                            total,
                        })
                    }
                }
            })
            .map_err(|_: sqlx::Error| SqlitePersistedEntityError::PersistenceFailure)
    }

    pub fn list_entities(
        &self,
        association: &AiotStorageAssociation,
        entity_kind: &str,
    ) -> Vec<SqlitePersistedEntityRecord> {
        let association = association.clone();
        let entity_kind = entity_kind.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT entity_kind, entity_key, payload_json
                 FROM iot_admin_entity
                 WHERE tenant_id = ?1 AND organization_id = ?2 AND entity_kind = ?3 AND status = ?4
                 ORDER BY id ASC",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let rows = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<Vec<SqlitePersistedEntityRecord>, sqlx::Error>(
                            rows.iter()
                                .filter_map(|row| row_to_entity_record(row).ok())
                                .collect(),
                        )
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let rows = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<Vec<SqlitePersistedEntityRecord>, sqlx::Error>(
                            rows.iter()
                                .filter_map(|row| row_to_entity_record_postgres(row).ok())
                                .collect(),
                        )
                    }
                }
            })
            .unwrap_or_default()
    }

    pub fn delete_entity(
        &self,
        association: &AiotStorageAssociation,
        entity_kind: &str,
        entity_key: &str,
    ) -> Result<(), SqlitePersistedEntityError> {
        let now = current_timestamp();
        let association = association.clone();
        let entity_kind = entity_kind.to_string();
        let entity_key = entity_key.to_string();
        let updated = self
            .db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "UPDATE iot_admin_entity
                     SET status = 0, updated_at = ?1
                     WHERE tenant_id = ?2 AND organization_id = ?3 AND entity_kind = ?4 AND entity_key = ?5 AND status = ?6",
                );
                let updated: u64 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query(&sql)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(&entity_key)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .execute(pool.sqlite_pool().expect("sqlite pool"))
                            .await?
                            .rows_affected()
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query(&sql)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&entity_kind)
                            .bind(&entity_key)
                            .bind(ENTITY_STATUS_ACTIVE)
                            .execute(pool.postgres_pool().expect("postgres pool"))
                            .await?
                            .rows_affected()
                    }
                };
                Ok::<u64, sqlx::Error>(updated)
            })
            .map_err(|_: sqlx::Error| SqlitePersistedEntityError::PersistenceFailure)?;
        if updated == 0 {
            return Err(SqlitePersistedEntityError::NotFound);
        }
        Ok(())
    }
}

fn row_to_entity_record(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<SqlitePersistedEntityRecord, sqlx::Error> {
    Ok(SqlitePersistedEntityRecord {
        entity_kind: row.try_get("entity_kind")?,
        entity_key: row.try_get("entity_key")?,
        payload_json: row.try_get("payload_json")?,
    })
}

fn row_to_entity_record_postgres(
    row: &sqlx::postgres::PgRow,
) -> Result<SqlitePersistedEntityRecord, sqlx::Error> {
    Ok(SqlitePersistedEntityRecord {
        entity_kind: row.try_get("entity_kind")?,
        entity_key: row.try_get("entity_key")?,
        payload_json: row.try_get("payload_json")?,
    })
}

fn current_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}
