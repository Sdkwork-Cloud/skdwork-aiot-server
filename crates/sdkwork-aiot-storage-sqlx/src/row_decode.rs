//! Shared SQL row decoding helpers for dialect-aware repositories.

use sqlx::{postgres::PgRow, sqlite::SqliteRow, Row};

pub fn read_sqlite_timestamp(row: &SqliteRow, column: &'static str) -> Result<String, sqlx::Error> {
    row.try_get::<String, _>(column).or_else(|_| {
        let value: i64 = row.try_get(column)?;
        Ok(value.to_string())
    })
}

pub fn read_sqlite_optional_timestamp(
    row: &SqliteRow,
    column: &'static str,
) -> Result<Option<String>, sqlx::Error> {
    if row.try_get::<Option<String>, _>(column)?.is_none()
        && row.try_get::<Option<i64>, _>(column)?.is_none()
    {
        return Ok(None);
    }
    read_sqlite_timestamp(row, column).map(Some)
}

pub fn read_postgres_timestamp(row: &PgRow, column: &'static str) -> Result<String, sqlx::Error> {
    if let Ok(value) = row.try_get::<String, _>(column) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get::<i64, _>(column) {
        return Ok(value.to_string());
    }
    let value: chrono::NaiveDateTime = row.try_get(column)?;
    Ok(value.format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub fn read_postgres_optional_timestamp(
    row: &PgRow,
    column: &'static str,
) -> Result<Option<String>, sqlx::Error> {
    if row.try_get::<Option<String>, _>(column)?.is_some() {
        return read_postgres_timestamp(row, column).map(Some);
    }
    if row.try_get::<Option<i64>, _>(column)?.is_some() {
        return read_postgres_timestamp(row, column).map(Some);
    }
    if row.try_get::<Option<chrono::NaiveDateTime>, _>(column)?.is_some() {
        return read_postgres_timestamp(row, column).map(Some);
    }
    Ok(None)
}
