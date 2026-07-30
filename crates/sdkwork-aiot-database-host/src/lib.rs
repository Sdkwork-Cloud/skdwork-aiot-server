use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_database_config::{ConfigError, DatabaseConfig};
use sdkwork_database_lifecycle::{lifecycle_options_from_env, LifecycleOrchestrator};
use sdkwork_database_spi::{DatabaseAssetProvider, DatabaseManifest, DefaultDatabaseModule};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool};

pub struct AiotDatabaseHost {
    pool: DatabasePool,
    module: Arc<DefaultDatabaseModule>,
}

pub fn resolve_aiot_database_config_from_env() -> Result<DatabaseConfig, ConfigError> {
    let mut config = DatabaseConfig::from_env("AIOT_DEVICE")?;
    config.table_prefix = "iot_".to_owned();
    Ok(config)
}

impl AiotDatabaseHost {
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    pub fn module(&self) -> Arc<DefaultDatabaseModule> {
        self.module.clone()
    }
}

pub async fn bootstrap_aiot_database(pool: DatabasePool) -> Result<AiotDatabaseHost, String> {
    let app_root = resolve_app_root();
    let module = Arc::new(
        DefaultDatabaseModule::from_app_root(&app_root)
            .map_err(|error| format!("load aiot database module failed: {error}"))?,
    );
    let manifest = DatabaseManifest::from_file(module.manifest_path())
        .map_err(|error| format!("read aiot database manifest failed: {error}"))?;
    let options = lifecycle_options_from_env("AIOT_DEVICE", &manifest);
    let orchestrator =
        LifecycleOrchestrator::new(pool.clone(), module.clone()).with_applied_by("sdkwork-aiot");

    orchestrator
        .init()
        .await
        .map_err(|error| format!("aiot database init failed: {error}"))?;

    if options.auto_migrate {
        orchestrator
            .migrate()
            .await
            .map_err(|error| format!("aiot database migrate failed: {error}"))?;
    }

    Ok(AiotDatabaseHost { pool, module })
}

pub async fn bootstrap_aiot_database_from_env() -> Result<AiotDatabaseHost, String> {
    let _ = dotenvy::dotenv();
    let config = resolve_aiot_database_config_from_env()
        .map_err(|error| format!("read aiot database config failed: {error}"))?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| format!("create aiot database pool failed: {error}"))?;
    bootstrap_aiot_database(pool).await
}

fn resolve_app_root() -> PathBuf {
    std::env::var("SDKWORK_AIOT_DEVICE_APP_ROOT")
        .or_else(|_| std::env::var("SDKWORK_AIOT_APP_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        })
}
