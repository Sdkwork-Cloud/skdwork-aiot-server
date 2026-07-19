//! Synchronous facade over `sqlx::PgPool` for Postgres-backed device repositories.

use std::future::Future;
use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use tokio::runtime::Runtime;

use crate::{SqlBindValue, SqlStatementBatch, SqlStatementPlan};

pub type StoragePostgresError = sqlx::Error;

#[derive(Debug, Clone)]
pub struct BlockingPostgresPool {
    pool: PgPool,
    runtime: Arc<Runtime>,
}

impl BlockingPostgresPool {
    fn build_runtime() -> Result<Arc<Runtime>, StoragePostgresError> {
        crate::runtime_bridge::shared_runtime().map_err(|error| {
            StoragePostgresError::Configuration(format!("tokio runtime: {error}").into())
        })
    }

    pub fn from_pool(pool: PgPool) -> Result<Self, StoragePostgresError> {
        let runtime = Self::build_runtime()?;
        Ok(Self { pool, runtime })
    }

    pub fn connect(url: &str) -> Result<Self, StoragePostgresError> {
        let runtime = Self::build_runtime()?;
        let pool = crate::runtime_bridge::block_on(&runtime, PgPool::connect(url))?;
        Ok(Self { pool, runtime })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        crate::runtime_bridge::block_on(&self.runtime, future)
    }

    pub fn run<F, T, E>(&self, future: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        crate::runtime_bridge::block_on(&self.runtime, future)
    }

    pub fn execute_batch_sql(&self, sql: &str) -> Result<(), StoragePostgresError> {
        self.run(async {
            sqlx::raw_sql(sql).execute(&self.pool).await?;
            Ok(())
        })
    }

    pub fn with_transaction<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        for<'a> F: FnOnce(
            &'a mut Transaction<'_, Postgres>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<T, E>> + 'a + Send>>,
        E: From<StoragePostgresError>,
    {
        self.run(async {
            let mut tx = self.pool.begin().await?;
            let result = operation(&mut tx).await?;
            tx.commit().await?;
            Ok(result)
        })
    }

    pub fn execute_statement_batch(
        &self,
        batch: SqlStatementBatch,
    ) -> Result<(), StoragePostgresError> {
        self.with_transaction(|tx| {
            Box::pin(async move {
                let dialect = crate::SqlDialect::Postgres;
                for statement in batch.statements {
                    let mut device_tx =
                        crate::blocking_device_pool::DeviceDbTransaction::Postgres(tx);
                    let statement = crate::row_id_allocator::prepend_allocated_row_id_bind(
                        &mut device_tx,
                        dialect,
                        statement,
                    )
                    .await?;
                    device_tx.execute_plan(&statement).await?;
                }
                Ok(())
            })
        })
    }
}

pub fn bind_sql_plan<'q>(
    mut query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    statement: &'q SqlStatementPlan,
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    for bind in &statement.binds {
        query = match bind {
            SqlBindValue::Text(value) => query.bind(value),
            SqlBindValue::Int64(value) => query.bind(value),
            SqlBindValue::Null => query.bind(None::<String>),
        };
    }
    query
}

pub async fn execute_sql_plan<'e, E>(
    executor: E,
    statement: &SqlStatementPlan,
) -> Result<u64, StoragePostgresError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let query = bind_sql_plan(sqlx::query(&statement.sql), statement);
    Ok(query.execute(executor).await?.rows_affected())
}

#[allow(dead_code)]
pub fn read_timestamp_column(
    row: &sqlx::postgres::PgRow,
    index: &str,
) -> Result<String, sqlx::Error> {
    use sqlx::Row;
    row.try_get::<String, _>(index)
        .or_else(|_| row.try_get::<i64, _>(index).map(|value| value.to_string()))
}

#[allow(dead_code)]
pub fn read_optional_timestamp_column(
    row: &sqlx::postgres::PgRow,
    index: &str,
) -> Result<Option<String>, sqlx::Error> {
    use sqlx::Row;
    if row.try_get::<Option<String>, _>(index)?.is_none()
        && row.try_get::<Option<i64>, _>(index)?.is_none()
    {
        return Ok(None);
    }
    read_timestamp_column(row, index).map(Some)
}
