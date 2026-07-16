use mysql::{prelude::Queryable, OptsBuilder, Pool, Row, TxOpts, Value};
use serde_json::{Number, Value as JsonValue};
use std::collections::{BTreeMap, HashMap};

use super::{ColumnMeta, DbConnection, TableMeta};

fn pool(connection: &DbConnection) -> Result<Pool, String> {
    connection.validate()?;
    let options = OptsBuilder::new()
        .ip_or_hostname(Some(connection.host.clone().unwrap_or_default()))
        .tcp_port(connection.port.unwrap_or(3306))
        .db_name(Some(connection.database.clone()))
        .user(connection.username.clone())
        .pass(connection.password.clone());
    Pool::new(options).map_err(|error| format!("Unable to create MySQL connection: {error}"))
}

pub fn test_connection(connection: &DbConnection) -> Result<String, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let version: Option<String> = conn
        .query_first("SELECT VERSION()")
        .map_err(|error| format!("Connection test failed: {error}"))?;
    Ok(format!(
        "Connected to MySQL {}",
        version.unwrap_or_else(|| "server".into())
    ))
}

pub fn list_tables(connection: &DbConnection) -> Result<Vec<TableMeta>, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let rows: Vec<(String, String)> = conn.exec(
        "SELECT table_name, table_type FROM information_schema.tables WHERE table_schema = DATABASE() ORDER BY table_name",
        (),
    ).map_err(|error| format!("Unable to load tables: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|(name, table_type)| TableMeta {
            name,
            schema: Some(connection.database.clone()),
            table_type,
            comment: None,
        })
        .collect())
}

pub fn list_columns(connection: &DbConnection, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    conn.exec_map(
        "SELECT table_name, column_name, column_type, is_nullable, column_default, column_key, extra, ordinal_position, srs_id
         FROM information_schema.columns
         WHERE table_schema = DATABASE() AND table_name = ?
         ORDER BY ordinal_position",
        (table,),
        |(
            table_name,
            name,
            column_type,
            is_nullable,
            default_value,
            column_key,
            extra,
            ordinal_position,
            spatial_srid,
        ): (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            u64,
            Option<u32>,
        )| {
            ColumnMeta {
                table_name,
                name,
                column_type,
                nullable: is_nullable == "YES",
                default_value,
                is_primary_key: column_key == "PRI",
                extra: if extra.is_empty() { None } else { Some(extra) },
                ordinal_position,
                comment: None,
                spatial_srid,
            }
        },
    )
    .map_err(|error| format!("Unable to load columns for {table}: {error}"))
}

pub fn show_create_table(connection: &DbConnection, table: &str) -> Result<String, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let escaped_table = escape_identifier(table);
    let row: Option<(String, String)> = conn
        .query_first(format!("SHOW CREATE TABLE `{escaped_table}`"))
        .map_err(|error| format!("Unable to read CREATE TABLE for {table}: {error}"))?;
    row.map(|(_, create_sql)| format!("{create_sql};"))
        .ok_or_else(|| format!("CREATE TABLE statement was not found for {table}"))
}

pub fn show_create_view(connection: &DbConnection, view: &str) -> Result<String, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let definition: Option<String> = conn
        .exec_first(
            "SELECT view_definition FROM information_schema.views WHERE table_schema = DATABASE() AND table_name = ?",
            (view,),
        )
        .map_err(|error| format!("Unable to read CREATE VIEW for {view}: {error}"))?;
    definition
        .map(|definition| {
            format!(
                "CREATE OR REPLACE VIEW `{}` AS {};",
                escape_identifier(view),
                definition.trim().trim_end_matches(';')
            )
        })
        .ok_or_else(|| format!("View definition was not found for {view}"))
}

pub fn primary_keys(connection: &DbConnection, table: &str) -> Result<Vec<String>, String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    conn.exec_map(
        "SELECT column_name
         FROM information_schema.key_column_usage
         WHERE table_schema = DATABASE() AND table_name = ? AND constraint_name = 'PRIMARY'
         ORDER BY ordinal_position",
        (table,),
        |column_name: String| column_name,
    )
    .map_err(|error| format!("Unable to load primary keys for {table}: {error}"))
}

pub fn fetch_rows(
    connection: &DbConnection,
    table: &str,
    order_columns: &[String],
    limit: usize,
    offset: usize,
) -> Result<Vec<BTreeMap<String, JsonValue>>, String> {
    let columns = list_columns(connection, table)?;
    let column_types = columns
        .iter()
        .map(|column| (column.name.clone(), column.column_type.clone()))
        .collect::<HashMap<_, _>>();
    let select_list = columns
        .into_iter()
        .map(|column| {
            let escaped_name = escape_identifier(&column.name);
            if is_spatial_type(&column.column_type) {
                format!("ST_AsWKB(`{escaped_name}`) AS `{escaped_name}`")
            } else {
                format!("`{escaped_name}`")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let escaped_table = escape_identifier(table);
    let order_by = if order_columns.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            order_columns
                .iter()
                .map(|column| format!("`{}`", escape_identifier(column)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let sql = format!(
        "SELECT {select_list} FROM `{escaped_table}`{order_by} LIMIT {limit} OFFSET {offset}"
    );
    let rows: Vec<Row> = conn
        .query(sql)
        .map_err(|error| format!("Unable to fetch rows from {table}: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|row| row_to_map(row, &column_types))
        .collect())
}

pub fn execute_schema_statements(
    connection: &DbConnection,
    statements: &[String],
) -> Result<(), String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    for statement in statements {
        conn.query_drop(statement)
            .map_err(|error| format!("Unable to execute MySQL schema SQL: {error}\n{statement}"))?;
    }
    Ok(())
}

pub fn execute_data_statements(
    connection: &DbConnection,
    statements: &[String],
) -> Result<(), String> {
    let pool = pool(connection)?;
    let mut conn = pool
        .get_conn()
        .map_err(|error| format!("Connection failed: {error}"))?;
    let mut transaction = conn
        .start_transaction(TxOpts::default())
        .map_err(|error| format!("Unable to start MySQL data transaction: {error}"))?;
    for statement in statements {
        if let Err(error) = transaction.query_drop(statement) {
            let rollback_error = transaction.rollback().err();
            return Err(format_transaction_error(
                "MySQL",
                error,
                rollback_error,
                statement,
            ));
        }
    }
    transaction
        .commit()
        .map_err(|error| format!("Unable to commit MySQL data transaction: {error}"))
}

fn format_transaction_error(
    database: &str,
    error: impl std::fmt::Display,
    rollback_error: Option<impl std::fmt::Display>,
    statement: &str,
) -> String {
    let rollback_detail = rollback_error
        .map(|error| format!(" Rollback also failed: {error}."))
        .unwrap_or_default();
    format!("Unable to execute {database} data SQL: {error}.{rollback_detail}\n{statement}")
}

fn escape_identifier(value: &str) -> String {
    value.replace('`', "``")
}

fn row_to_map(row: Row, column_types: &HashMap<String, String>) -> BTreeMap<String, JsonValue> {
    let names = row
        .columns_ref()
        .iter()
        .map(|column| column.name_str().to_string())
        .collect::<Vec<_>>();
    names
        .into_iter()
        .zip(row.unwrap())
        .map(|(name, value)| {
            let json_value =
                mysql_value_to_json(value, column_types.get(&name).map(String::as_str));
            (name, json_value)
        })
        .collect()
}

fn mysql_value_to_json(value: Value, column_type: Option<&str>) -> JsonValue {
    match value {
        Value::NULL => JsonValue::Null,
        Value::Bytes(bytes) => {
            if column_type.is_some_and(is_binary_json_or_spatial_type) {
                JsonValue::String(bytes_to_hex(&bytes))
            } else {
                JsonValue::String(String::from_utf8_lossy(&bytes).to_string())
            }
        }
        Value::Int(value) => JsonValue::Number(Number::from(value)),
        Value::UInt(value) => JsonValue::Number(Number::from(value)),
        Value::Float(value) => Number::from_f64(value as f64)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Double(value) => Number::from_f64(value)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Value::Date(year, month, day, hour, minute, second, micros) => JsonValue::String(format!(
            "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{:06}",
            micros
        )),
        Value::Time(is_negative, days, hours, minutes, seconds, micros) => {
            JsonValue::String(format!(
                "{}{} {:02}:{:02}:{:02}.{:06}",
                if is_negative { "-" } else { "" },
                days,
                hours,
                minutes,
                seconds,
                micros
            ))
        }
    }
}

fn is_binary_json_or_spatial_type(column_type: &str) -> bool {
    let lower = column_type.to_ascii_lowercase();
    lower.starts_with("bit(")
        || lower.starts_with("binary(")
        || lower.starts_with("varbinary(")
        || matches!(
            lower.as_str(),
            "tinyblob" | "blob" | "mediumblob" | "longblob"
        )
        || is_spatial_type(&lower)
}

fn is_spatial_type(column_type: &str) -> bool {
    matches!(
        column_type.to_ascii_lowercase().as_str(),
        "geometry"
            | "point"
            | "linestring"
            | "polygon"
            | "multipoint"
            | "multilinestring"
            | "multipolygon"
            | "geometrycollection"
    )
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join("")
}
