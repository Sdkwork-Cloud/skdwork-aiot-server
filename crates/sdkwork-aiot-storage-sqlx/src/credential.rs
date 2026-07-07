use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sdkwork_aiot_storage::{AiotOffsetListResult, AiotStorageAssociation, OffsetListPageParams};
use sqlx::Row;

use sdkwork_utils_rust::is_blank;

use crate::blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine, DeviceDbTransaction};
use crate::credential_hash::{hash_device_credential_secret, verify_device_credential_secret};
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::row_id_allocator::allocate_row_id;
use crate::schema::ensure_device_schema;
use crate::sqlite_sync::{sqlite_connect_url, BlockingSqlitePool};

const CREDENTIAL_STATUS_ACTIVE: i32 = 1;
const CREDENTIAL_STATUS_REVOKED: i32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteDeviceCredentialRecord {
    pub credential_id: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub device_id: String,
    pub credential_type: String,
    pub status: String,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub issued_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SqliteCredentialCreateCommand {
    pub association: AiotStorageAssociation,
    pub device_id: String,
    pub credential_type: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqliteCredentialRepositoryError {
    PersistenceFailure,
    CredentialNotFound,
}

impl From<sqlx::Error> for SqliteCredentialRepositoryError {
    fn from(_: sqlx::Error) -> Self {
        Self::PersistenceFailure
    }
}

#[allow(dead_code)]
enum CredentialRepoTxError {
    Repo(SqliteCredentialRepositoryError),
    Sql(sqlx::Error),
}

impl From<sqlx::Error> for CredentialRepoTxError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sql(error)
    }
}

impl CredentialRepoTxError {
    fn into_repo(self) -> SqliteCredentialRepositoryError {
        match self {
            Self::Repo(error) => error,
            Self::Sql(_) => SqliteCredentialRepositoryError::PersistenceFailure,
        }
    }
}

pub struct SqliteSqlxCredentialRepository {
    db: BlockingDevicePool,
}

impl SqliteSqlxCredentialRepository {
    pub fn new_in_memory() -> Result<Self, sqlx::Error> {
        Self::open("file:sdkwork-aiot-credential?mode=memory&cache=shared")
    }

    pub fn from_blocking_pool(db: BlockingDevicePool) -> Result<Self, sqlx::Error> {
        ensure_device_schema(&db)?;
        Ok(Self { db })
    }

    pub fn open(path_or_uri: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url(path_or_uri.as_ref().to_string_lossy().as_ref());
        let sqlite = BlockingSqlitePool::connect(&url)?;
        Self::from_blocking_pool(BlockingDevicePool::Sqlite(sqlite))
    }

    pub fn verify_bearer_token(&self, device_id: &str, token: &str) -> bool {
        self.resolve_association_for_bearer_token(device_id, token)
            .is_some()
    }

    /// Verifies a device bearer token. Tenant and organization scope are resolved from
    /// stored credentials — client-supplied tenant headers are not trusted.
    pub fn verify_bearer_token_scoped(
        &self,
        tenant_id: Option<i64>,
        organization_id: Option<i64>,
        device_id: &str,
        token: &str,
    ) -> bool {
        if is_blank(Some(device_id)) || is_blank(Some(token)) {
            return false;
        }
        let now = current_rfc3339_timestamp();
        let device_id = device_id.to_string();
        let token = token.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let mut query = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT credential_hash FROM iot_device_credential
                 WHERE device_id = ?1
                   AND status = ?2
                   AND (expires_at IS NULL OR expires_at > ?3)",
                );
                if tenant_id.is_some() {
                    query.push_str(" AND tenant_id = ?4");
                }
                if organization_id.is_some() {
                    query.push_str(" AND organization_id = ?5");
                }
                let query = adapt_sqlite_placeholders(dialect, &query);

                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let mut sql_query = sqlx::query(&query)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now);
                        if let Some(tenant_id) = tenant_id {
                            sql_query = sql_query.bind(tenant_id);
                        }
                        if let Some(organization_id) = organization_id {
                            sql_query = sql_query.bind(organization_id);
                        }
                        let rows = sql_query
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<bool, sqlx::Error>(rows.into_iter().any(|row| {
                            row.try_get::<String, _>("credential_hash")
                                .ok()
                                .filter(|stored| !stored.is_empty())
                                .is_some_and(|stored| {
                                    verify_device_credential_secret(&stored, token.as_bytes())
                                })
                        }))
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let mut sql_query = sqlx::query(&query)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now);
                        if let Some(tenant_id) = tenant_id {
                            sql_query = sql_query.bind(tenant_id);
                        }
                        if let Some(organization_id) = organization_id {
                            sql_query = sql_query.bind(organization_id);
                        }
                        let rows = sql_query
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<bool, sqlx::Error>(rows.into_iter().any(|row| {
                            row.try_get::<String, _>("credential_hash")
                                .ok()
                                .filter(|stored| !stored.is_empty())
                                .is_some_and(|stored| {
                                    verify_device_credential_secret(&stored, token.as_bytes())
                                })
                        }))
                    }
                }
            })
            .unwrap_or(false)
    }

    /// Resolves tenant scope from the credential row that matches the bearer token.
    pub fn resolve_association_for_bearer_token(
        &self,
        device_id: &str,
        token: &str,
    ) -> Option<AiotStorageAssociation> {
        if is_blank(Some(device_id)) || is_blank(Some(token)) {
            return None;
        }
        let now = current_rfc3339_timestamp();
        let device_id = device_id.to_string();
        let token = token.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT tenant_id, organization_id, credential_hash
                 FROM iot_device_credential
                 WHERE device_id = ?1
                   AND status = ?2
                   AND (expires_at IS NULL OR expires_at > ?3)",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let rows = sqlx::query(&sql)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now)
                            .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        Ok::<Option<AiotStorageAssociation>, sqlx::Error>(
                            rows.iter().find_map(|row| {
                                row.try_get::<String, _>("credential_hash")
                                    .ok()
                                    .filter(|stored| !stored.is_empty())
                                    .filter(|stored| {
                                        verify_device_credential_secret(stored, token.as_bytes())
                                    })
                                    .map(|_| {
                                        AiotStorageAssociation::tenant_org(
                                            row.get("tenant_id"),
                                            row.get("organization_id"),
                                        )
                                    })
                            }),
                        )
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let rows = sqlx::query(&sql)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now)
                            .fetch_all(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        Ok::<Option<AiotStorageAssociation>, sqlx::Error>(
                            rows.iter().find_map(|row| {
                                row.try_get::<String, _>("credential_hash")
                                    .ok()
                                    .filter(|stored| !stored.is_empty())
                                    .filter(|stored| {
                                        verify_device_credential_secret(stored, token.as_bytes())
                                    })
                                    .map(|_| {
                                        AiotStorageAssociation::tenant_org(
                                            row.get("tenant_id"),
                                            row.get("organization_id"),
                                        )
                                    })
                            }),
                        )
                    }
                }
            })
            .ok()
            .flatten()
    }

    pub fn device_has_active_credential(&self, device_id: &str) -> bool {
        if is_blank(Some(device_id)) {
            return false;
        }
        let now = current_rfc3339_timestamp();
        let device_id = device_id.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT COUNT(1) FROM iot_device_credential
                 WHERE device_id = ?1
                   AND status = ?2
                   AND (expires_at IS NULL OR expires_at > ?3)",
                );
                let count: i64 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query_scalar(&sql)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query_scalar(&sql)
                            .bind(&device_id)
                            .bind(CREDENTIAL_STATUS_ACTIVE)
                            .bind(&now)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?
                    }
                };
                Ok::<bool, sqlx::Error>(count > 0)
            })
            .unwrap_or(false)
    }

    pub fn create_credential(
        &self,
        command: SqliteCredentialCreateCommand,
    ) -> Result<SqliteDeviceCredentialRecord, SqliteCredentialRepositoryError> {
        let now = current_rfc3339_timestamp();
        self.db
            .with_device_transaction(|mut tx, dialect| {
                Box::pin(async move {
                    let next_id =
                        allocate_row_id(&mut tx, dialect, "iot_device_credential").await?;
                    let credential_id = format!("credential-{next_id:04}");
                    let issued_secret = generate_device_secret(&command.device_id, next_id);
                    let credential_hash = hash_device_credential_secret(issued_secret.as_bytes());

                    let insert_sql = adapt_sqlite_placeholders(
                        dialect,
                        "INSERT INTO iot_device_credential (
                            id, uuid, tenant_id, organization_id, data_scope, device_id,
                            credential_type, credential_hash, credential_ref, expires_at,
                            status, created_at, updated_at, version
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0)",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query(&insert_sql)
                                .bind(next_id)
                                .bind(&credential_id)
                                .bind(command.association.tenant_id)
                                .bind(command.association.organization_id)
                                .bind(command.association.data_scope)
                                .bind(&command.device_id)
                                .bind(&command.credential_type)
                                .bind(credential_hash)
                                .bind(&credential_id)
                                .bind(command.expires_at.as_deref())
                                .bind(CREDENTIAL_STATUS_ACTIVE)
                                .bind(&now)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query(&insert_sql)
                                .bind(next_id)
                                .bind(&credential_id)
                                .bind(command.association.tenant_id)
                                .bind(command.association.organization_id)
                                .bind(command.association.data_scope)
                                .bind(&command.device_id)
                                .bind(&command.credential_type)
                                .bind(credential_hash)
                                .bind(&credential_id)
                                .bind(command.expires_at.as_deref())
                                .bind(CREDENTIAL_STATUS_ACTIVE)
                                .bind(&now)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                    }

                    Ok(SqliteDeviceCredentialRecord {
                        credential_id,
                        tenant_id: command.association.tenant_id,
                        organization_id: command.association.organization_id,
                        device_id: command.device_id,
                        credential_type: command.credential_type,
                        status: "active".to_string(),
                        expires_at: command.expires_at,
                        created_at: now.clone(),
                        revoked_at: None,
                        issued_secret: Some(issued_secret),
                    })
                })
            })
            .map_err(CredentialRepoTxError::into_repo)
    }

    pub fn list_credentials(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        params: OffsetListPageParams,
    ) -> Result<AiotOffsetListResult<SqliteDeviceCredentialRecord>, SqliteCredentialRepositoryError>
    {
        let association = association.clone();
        let device_id = device_id.to_string();
        let limit = params.page_size.max(1);
        let offset = params.offset.max(0);
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let count_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT COUNT(1) FROM iot_device_credential
                 WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3",
                );
                let list_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT uuid, tenant_id, organization_id, device_id, credential_type, status,
                        expires_at, created_at, updated_at
                 FROM iot_device_credential
                 WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3
                 ORDER BY id ASC
                 LIMIT ?4 OFFSET ?5",
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
                                .filter_map(|row| row_to_credential_record(row).ok())
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
                                .filter_map(|row| row_to_credential_record_postgres(row).ok())
                                .collect(),
                            total,
                        })
                    }
                }
            })
            .map_err(|_: sqlx::Error| SqliteCredentialRepositoryError::PersistenceFailure)
    }

    pub fn get_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Option<SqliteDeviceCredentialRecord> {
        let association = association.clone();
        let device_id = device_id.to_string();
        let credential_id = credential_id.to_string();
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT uuid, tenant_id, organization_id, device_id, credential_type, status,
                        expires_at, created_at
                 FROM iot_device_credential
                 WHERE tenant_id = ?1 AND organization_id = ?2 AND device_id = ?3 AND uuid = ?4",
                );
                match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&credential_id)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        row.as_ref().map(row_to_credential_record).transpose()
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&sql)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&credential_id)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        row.as_ref()
                            .map(row_to_credential_record_postgres)
                            .transpose()
                    }
                }
            })
            .ok()
            .flatten()
    }

    pub fn revoke_credential(
        &self,
        association: &AiotStorageAssociation,
        device_id: &str,
        credential_id: &str,
    ) -> Result<(), SqliteCredentialRepositoryError> {
        let now = current_rfc3339_timestamp();
        let association = association.clone();
        let device_id = device_id.to_string();
        let credential_id = credential_id.to_string();
        let updated = self
            .db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "UPDATE iot_device_credential
                     SET status = ?1, updated_at = ?2
                     WHERE tenant_id = ?3 AND organization_id = ?4 AND device_id = ?5 AND uuid = ?6",
                );
                let result = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query(&sql)
                            .bind(CREDENTIAL_STATUS_REVOKED)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&credential_id)
                            .execute(pool.sqlite_pool().expect("sqlite pool"))
                            .await
                            .map(|result| result.rows_affected())
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query(&sql)
                            .bind(CREDENTIAL_STATUS_REVOKED)
                            .bind(&now)
                            .bind(association.tenant_id)
                            .bind(association.organization_id)
                            .bind(&device_id)
                            .bind(&credential_id)
                            .execute(pool.postgres_pool().expect("postgres pool"))
                            .await
                            .map(|result| result.rows_affected())
                    }
                }?;
                Ok(result)
            })
            .map_err(|_: sqlx::Error| SqliteCredentialRepositoryError::PersistenceFailure)?;
        if updated == 0 {
            return Err(SqliteCredentialRepositoryError::CredentialNotFound);
        }
        Ok(())
    }
}

fn row_to_credential_record(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<SqliteDeviceCredentialRecord, sqlx::Error> {
    Ok(SqliteDeviceCredentialRecord {
        credential_id: row.try_get("uuid")?,
        tenant_id: row.try_get("tenant_id")?,
        organization_id: row.try_get("organization_id")?,
        device_id: row.try_get("device_id")?,
        credential_type: row.try_get("credential_type")?,
        status: credential_status_label(row.try_get::<i64, _>("status")? as i32).to_string(),
        expires_at: read_optional_timestamp_column_sqlite(row, "expires_at")?,
        created_at: read_timestamp_column_sqlite(row, "created_at")?,
        revoked_at: None,
        issued_secret: None,
    })
}

fn row_to_credential_record_postgres(
    row: &sqlx::postgres::PgRow,
) -> Result<SqliteDeviceCredentialRecord, sqlx::Error> {
    Ok(SqliteDeviceCredentialRecord {
        credential_id: row.try_get("uuid")?,
        tenant_id: row.try_get("tenant_id")?,
        organization_id: row.try_get("organization_id")?,
        device_id: row.try_get("device_id")?,
        credential_type: row.try_get("credential_type")?,
        status: credential_status_label(row.try_get::<i64, _>("status")? as i32).to_string(),
        expires_at: read_optional_timestamp_column_postgres(row, "expires_at")?,
        created_at: read_timestamp_column_postgres(row, "created_at")?,
        revoked_at: None,
        issued_secret: None,
    })
}

fn read_timestamp_column_sqlite(
    row: &sqlx::sqlite::SqliteRow,
    column: &'static str,
) -> Result<String, sqlx::Error> {
    row.try_get::<String, _>(column).or_else(|_| {
        let value: i64 = row.try_get(column)?;
        Ok(value.to_string())
    })
}

fn read_timestamp_column_postgres(
    row: &sqlx::postgres::PgRow,
    column: &'static str,
) -> Result<String, sqlx::Error> {
    row.try_get::<String, _>(column).or_else(|_| {
        let value: i64 = row.try_get(column)?;
        Ok(value.to_string())
    })
}

fn read_optional_timestamp_column_sqlite(
    row: &sqlx::sqlite::SqliteRow,
    column: &'static str,
) -> Result<Option<String>, sqlx::Error> {
    if row.try_get::<Option<String>, _>(column)?.is_none()
        && row.try_get::<Option<i64>, _>(column)?.is_none()
    {
        return Ok(None);
    }
    read_timestamp_column_sqlite(row, column).map(Some)
}

fn read_optional_timestamp_column_postgres(
    row: &sqlx::postgres::PgRow,
    column: &'static str,
) -> Result<Option<String>, sqlx::Error> {
    if row.try_get::<Option<String>, _>(column)?.is_none()
        && row.try_get::<Option<i64>, _>(column)?.is_none()
    {
        return Ok(None);
    }
    read_timestamp_column_postgres(row, column).map(Some)
}

fn credential_status_label(status: i32) -> &'static str {
    if status == CREDENTIAL_STATUS_ACTIVE {
        "active"
    } else {
        "revoked"
    }
}

fn generate_device_secret(_device_id: &str, _sequence: i64) -> String {
    use rand_core::{OsRng, RngCore};
    use sdkwork_utils_rust::sha256_hash;

    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    sha256_hash(&bytes)
}

fn current_rfc3339_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("{seconds}")
}
