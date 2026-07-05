//! Device schema bootstrap through engine-aware blocking pools.

use std::collections::BTreeSet;
use std::sync::Mutex;

use sqlx::Row;

use crate::blocking_device_pool::{BlockingDevicePool, DeviceDatabaseEngine};
use crate::{migration_catalog, SqlMigration};

static DEVICE_SCHEMA_INIT: Mutex<()> = Mutex::new(());

pub(crate) fn ensure_device_schema(pool: &BlockingDevicePool) -> Result<(), sqlx::Error> {
    let _init_guard = DEVICE_SCHEMA_INIT
        .lock()
        .expect("device schema init mutex poisoned");
    pool.execute_batch_sql(
        "CREATE TABLE IF NOT EXISTS iot_schema_version (
            version TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
    )?;

    let mut applied_versions = pool.run(async { load_applied_schema_versions(pool).await })?;
    bootstrap_legacy_schema_version(pool, &mut applied_versions)?;

    for migration in migration_catalog() {
        if applied_versions.contains(migration.version) {
            continue;
        }
        pool.execute_batch_sql(migration.sql)?;
        record_applied_schema_version(pool, &migration)?;
        applied_versions.insert(migration.version.to_string());
    }

    Ok(())
}

async fn load_applied_schema_versions(
    pool: &BlockingDevicePool,
) -> Result<BTreeSet<String>, sqlx::Error> {
    let sql = pool.adapt_sql("SELECT version FROM iot_schema_version ORDER BY version ASC");
    match pool.engine() {
        DeviceDatabaseEngine::Sqlite => {
            let rows = sqlx::query(&sql)
                .fetch_all(pool.sqlite_pool().expect("sqlite pool"))
                .await?;
            Ok(rows
                .into_iter()
                .filter_map(|row| row.try_get::<String, _>("version").ok())
                .collect())
        }
        DeviceDatabaseEngine::Postgres => {
            let rows = sqlx::query(&sql)
                .fetch_all(pool.postgres_pool().expect("postgres pool"))
                .await?;
            Ok(rows
                .into_iter()
                .filter_map(|row| row.try_get::<String, _>("version").ok())
                .collect())
        }
    }
}

fn bootstrap_legacy_schema_version(
    pool: &BlockingDevicePool,
    applied_versions: &mut BTreeSet<String>,
) -> Result<(), sqlx::Error> {
    if !applied_versions.is_empty() {
        return Ok(());
    }

    let legacy_device_table: i64 = pool.run(async {
        match pool.engine() {
            DeviceDatabaseEngine::Sqlite => {
                sqlx::query_scalar(
                    "SELECT COUNT(1) FROM sqlite_master WHERE type = 'table' AND name = 'iot_device'",
                )
                .fetch_one(pool.sqlite_pool().expect("sqlite pool"))
                .await
            }
            DeviceDatabaseEngine::Postgres => {
                sqlx::query_scalar(
                    "SELECT COUNT(1) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'iot_device'",
                )
                .fetch_one(pool.postgres_pool().expect("postgres pool"))
                .await
            }
        }
    })?;
    if legacy_device_table == 0 {
        return Ok(());
    }

    let Some(migration) = migration_catalog().into_iter().next() else {
        return Ok(());
    };
    record_applied_schema_version(pool, &migration)?;
    applied_versions.insert(migration.version.to_string());
    Ok(())
}

fn record_applied_schema_version(
    pool: &BlockingDevicePool,
    migration: &SqlMigration,
) -> Result<(), sqlx::Error> {
    pool.run(async {
        let applied_at = default_timestamp();
        match pool.engine() {
            DeviceDatabaseEngine::Sqlite => {
                sqlx::query(
                    "INSERT OR IGNORE INTO iot_schema_version (version, name, schema_version, applied_at) VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(migration.version)
                .bind(migration.name)
                .bind(migration.schema_version)
                .bind(applied_at)
                .execute(pool.sqlite_pool().expect("sqlite pool"))
                .await?;
            }
            DeviceDatabaseEngine::Postgres => {
                sqlx::query(
                    "INSERT INTO iot_schema_version (version, name, schema_version, applied_at) VALUES ($1, $2, $3, $4) ON CONFLICT (version) DO NOTHING",
                )
                .bind(migration.version)
                .bind(migration.name)
                .bind(migration.schema_version)
                .bind(applied_at)
                .execute(pool.postgres_pool().expect("postgres pool"))
                .await?;
            }
        }
        Ok(())
    })
}

fn default_timestamp() -> &'static str {
    "2026-06-01T00:00:00Z"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_version;
    use crate::sqlite_sync::BlockingSqlitePool;

    #[test]
    fn ensure_device_schema_applies_migrations() {
        let sqlite =
            BlockingSqlitePool::connect("file:sdkwork-aiot-schema-test?mode=memory&cache=shared")
                .expect("connect");
        let pool = BlockingDevicePool::Sqlite(sqlite);
        ensure_device_schema(&pool).expect("schema");
        let version = pool
            .run(async {
                sqlx::query_scalar::<_, String>(
                    "SELECT schema_version FROM iot_schema_version LIMIT 1",
                )
                .fetch_optional(pool.sqlite_pool().expect("sqlite pool"))
                .await
            })
            .expect("version query");
        assert_eq!(version.as_deref(), Some(schema_version()));
    }
}
