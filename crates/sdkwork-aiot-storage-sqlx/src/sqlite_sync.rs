//! Synchronous facade over `sqlx::SqlitePool` for legacy sync repository traits.

use std::future::Future;
use std::sync::Arc;

use sqlx::{Sqlite, SqlitePool, Transaction};
use tokio::runtime::Runtime;

use crate::{SqlBindValue, SqlStatementBatch, SqlStatementPlan};

pub type StorageSqliteError = sqlx::Error;

#[derive(Debug, Clone)]
pub struct BlockingSqlitePool {
    pool: SqlitePool,
    runtime: Arc<Runtime>,
}

impl BlockingSqlitePool {
    fn build_runtime() -> Result<Arc<Runtime>, StorageSqliteError> {
        Ok(Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    StorageSqliteError::Configuration(format!("tokio runtime: {error}").into())
                })?,
        ))
    }

    pub fn from_pool(pool: SqlitePool) -> Result<Self, StorageSqliteError> {
        let runtime = Self::build_runtime()?;
        Ok(Self { pool, runtime })
    }

    pub fn connect(url: &str) -> Result<Self, StorageSqliteError> {
        let runtime = Self::build_runtime()?;
        let pool = runtime.block_on(SqlitePool::connect(url))?;
        Ok(Self { pool, runtime })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        self.runtime.block_on(future)
    }

    pub fn run<F, T, E>(&self, future: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        self.runtime.block_on(future)
    }

    pub fn execute_batch_sql(&self, sql: &str) -> Result<(), StorageSqliteError> {
        self.run(async {
            sqlx::raw_sql(sql).execute(&self.pool).await?;
            Ok(())
        })
    }

    pub fn with_transaction<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: for<'a> FnOnce(
            &'a mut Transaction<'_, Sqlite>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<T, E>> + 'a + Send>>,
        E: From<StorageSqliteError>,
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
    ) -> Result<(), StorageSqliteError> {
        self.with_transaction(|tx| {
            Box::pin(async move {
                for statement in batch.statements {
                    execute_sql_plan(&mut **tx, &statement).await?;
                }
                Ok(())
            })
        })
    }
}

pub fn sqlite_connect_url(path_or_uri: impl AsRef<str>) -> String {
    let value = path_or_uri.as_ref();
    if value.starts_with("file:") || value.starts_with("sqlite:") {
        value.to_string()
    } else {
        format!("sqlite:{}?mode=rwc", value.replace('\\', "/"))
    }
}

pub fn bind_sql_plan<'q>(
    mut query: sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    statement: &'q SqlStatementPlan,
) -> sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
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
) -> Result<u64, StorageSqliteError>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let query = bind_sql_plan(sqlx::query(&statement.sql), statement);
    Ok(query.execute(executor).await?.rows_affected())
}

#[allow(dead_code)]
pub fn read_timestamp_column(
    row: &sqlx::sqlite::SqliteRow,
    index: &str,
) -> Result<String, sqlx::Error> {
    use sqlx::Row;
    row.try_get::<String, _>(index)
        .or_else(|_| row.try_get::<i64, _>(index).map(|value| value.to_string()))
}

#[allow(dead_code)]
pub fn read_optional_timestamp_column(
    row: &sqlx::sqlite::SqliteRow,
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
