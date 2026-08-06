//! Shared persistence bootstrap for AIoT HTTP services.

use std::sync::Arc;

use sdkwork_aiot_storage_sqlx::{
    open_aiot_device_database_from_env, resolve_device_database_config_from_env,
};
use sdkwork_database_config::DatabaseEngine;

use crate::{
    AiotCatalogRepositoryHandle, AiotCredentialRepository, AiotFirmwareRepositoryHandle,
    CredentialRepositoryAdapter,
};

pub struct AiotAppServiceStores {
    pub device_repository: Arc<sdkwork_aiot_storage_sqlx::SqlxDeviceRepository>,
    pub credential_repository: Arc<dyn AiotCredentialRepository>,
    pub catalog_repository: Arc<AiotCatalogRepositoryHandle>,
}

pub struct AiotAdminServiceStores {
    pub device_repository: Arc<sdkwork_aiot_storage_sqlx::SqlxDeviceRepository>,
    pub credential_repository: Arc<dyn AiotCredentialRepository>,
    pub catalog_repository: Arc<AiotCatalogRepositoryHandle>,
    pub firmware_repository: Arc<AiotFirmwareRepositoryHandle>,
}

pub fn open_app_service_stores(service_label: &str) -> Result<AiotAppServiceStores, String> {
    log_device_database_target(service_label);
    let database = open_aiot_device_database_from_env().map_err(|error| error.to_string())?;
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
        credential_repository: Arc::new(CredentialRepositoryAdapter::from_repository(
            database
                .credential_repository()
                .map_err(|error| error.to_string())?,
        )),
        catalog_repository: Arc::new(AiotCatalogRepositoryHandle::from_entity_store(entity_store)),
    })
}

pub fn open_admin_service_stores(service_label: &str) -> Result<AiotAdminServiceStores, String> {
    log_device_database_target(service_label);
    let database = open_aiot_device_database_from_env().map_err(|error| error.to_string())?;
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
        credential_repository: Arc::new(CredentialRepositoryAdapter::from_repository(
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

fn log_device_database_target(service_label: &str) {
    match resolve_device_database_config_from_env(None) {
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
