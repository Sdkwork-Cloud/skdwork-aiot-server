//! Monotonic row-id allocation for SQL tables (`API_SPEC` store-level identity).

use crate::blocking_device_pool::DeviceDbTransaction;
use crate::dialect_sql::adapt_sqlite_placeholders;
use crate::{SqlBindValue, SqlDialect, SqlStatementPlan};

/// Maps ingest/dead-letter statement kinds to allocator table keys.
pub fn row_id_allocator_table_for_statement(statement_kind: &str) -> Option<&'static str> {
    match statement_kind {
        "idempotency_guard" => Some("iot_protocol_ingest_record"),
        "outbox_write" => Some("iot_outbox_event"),
        "dead_letter_write" => Some("iot_protocol_message_dead_letter"),
        _ => None,
    }
}

/// Prepends a monotonic row `id` bind for INSERT statements that declare `id` as the first placeholder.
pub async fn prepend_allocated_row_id_bind(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    mut statement: SqlStatementPlan,
) -> Result<SqlStatementPlan, sqlx::Error> {
    let Some(table) = row_id_allocator_table_for_statement(statement.statement_kind) else {
        return Ok(statement);
    };
    let row_id = allocate_row_id(tx, dialect, table).await?;
    statement.binds.insert(0, SqlBindValue::Int64(row_id));
    Ok(statement)
}

/// Allocates the next monotonic `id` for `table_name` inside an open device transaction.
pub async fn allocate_row_id(
    tx: &mut DeviceDbTransaction<'_>,
    dialect: SqlDialect,
    table_name: &str,
) -> Result<i64, sqlx::Error> {
    let sql = adapt_sqlite_placeholders(
        dialect,
        "INSERT INTO iot_row_id_allocator (table_name, next_id)
         VALUES (?1, 1)
         ON CONFLICT(table_name) DO UPDATE SET next_id = iot_row_id_allocator.next_id + 1
         RETURNING next_id",
    );
    let table_name = table_name.to_string();
    match tx {
        DeviceDbTransaction::Sqlite(connection) => {
            sqlx::query_scalar(&sql)
                .bind(&table_name)
                .fetch_one(&mut **connection)
                .await
        }
        DeviceDbTransaction::Postgres(connection) => {
            sqlx::query_scalar(&sql)
                .bind(&table_name)
                .fetch_one(&mut **connection)
                .await
        }
    }
}
