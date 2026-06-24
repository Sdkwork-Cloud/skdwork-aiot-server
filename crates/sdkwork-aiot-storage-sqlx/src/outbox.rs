use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sdkwork_aiot_storage::{
    AiotOutboxPendingEvent, OutboxEventRepository, OutboxEventRepositoryError,
    OUTBOX_STATUS_CLAIMED, OUTBOX_STATUS_FAILED, OUTBOX_STATUS_PENDING, OUTBOX_STATUS_PUBLISHED,
};
use sqlx::Row;

use crate::blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine, DeviceDbTransaction};
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::schema::ensure_device_schema;
use crate::sqlite_sync::{sqlite_connect_url, BlockingSqlitePool};
use crate::SqlDialect;

const OUTBOX_RETRY_BASE_SECONDS: i64 = 5;
const OUTBOX_CLAIM_LEASE_SECONDS: i64 = 60;

pub struct SqliteOutboxEventRepository {
    db: BlockingDevicePool,
}

impl SqliteOutboxEventRepository {
    pub fn from_blocking_pool(db: BlockingDevicePool) -> Result<Self, sqlx::Error> {
        ensure_device_schema(&db)?;
        Ok(Self { db })
    }

    pub fn open(path_or_uri: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let url = sqlite_connect_url(path_or_uri.as_ref().to_string_lossy().as_ref());
        let sqlite = BlockingSqlitePool::connect(&url)?;
        Self::from_blocking_pool(BlockingDevicePool::Sqlite(sqlite))
    }
}

impl OutboxEventRepository for SqliteOutboxEventRepository {
    fn pending_lag_count(&self) -> u64 {
        self.db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT COUNT(1) FROM iot_outbox_event WHERE status = ?1",
                );
                let count: i64 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        sqlx::query_scalar(&sql)
                            .bind(OUTBOX_STATUS_PENDING)
                            .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                            .await?
                    }
                    DeviceDatabaseEngine::Postgres => {
                        sqlx::query_scalar(&sql)
                            .bind(OUTBOX_STATUS_PENDING)
                            .fetch_one(pool.postgres_pool().expect("postgres pool"))
                            .await?
                    }
                };
                Ok::<u64, sqlx::Error>(count.max(0) as u64)
            })
            .unwrap_or(0)
    }

    fn claim_pending_batch(
        &self,
        limit: usize,
    ) -> Result<Vec<AiotOutboxPendingEvent>, OutboxEventRepositoryError> {
        let limit = limit.max(1) as i64;
        let now = current_rfc3339_timestamp();
        let lease_until = format_rfc3339(
            SystemTime::now() + Duration::from_secs(OUTBOX_CLAIM_LEASE_SECONDS as u64),
        );
        self.db
            .with_device_transaction(|mut tx, dialect| {
                let now = now.clone();
                let lease_until = lease_until.clone();
                Box::pin(async move {
                    let reclaim_sql = adapt_sqlite_placeholders(
                        dialect,
                        "UPDATE iot_outbox_event
                         SET status = ?1, next_attempt_at = NULL
                         WHERE status = ?2
                           AND next_attempt_at IS NOT NULL
                           AND next_attempt_at <= ?3",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query(&reclaim_sql)
                                .bind(OUTBOX_STATUS_PENDING)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query(&reclaim_sql)
                                .bind(OUTBOX_STATUS_PENDING)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&now)
                                .execute(&mut **connection)
                                .await?;
                        }
                    }

                    let claim_sql = outbox_claim_sql(dialect);
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            sqlx::query(&claim_sql)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&lease_until)
                                .bind(OUTBOX_STATUS_PENDING)
                                .bind(&now)
                                .bind(limit)
                                .execute(&mut **connection)
                                .await?;
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            sqlx::query(&claim_sql)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&lease_until)
                                .bind(OUTBOX_STATUS_PENDING)
                                .bind(&now)
                                .bind(limit)
                                .execute(&mut **connection)
                                .await?;
                        }
                    }

                    let select_sql = adapt_sqlite_placeholders(
                        dialect,
                        "SELECT tenant_id, organization_id, event_id, event_type, event_version, aggregate_type, aggregate_id, payload, trace_id, attempt_count
                         FROM iot_outbox_event
                         WHERE status = ?1 AND next_attempt_at = ?2
                         ORDER BY created_at ASC
                         LIMIT ?3",
                    );
                    match &mut tx {
                        DeviceDbTransaction::Sqlite(connection) => {
                            let rows = sqlx::query(&select_sql)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&lease_until)
                                .bind(limit)
                                .fetch_all(&mut **connection)
                                .await?;
                            Ok(rows
                                .iter()
                                .filter_map(row_to_pending_event)
                                .collect::<Vec<_>>())
                        }
                        DeviceDbTransaction::Postgres(connection) => {
                            let rows = sqlx::query(&select_sql)
                                .bind(OUTBOX_STATUS_CLAIMED)
                                .bind(&lease_until)
                                .bind(limit)
                                .fetch_all(&mut **connection)
                                .await?;
                            Ok(rows
                                .iter()
                                .filter_map(row_to_pending_event_postgres)
                                .collect::<Vec<_>>())
                        }
                    }
                })
            })
            .map_err(|_: sqlx::Error| OutboxEventRepositoryError::PersistenceFailure)
    }

    fn mark_published(
        &self,
        tenant_id: i64,
        event_id: &str,
    ) -> Result<(), OutboxEventRepositoryError> {
        let now = current_rfc3339_timestamp();
        let event_id = event_id.to_string();
        let changed = self
            .db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let sql = adapt_sqlite_placeholders(
                    dialect,
                    "UPDATE iot_outbox_event
                     SET status = ?1, published_at = ?2, next_attempt_at = NULL
                     WHERE tenant_id = ?3 AND event_id = ?4 AND status = ?5",
                );
                let changed: i64 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => sqlx::query(&sql)
                        .bind(OUTBOX_STATUS_PUBLISHED)
                        .bind(&now)
                        .bind(tenant_id)
                        .bind(&event_id)
                        .bind(OUTBOX_STATUS_CLAIMED)
                        .execute(pool.sqlite_pool().expect("sqlite pool"))
                        .await?
                        .rows_affected() as i64,
                    DeviceDatabaseEngine::Postgres => sqlx::query(&sql)
                        .bind(OUTBOX_STATUS_PUBLISHED)
                        .bind(&now)
                        .bind(tenant_id)
                        .bind(&event_id)
                        .bind(OUTBOX_STATUS_CLAIMED)
                        .execute(pool.postgres_pool().expect("postgres pool"))
                        .await?
                        .rows_affected()
                        as i64,
                };
                Ok::<i64, sqlx::Error>(changed)
            })
            .map_err(|_: sqlx::Error| OutboxEventRepositoryError::PersistenceFailure)?;
        if changed == 0 {
            return Err(OutboxEventRepositoryError::NotFound);
        }
        Ok(())
    }

    fn record_publish_failure(
        &self,
        tenant_id: i64,
        event_id: &str,
        max_attempts: u32,
    ) -> Result<(), OutboxEventRepositoryError> {
        let event_id = event_id.to_string();
        let changed = self
            .db
            .run_owned(|pool| async move {
                let dialect = pool.dialect();
                let select_sql = adapt_sqlite_placeholders(
                    dialect,
                    "SELECT attempt_count FROM iot_outbox_event
                     WHERE tenant_id = ?1 AND event_id = ?2 AND status = ?3",
                );
                let attempt_count: i32 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => {
                        let row = sqlx::query(&select_sql)
                            .bind(tenant_id)
                            .bind(&event_id)
                            .bind(OUTBOX_STATUS_CLAIMED)
                            .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                            .await?;
                        let Some(row) = row else {
                            return Ok(0_i64);
                        };
                        row.try_get("attempt_count")?
                    }
                    DeviceDatabaseEngine::Postgres => {
                        let row = sqlx::query(&select_sql)
                            .bind(tenant_id)
                            .bind(&event_id)
                            .bind(OUTBOX_STATUS_CLAIMED)
                            .fetch_optional(pool.postgres_pool().expect("postgres pool"))
                            .await?;
                        let Some(row) = row else {
                            return Ok(0_i64);
                        };
                        row.try_get("attempt_count")?
                    }
                };
                let next_attempt = attempt_count + 1;
                let backoff_seconds =
                    OUTBOX_RETRY_BASE_SECONDS.saturating_mul(1_i64 << next_attempt.min(6));
                let next_attempt_at =
                    format_rfc3339(SystemTime::now() + Duration::from_secs(backoff_seconds as u64));
                let status = if next_attempt as u32 >= max_attempts {
                    OUTBOX_STATUS_FAILED
                } else {
                    OUTBOX_STATUS_PENDING
                };
                let update_sql = adapt_sqlite_placeholders(
                    dialect,
                    "UPDATE iot_outbox_event
                     SET attempt_count = ?1, next_attempt_at = ?2, status = ?3
                     WHERE tenant_id = ?4 AND event_id = ?5 AND status = ?6",
                );
                let changed: i64 = match pool.engine() {
                    DeviceDatabaseEngine::Sqlite => sqlx::query(&update_sql)
                        .bind(next_attempt)
                        .bind(next_attempt_at)
                        .bind(status)
                        .bind(tenant_id)
                        .bind(&event_id)
                        .bind(OUTBOX_STATUS_CLAIMED)
                        .execute(pool.sqlite_pool().expect("sqlite pool"))
                        .await?
                        .rows_affected() as i64,
                    DeviceDatabaseEngine::Postgres => sqlx::query(&update_sql)
                        .bind(next_attempt)
                        .bind(next_attempt_at)
                        .bind(status)
                        .bind(tenant_id)
                        .bind(&event_id)
                        .bind(OUTBOX_STATUS_CLAIMED)
                        .execute(pool.postgres_pool().expect("postgres pool"))
                        .await?
                        .rows_affected()
                        as i64,
                };
                Ok::<i64, sqlx::Error>(changed)
            })
            .map_err(|_: sqlx::Error| OutboxEventRepositoryError::PersistenceFailure)?;
        if changed == 0 {
            return Err(OutboxEventRepositoryError::NotFound);
        }
        Ok(())
    }
}

fn outbox_claim_sql(dialect: SqlDialect) -> String {
    let sql = match dialect {
        SqlDialect::Sqlite => {
            "UPDATE iot_outbox_event
             SET status = ?1, next_attempt_at = ?2
             WHERE rowid IN (
               SELECT rowid FROM iot_outbox_event
               WHERE status = ?3
                 AND (next_attempt_at IS NULL OR next_attempt_at <= ?4)
               ORDER BY created_at ASC
               LIMIT ?5
             )"
        }
        SqlDialect::Postgres => {
            "UPDATE iot_outbox_event
             SET status = ?1, next_attempt_at = ?2
             WHERE id IN (
               SELECT id FROM iot_outbox_event
               WHERE status = ?3
                 AND (next_attempt_at IS NULL OR next_attempt_at <= ?4)
               ORDER BY created_at ASC
               LIMIT ?5
             )"
        }
    };
    adapt_sqlite_placeholders(dialect, sql)
}

fn row_to_pending_event(row: &sqlx::sqlite::SqliteRow) -> Option<AiotOutboxPendingEvent> {
    Some(AiotOutboxPendingEvent {
        tenant_id: row.try_get("tenant_id").ok()?,
        organization_id: row.try_get("organization_id").ok()?,
        event_id: row.try_get("event_id").ok()?,
        event_type: row.try_get("event_type").ok()?,
        event_version: row.try_get("event_version").ok()?,
        aggregate_type: row.try_get("aggregate_type").ok()?,
        aggregate_id: row.try_get("aggregate_id").ok()?,
        payload_json: row.try_get("payload").ok()?,
        trace_id: row.try_get("trace_id").ok(),
        attempt_count: row.try_get("attempt_count").ok()?,
    })
}

fn row_to_pending_event_postgres(row: &sqlx::postgres::PgRow) -> Option<AiotOutboxPendingEvent> {
    Some(AiotOutboxPendingEvent {
        tenant_id: row.try_get("tenant_id").ok()?,
        organization_id: row.try_get("organization_id").ok()?,
        event_id: row.try_get("event_id").ok()?,
        event_type: row.try_get("event_type").ok()?,
        event_version: row.try_get("event_version").ok()?,
        aggregate_type: row.try_get("aggregate_type").ok()?,
        aggregate_id: row.try_get("aggregate_id").ok()?,
        payload_json: row.try_get("payload").ok()?,
        trace_id: row.try_get("trace_id").ok(),
        attempt_count: row.try_get("attempt_count").ok()?,
    })
}

fn current_rfc3339_timestamp() -> String {
    format_rfc3339(SystemTime::now())
}

pub(crate) fn current_rfc3339_timestamp_for_insert() -> String {
    current_rfc3339_timestamp()
}

fn format_rfc3339(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    let days = secs / 86_400;
    let remaining = secs % 86_400;
    let hours = remaining / 3_600;
    let minutes = (remaining % 3_600) / 60;
    let seconds = remaining % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
