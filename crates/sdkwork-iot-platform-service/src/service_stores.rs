//! Shared persistence bootstrap for AIoT HTTP services.

use std::path::Path;
use std::sync::Arc;

use sdkwork_aiot_storage_sqlx::{
    open_aiot_device_database, resolve_device_database_config_from_env,
    DEFAULT_SHARED_SQLITE_MEMORY_URI,
};
use sdkwork_database_config::DatabaseEngine;

use crate::{
    AiotCatalogRepositoryHandle, AiotCredentialRepository, AiotFirmwareRepositoryHandle,
    SqliteCredentialRepositoryAdapter,
};

pub struct AiotAppServiceStores {
    pub device_repository: Arc<sdkwork_aiot_storage_sqlx::SqliteSqlxDeviceRepository>,
    pub credential_repository: Arc<dyn AiotCredentialRepository>,
    pub catalog_repository: Arc<AiotCatalogRepositoryHandle>,
}

pub struct AiotAdminServiceStores {
    pub device_repository: Arc<sdkwork_aiot_storage_sqlx::SqliteSqlxDeviceRepository>,
    pub credential_repository: Arc<dyn AiotCredentialRepository>,
    pub catalog_repository: Arc<AiotCatalogRepositoryHandle>,
    pub firmware_repository: Arc<AiotFirmwareRepositoryHandle>,
}

pub fn configured_device_db_path(service_env_key: &str) -> Option<String> {
    std::env::var(service_env_key)
        .ok()
        .or_else(|| std::env::var("SDKWORK_AIOT_DEVICE_DB_PATH").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn open_app_service_stores(
    device_db_path: Option<&str>,
    service_label: &str,
) -> Result<AiotAppServiceStores, String> {
    log_device_database_target(device_db_path, service_label);
    if let Some(path) = device_db_path {
        ensure_parent_directory_exists(path);
    }
    let database = open_aiot_device_database(device_db_path).map_err(|error| error.to_string())?;
    let entity_store = Arc::new(
        database
            .persisted_entity_repository()
            .map_err(|error| error.to_string())?,
    );

    Ok(AiotAppServiceStores {
        device_repository: Arc::new(
            database
                .device_repository()
                .map_err(|error| error.to_string())?,
        ),
        credential_repository: Arc::new(SqliteCredentialRepositoryAdapter::from_repository(
            database
                .credential_repository()
                .map_err(|error| error.to_string())?,
        )),
        catalog_repository: Arc::new(AiotCatalogRepositoryHandle::from_entity_store(entity_store)),
    })
}

pub fn open_admin_service_stores(
    device_db_path: Option<&str>,
    service_label: &str,
) -> Result<AiotAdminServiceStores, String> {
    log_device_database_target(device_db_path, service_label);
    if let Some(path) = device_db_path {
        ensure_parent_directory_exists(path);
    }
    let database = open_aiot_device_database(device_db_path).map_err(|error| error.to_string())?;
    let entity_store = Arc::new(
        database
            .persisted_entity_repository()
            .map_err(|error| error.to_string())?,
    );

    Ok(AiotAdminServiceStores {
        device_repository: Arc::new(
            database
                .device_repository()
                .map_err(|error| error.to_string())?,
        ),
        credential_repository: Arc::new(SqliteCredentialRepositoryAdapter::from_repository(
            database
                .credential_repository()
                .map_err(|error| error.to_string())?,
        )),
        catalog_repository: Arc::new(AiotCatalogRepositoryHandle::from_entity_store(
            entity_store.clone(),
        )),
        firmware_repository: Arc::new(AiotFirmwareRepositoryHandle::from_entity_store(
            entity_store,
        )),
    })
}

fn log_device_database_target(device_db_path: Option<&str>, service_label: &str) {
    match resolve_device_database_config_from_env(device_db_path) {
        Ok(config) if config.url.contains("mode=memory") => {
            println!("{service_label} device-db=sqlite mode=memory uri={DEFAULT_SHARED_SQLITE_MEMORY_URI}");
        }
        Ok(config) => {
            let engine = match config.engine {
                DatabaseEngine::Sqlite => "sqlite",
                DatabaseEngine::Postgres => "postgres",
            };
            println!("{service_label} device-db={engine} configured");
        }
        Err(error) => {
            eprintln!("{service_label} device-db=error={error}");
        }
    }
}

fn ensure_parent_directory_exists(path: &str) {
    let parent = Path::new(path).parent();
    if let Some(parent) = parent {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).expect("create sqlite parent directory");
        }
    }
}
