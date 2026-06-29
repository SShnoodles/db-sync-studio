use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde_json::{Number, Value as JsonValue};
use std::collections::BTreeMap;

use super::{ColumnMeta, DbConnection, TableMeta};

fn connection(config: &DbConnection) -> Result<Connection, String> {
    config.validate()?;
    Connection::open_with_flags(
        config.database.trim(),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Unable to open SQLite database: {error}"))
}

pub fn test_connection(config: &DbConnection) -> Result<String, String> {
    let connection = connection(config)?;
    let version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(|error| format!("Connection test failed: {error}"))?;
    Ok(format!("Connected to SQLite {version}"))
}

pub fn list_tables(config: &DbConnection) -> Result<Vec<TableMeta>, String> {
    let connection = connection(config)?;
    let mut statement = connection
        .prepare(
            "SELECT name,
                CASE type WHEN 'table' THEN 'BASE TABLE' WHEN 'view' THEN 'VIEW' ELSE upper(type) END
             FROM sqlite_schema
             WHERE type IN ('table', 'view')
               AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .map_err(|error| format!("Unable to load SQLite tables: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(TableMeta {
                name: row.get(0)?,
                schema: None,
                table_type: row.get(1)?,
                comment: None,
            })
        })
        .map_err(|error| format!("Unable to load SQLite tables: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Unable to load SQLite tables: {error}"))?;
    Ok(rows)
}

pub fn list_columns(config: &DbConnection, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let connection = connection(config)?;
    let sql = format!("PRAGMA table_info({})", quote_identifier(table));
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Unable to load columns for {table}: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let cid: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let column_type: String = row.get::<_, String>(2)?.trim().to_string();
            let not_null: i64 = row.get(3)?;
            let default_value: Option<String> = row.get(4)?;
            let pk_position: i64 = row.get(5)?;
            Ok(ColumnMeta {
                table_name: table.into(),
                name,
                column_type: if column_type.is_empty() {
                    "BLOB".into()
                } else {
                    column_type
                },
                nullable: not_null == 0 && pk_position == 0,
                default_value,
                is_primary_key: pk_position > 0,
                extra: None,
                ordinal_position: (cid + 1) as u64,
                comment: None,
                spatial_srid: None,
            })
        })
        .map_err(|error| format!("Unable to load columns for {table}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Unable to load columns for {table}: {error}"))?;
    Ok(rows)
}

pub fn show_create_table(config: &DbConnection, table: &str) -> Result<String, String> {
    let connection = connection(config)?;
    let sql: String = connection
        .query_row(
            "SELECT sql
             FROM sqlite_schema
             WHERE type IN ('table', 'view') AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| format!("Unable to read CREATE TABLE for {table}: {error}"))?;
    Ok(if sql.trim_end().ends_with(';') {
        sql
    } else {
        format!("{sql};")
    })
}

pub fn primary_keys(config: &DbConnection, table: &str) -> Result<Vec<String>, String> {
    let mut columns = list_columns(config, table)?
        .into_iter()
        .filter(|column| column.is_primary_key)
        .collect::<Vec<_>>();
    columns.sort_by_key(|column| column.ordinal_position);
    Ok(columns.into_iter().map(|column| column.name).collect())
}

pub fn fetch_rows(
    config: &DbConnection,
    table: &str,
    order_columns: &[String],
    limit: usize,
) -> Result<Vec<BTreeMap<String, JsonValue>>, String> {
    let connection = connection(config)?;
    let order_by = if order_columns.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            order_columns
                .iter()
                .map(|column| quote_identifier(column))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let sql = format!(
        "SELECT * FROM {}{order_by} LIMIT {limit}",
        quote_identifier(table)
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Unable to fetch rows from {table}: {error}"))?;
    let names = statement
        .column_names()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let rows = statement
        .query_map([], |row| {
            let mut item = BTreeMap::new();
            for (index, name) in names.iter().enumerate() {
                item.insert(name.clone(), sqlite_value_to_json(row.get_ref(index)?));
            }
            Ok(item)
        })
        .map_err(|error| format!("Unable to fetch rows from {table}: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Unable to fetch rows from {table}: {error}"))
}

pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> JsonValue {
    match value {
        ValueRef::Null => JsonValue::Null,
        ValueRef::Integer(value) => JsonValue::Number(Number::from(value)),
        ValueRef::Real(value) => Number::from_f64(value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        ValueRef::Text(value) => JsonValue::String(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => JsonValue::String(bytes_to_hex(value)),
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join("")
}
