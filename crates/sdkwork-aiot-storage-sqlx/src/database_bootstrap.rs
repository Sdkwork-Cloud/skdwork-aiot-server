//! AIoT device database bootstrap through `sdkwork-database`.

use sdkwork_database_config::workspace_database::workspace_postgres_env_is_configured;
use sdkwork_database_config::{DatabaseConfig, DatabaseEngine, DeploymentMode};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool, PoolError};

use crate::blocking_device_pool::BlockingDevicePool;
use crate::postgres_sync::BlockingPostgresPool;
use crate::sqlite_sync::BlockingSqlitePool;

pub const AIOT_DEVICE_DATABASE_SERVICE_NAME: &str = "AIOT_DEVICE";

/// Canonical SQLite memory configuration for explicit client-local tests.
pub fn aiot_device_sqlite_memory_config() -> DatabaseConfig {
    DatabaseConfig {
        engine: DatabaseEngine::Sqlite,
        url: "file:sdkwork-aiot-device-db?mode=memory&cache=shared".to_owned(),
        mode: DeploymentMode::Standalone,
        table_prefix: "iot_".to_owned(),
        max_connections: 5,
        min_connections: 1,
        ..DatabaseConfig::default()
    }
}

/// SQLite file configuration for durable device persistence.
pub fn aiot_device_sqlite_file_config(path: &str) -> DatabaseConfig {
    DatabaseConfig {
        engine: DatabaseEngine::Sqlite,
        url: format!("sqlite:{}?mode=rwc", path.replace('\\', "/")),
        mode: DeploymentMode::Standalone,
        table_prefix: "iot_".to_owned(),
        max_connections: 5,
        min_connections: 1,
        ..DatabaseConfig::default()
    }
}

/// Resolves the active device database config from an explicit client-local path or canonical env.
pub fn resolve_device_database_config(device_db_path: Option<&str>) -> DatabaseConfig {
    resolve_device_database_config_from_env(device_db_path)
        .expect("device database config must resolve for sqlite-backed callers")
}

/// Resolves device database config from an explicit client-local path or `SDKWORK_DATABASE_*`.
pub fn resolve_device_database_config_from_env(
    device_db_path: Option<&str>,
) -> Result<DatabaseConfig, PoolError> {
    if let Some(path) = device_db_path {
        return Ok(aiot_device_sqlite_file_config(path));
    }
    sdkwork_aiot_database_host::resolve_aiot_database_config_from_env()
        .map_err(|error| PoolError::DatabaseConfig(error.to_string()))
}

/// Returns true when an explicit canonical profile resolves to authoritative PostgreSQL.
pub fn device_database_config_is_authoritative_postgres_from_env() -> bool {
    if !workspace_postgres_env_is_configured() {
        return false;
    }
    sdkwork_aiot_database_host::resolve_aiot_database_config_from_env()
        .ok()
        .is_some_and(|config| config.engine == DatabaseEngine::Postgres)
}

/// Returns the SQLite URL from a validated SDKWork database config.
#[allow(dead_code)]
pub fn sqlite_url_from_config(config: &DatabaseConfig) -> &str {
    assert_eq!(config.engine, DatabaseEngine::Sqlite);
    assert!(
        config.table_prefix.starts_with("iot_"),
        "AIoT device database must use iot_ table prefix"
    );
    &config.url
}

/// Opens a shared-memory SQLite pool through `sdkwork-database-sqlx`.
pub async fn aiot_device_sqlite_memory_pool() -> Result<DatabasePool, PoolError> {
    create_pool_from_config(aiot_device_sqlite_memory_config()).await
}

/// Loads a device database pool from canonical `SDKWORK_DATABASE_*` variables.
pub async fn aiot_device_pool_from_env() -> Result<Option<DatabasePool>, PoolError> {
    let config = resolve_device_database_config_from_env(None)?;
    create_pool_from_config(config).await.map(Some)
}

/// Opens a synchronous SQLite pool through `sdkwork-database-sqlx` for legacy sync repositories.
pub fn aiot_device_blocking_pool(
    device_db_path: Option<&str>,
) -> Result<BlockingDevicePool, PoolError> {
    aiot_device_blocking_pool_from_env(device_db_path)
}

/// Opens a synchronous device pool from an explicit path or canonical database env.
pub fn aiot_device_blocking_pool_from_env(
    device_db_path: Option<&str>,
) -> Result<BlockingDevicePool, PoolError> {
    let config = resolve_device_database_config_from_env(device_db_path)?;
    match config.engine {
        DatabaseEngine::Sqlite => {
            let runtime = crate::runtime_bridge::shared_runtime()
                .map_err(|error| PoolError::DatabaseConfig(format!("tokio runtime: {error}")))?;
            let database_pool =
                crate::runtime_bridge::block_on(&runtime, create_pool_from_config(config))?;
            let sqlite_pool = database_pool.as_sqlite().ok_or_else(|| {
                PoolError::DatabaseConfig("AIoT device database requires SQLite engine".to_owned())
            })?;
            BlockingSqlitePool::from_pool(sqlite_pool.clone())
                .map(BlockingDevicePool::Sqlite)
                .map_err(PoolError::PoolCreation)
        }
        DatabaseEngine::Postgres => {
            let runtime = crate::runtime_bridge::shared_runtime()
                .map_err(|error| PoolError::DatabaseConfig(format!("tokio runtime: {error}")))?;
            let database_pool =
                crate::runtime_bridge::block_on(&runtime, create_pool_from_config(config))?;
            let postgres_pool = database_pool.as_postgres().ok_or_else(|| {
                PoolError::DatabaseConfig(
                    "AIoT device database requires Postgres engine".to_owned(),
                )
            })?;
            BlockingPostgresPool::from_pool(postgres_pool.clone())
                .map(BlockingDevicePool::Postgres)
                .map_err(PoolError::PoolCreation)
        }
    }
}

#[cfg(test)]
mod database_bootstrap_tests {
    use super::*;
    use crate::test_env::{lock_env_tests, EnvGuard, DEVICE_DATABASE_ENV_KEYS};

    #[test]
    fn resolve_device_database_config_defaults_to_workspace_postgres_without_env() {
        let _lock = lock_env_tests();
        let _guard = EnvGuard::clear(DEVICE_DATABASE_ENV_KEYS);

        let config = resolve_device_database_config_from_env(None).expect("config");
        assert_eq!(config.engine, DatabaseEngine::Postgres);
        assert!(config.url.contains("/sdkwork_ai_dev"));
        assert_eq!(config.table_prefix, "iot_");
    }

    #[test]
    fn resolve_device_database_config_from_env_accepts_postgres_engine() {
        let _lock = lock_env_tests();
        let _guard = EnvGuard::clear(DEVICE_DATABASE_ENV_KEYS);
        std::env::set_var("SDKWORK_DATABASE_ENGINE", "postgresql");
        std::env::set_var(
            "SDKWORK_DATABASE_URL",
            "postgres://sdkwork_ai_dev:test@localhost/sdkwork_ai_dev",
        );

        let config = resolve_device_database_config_from_env(None).expect("postgres config");
        assert_eq!(config.engine, DatabaseEngine::Postgres);
        assert_eq!(config.table_prefix, "iot_");
    }
}
