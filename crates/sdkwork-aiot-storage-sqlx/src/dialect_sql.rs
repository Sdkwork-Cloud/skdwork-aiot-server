use crate::SqlDialect;

/// Returns the positional placeholder for the active SQL dialect (`?1` vs `$1`).
#[allow(dead_code)]
pub fn positional_placeholder(dialect: SqlDialect, index: usize) -> String {
    match dialect {
        SqlDialect::Sqlite => format!("?{index}"),
        SqlDialect::Postgres => format!("${index}"),
    }
}

/// Adapts SQLite-style `?N` placeholders to Postgres `$N` placeholders.
pub fn adapt_sqlite_placeholders(dialect: SqlDialect, sql: &str) -> String {
    if dialect == SqlDialect::Sqlite {
        return sql.to_owned();
    }

    let mut adapted = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '?' {
            adapted.push(ch);
            continue;
        }

        let mut index = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_digit() {
                index.push(chars.next().expect("digit"));
            } else {
                break;
            }
        }

        if index.is_empty() {
            adapted.push('?');
            continue;
        }

        adapted.push('$');
        adapted.push_str(&index);
    }

    adapted
}

#[cfg(test)]
mod dialect_sql_tests {
    use super::*;

    #[test]
    fn adapt_sqlite_placeholders_rewrites_numbered_markers_for_postgres() {
        let sql = "SELECT * FROM iot_device WHERE tenant_id = ?1 AND device_id = ?2";
        assert_eq!(
            adapt_sqlite_placeholders(SqlDialect::Postgres, sql),
            "SELECT * FROM iot_device WHERE tenant_id = $1 AND device_id = $2"
        );
    }

    #[test]
    fn adapt_sqlite_placeholders_preserves_sqlite_markers() {
        let sql = "SELECT * FROM iot_device WHERE tenant_id = ?1";
        assert_eq!(adapt_sqlite_placeholders(SqlDialect::Sqlite, sql), sql);
    }
}
