//! SDKWork AIoT database lifecycle bootstrap exports.

pub use sdkwork_aiot_database_host::{
    bootstrap_aiot_database, bootstrap_aiot_database_from_env,
    resolve_aiot_database_config_from_env, AiotDatabaseHost,
};

use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool, PoolError};

pub type AiotDatabasePool = DatabasePool;

pub async fn connect_aiot_database_pool_from_env() -> Result<AiotDatabasePool, PoolError> {
    let config = resolve_aiot_database_config_from_env()
        .map_err(|error| PoolError::DatabaseConfig(error.to_string()))?;
    create_pool_from_config(config).await
}

pub async fn connect_and_bootstrap_aiot_database_from_env() -> Result<AiotDatabaseHost, String> {
    let pool = connect_aiot_database_pool_from_env()
        .await
        .map_err(|error| error.to_string())?;
    bootstrap_aiot_database(pool).await
}
