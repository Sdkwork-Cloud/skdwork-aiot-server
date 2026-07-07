//! Single-pool bootstrap for AIoT device persistence surfaces.

use sdkwork_database_sqlx::PoolError;

use crate::blocking_device_pool::BlockingDevicePool;
use crate::schema::ensure_device_schema;
use crate::{
    aiot_device_blocking_pool_from_env, SqlitePersistedEntityRepository,
    SqliteSqlxCredentialRepository, SqliteSqlxDeviceRepository,
};

/// Shared device database pool backing device, credential, and admin-entity repositories.
#[derive(Debug, Clone)]
pub struct AiotDeviceDatabase {
    pool: BlockingDevicePool,
}

impl AiotDeviceDatabase {
    pub fn open(device_db_path: Option<&str>) -> Result<Self, PoolError> {
        let pool = aiot_device_blocking_pool_from_env(device_db_path)?;
        ensure_device_schema(&pool).map_err(PoolError::PoolCreation)?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: BlockingDevicePool) -> Result<Self, PoolError> {
        ensure_device_schema(&pool).map_err(PoolError::PoolCreation)?;
        Ok(Self { pool })
    }

    pub fn blocking_pool(&self) -> BlockingDevicePool {
        self.pool.clone()
    }

    pub fn engine(&self) -> crate::DeviceDatabaseEngine {
        self.pool.engine()
    }

    pub fn device_repository(&self) -> Result<SqliteSqlxDeviceRepository, sqlx::Error> {
        SqliteSqlxDeviceRepository::from_blocking_pool(self.pool.clone())
    }

    pub fn credential_repository(&self) -> Result<SqliteSqlxCredentialRepository, sqlx::Error> {
        SqliteSqlxCredentialRepository::from_blocking_pool(self.pool.clone())
    }

    pub fn persisted_entity_repository(
        &self,
    ) -> Result<SqlitePersistedEntityRepository, sqlx::Error> {
        SqlitePersistedEntityRepository::from_blocking_pool(self.pool.clone())
    }
}

/// Opens the canonical shared device database for HTTP services and gateway persistence.
pub fn open_aiot_device_database(
    device_db_path: Option<&str>,
) -> Result<AiotDeviceDatabase, PoolError> {
    AiotDeviceDatabase::open(device_db_path)
}

/// Opens the device database using path args, env path, explicit database env, or memory default.
pub fn open_aiot_device_database_from_env() -> Result<AiotDeviceDatabase, PoolError> {
    open_aiot_device_database(None)
}

#[cfg(test)]
mod device_database_tests {
    use super::*;
    use crate::sqlite_sync::BlockingSqlitePool;
    use crate::BlockingDevicePool;
    use sdkwork_aiot_storage::AiotDeviceRepository;

    #[test]
    fn open_from_sqlite_blocking_pool_bootstraps_schema() {
        let sqlite =
            BlockingSqlitePool::connect("file:aiot-device-db-test?mode=memory&cache=shared")
                .expect("connect");
        let database =
            AiotDeviceDatabase::from_pool(BlockingDevicePool::Sqlite(sqlite)).expect("database");
        assert!(database.device_repository().expect("repo").storage_ready());
    }

    /// Run with `SDKWORK_AIOT_POSTGRES_TEST_URL=postgres://... cargo test -p sdkwork-aiot-storage-sqlx postgres_device_database_round_trip -- --ignored`
    #[test]
    #[ignore = "requires SDKWORK_AIOT_POSTGRES_TEST_URL"]
    fn postgres_device_database_round_trip() {
        use sdkwork_aiot_storage::{
            AiotCommandCreateCommand, AiotCommandRepository, AiotDeviceCreateCommand,
            AiotDeviceRepository, AiotStorageAssociation, OffsetListPageParams,
        };

        let url = std::env::var("SDKWORK_AIOT_POSTGRES_TEST_URL")
            .expect("SDKWORK_AIOT_POSTGRES_TEST_URL must be set");
        std::env::set_var("SDKWORK_AIOT_DEVICE_DATABASE_URL", &url);
        std::env::set_var("SDKWORK_AIOT_DEVICE_DATABASE_ENGINE", "postgres");
        std::env::set_var("SDKWORK_AIOT_DEVICE_DATABASE_MODE", "pool");
        std::env::set_var("SDKWORK_AIOT_DEVICE_DATABASE_TABLE_PREFIX", "iot_");
        std::env::remove_var("SDKWORK_AIOT_DEVICE_DB_PATH");

        let database = open_aiot_device_database_from_env().expect("postgres database");
        assert_eq!(database.engine(), crate::DeviceDatabaseEngine::Postgres);
        let device_repo = database.device_repository().expect("device repo");
        assert!(device_repo.storage_ready());
        assert!(database.credential_repository().is_ok());

        let association = AiotStorageAssociation::tenant_org(900_001, 0);
        let device = device_repo
            .create_device(AiotDeviceCreateCommand::new(
                association.clone(),
                "pg-roundtrip-device",
                "Postgres Roundtrip",
                "product-001",
            ))
            .expect("create device");
        let listed = device_repo
            .list_devices(
                &association,
                OffsetListPageParams::parse(Some(1), Some(20)),
            )
            .expect("list devices");
        assert!(listed
            .items
            .iter()
            .any(|item| item.device_id == device.device_id));

        let command = device_repo
            .create_command(
                AiotCommandCreateCommand::new(
                    association.clone(),
                    device.device_id.clone(),
                    "system.ping",
                    "ping",
                )
                .with_idempotency_key("pg-roundtrip-idem")
                .with_request_payload_json(r#"{"text":"pg"}"#.to_string()),
            )
            .expect("create command");
        assert_eq!(
            device_repo
                .get_command(&association, &device.device_id, &command.command_id)
                .expect("get command")
                .map(|record| record.command_id),
            Some(command.command_id)
        );
    }
}
