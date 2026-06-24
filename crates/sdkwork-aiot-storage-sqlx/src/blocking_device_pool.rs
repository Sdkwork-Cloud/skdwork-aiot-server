//! Engine-aware blocking pool used by synchronous AIoT repositories.

use std::future::Future;
use std::pin::Pin;

use sqlx::{PgConnection, Sqlite, SqliteConnection, Transaction};

use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::postgres_sync::{self, BlockingPostgresPool, StoragePostgresError};
use crate::sqlite_sync::{self, BlockingSqlitePool, StorageSqliteError};
use crate::{SqlDialect, SqlStatementBatch, SqlStatementPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceDatabaseEngine {
    Sqlite,
    Postgres,
}

impl DeviceDatabaseEngine {
    pub fn dialect(self) -> SqlDialect {
        match self {
            Self::Sqlite => SqlDialect::Sqlite,
            Self::Postgres => SqlDialect::Postgres,
        }
    }
}

pub enum DeviceDbTransaction<'c> {
    Sqlite(&'c mut SqliteConnection),
    Postgres(&'c mut PgConnection),
}

impl DeviceDbTransaction<'_> {
    pub fn sqlite_connection(&mut self) -> &mut SqliteConnection {
        match self {
            Self::Sqlite(connection) => connection,
            Self::Postgres(_) => panic!("expected sqlite device transaction"),
        }
    }

    pub fn postgres_connection(&mut self) -> &mut PgConnection {
        match self {
            Self::Postgres(connection) => connection,
            Self::Sqlite(_) => panic!("expected postgres device transaction"),
        }
    }

    pub async fn execute_plan(&mut self, statement: &SqlStatementPlan) -> Result<u64, sqlx::Error> {
        match self {
            Self::Sqlite(connection) => {
                sqlite_sync::execute_sql_plan(&mut **connection, statement).await
            }
            Self::Postgres(connection) => {
                postgres_sync::execute_sql_plan(&mut **connection, statement).await
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum BlockingDevicePool {
    Sqlite(BlockingSqlitePool),
    Postgres(BlockingPostgresPool),
}

impl BlockingDevicePool {
    pub fn engine(&self) -> DeviceDatabaseEngine {
        match self {
            Self::Sqlite(_) => DeviceDatabaseEngine::Sqlite,
            Self::Postgres(_) => DeviceDatabaseEngine::Postgres,
        }
    }

    pub fn dialect(&self) -> SqlDialect {
        self.engine().dialect()
    }

    pub fn adapt_sql(&self, sql: &str) -> String {
        adapt_sqlite_placeholders(self.dialect(), sql)
    }

    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        match self {
            Self::Sqlite(pool) => pool.block_on(future),
            Self::Postgres(pool) => pool.block_on(future),
        }
    }

    pub fn run<F, T, E>(&self, future: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        match self {
            Self::Sqlite(pool) => pool.run(future),
            Self::Postgres(pool) => pool.run(future),
        }
    }

    /// Runs an async operation with an owned pool clone to avoid borrow/move conflicts.
    pub fn run_owned<F, Fut, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: FnOnce(BlockingDevicePool) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let owned = self.clone();
        match self {
            Self::Sqlite(pool) => pool.run(operation(owned)),
            Self::Postgres(pool) => pool.run(operation(owned)),
        }
    }

    pub fn execute_batch_sql(&self, sql: &str) -> Result<(), sqlx::Error> {
        match self {
            Self::Sqlite(pool) => pool.execute_batch_sql(sql),
            Self::Postgres(pool) => pool.execute_batch_sql(sql),
        }
    }

    pub fn execute_statement_batch(&self, batch: SqlStatementBatch) -> Result<(), sqlx::Error> {
        match self {
            Self::Sqlite(pool) => pool.execute_statement_batch(batch),
            Self::Postgres(pool) => pool.execute_statement_batch(batch),
        }
    }

    pub fn sqlite_pool(&self) -> Option<&sqlx::SqlitePool> {
        match self {
            Self::Sqlite(pool) => Some(pool.pool()),
            Self::Postgres(_) => None,
        }
    }

    pub fn postgres_pool(&self) -> Option<&sqlx::PgPool> {
        match self {
            Self::Sqlite(_) => None,
            Self::Postgres(pool) => Some(pool.pool()),
        }
    }

    pub fn with_device_transaction<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        for<'c> F: FnOnce(
                DeviceDbTransaction<'c>,
                SqlDialect,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + 'c + Send>>
            + Send,
        E: From<StorageSqliteError> + From<StoragePostgresError>,
    {
        match self {
            Self::Sqlite(pool) => pool.with_transaction(|tx| {
                let dialect = SqlDialect::Sqlite;
                operation(DeviceDbTransaction::Sqlite(tx), dialect)
            }),
            Self::Postgres(pool) => pool.with_transaction(|tx| {
                let dialect = SqlDialect::Postgres;
                operation(DeviceDbTransaction::Postgres(tx), dialect)
            }),
        }
    }

    pub fn with_sqlite_transaction<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: for<'a> FnOnce(
                &'a mut Transaction<'_, Sqlite>,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + 'a + Send>>
            + Send,
        E: From<StorageSqliteError>,
    {
        match self {
            Self::Sqlite(pool) => pool.with_transaction(operation),
            Self::Postgres(_) => Err(StorageSqliteError::Configuration(
                "sqlite-only transaction requested on postgres device pool".into(),
            )
            .into()),
        }
    }
}
