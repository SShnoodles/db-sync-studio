use rusqlite::{types::ValueRef, Connection, OpenFlags, OptionalExtension};
use serde_json::{Number, Value as JsonValue};
use std::collections::BTreeMap;

use super::{ColumnMeta, DbConnection, ForeignKeyMeta, IndexMeta, TableMeta};

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
    let sql = format!("PRAGMA table_xinfo({})", quote_identifier(table));
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
            let hidden: i64 = row.get(6)?;
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
                extra: (hidden >= 2).then(|| "generated".into()),
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

pub fn show_create_view(config: &DbConnection, view: &str) -> Result<String, String> {
    let connection = connection(config)?;
    let sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'view' AND name = ?1",
            [view],
            |row| row.get(0),
        )
        .map_err(|error| format!("Unable to read CREATE VIEW for {view}: {error}"))?;
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

pub fn list_indexes(config: &DbConnection, table: &str) -> Result<Vec<IndexMeta>, String> {
    let connection = connection(config)?;
    let sql = format!("PRAGMA index_list({})", quote_identifier(table));
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Unable to load indexes for {table}: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("Unable to load indexes for {table}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Unable to load indexes for {table}: {error}"))?;
    drop(statement);

    rows.into_iter()
        .map(|(name, unique, origin)| {
            let info_sql = format!("PRAGMA index_info({})", quote_identifier(&name));
            let mut info_statement = connection
                .prepare(&info_sql)
                .map_err(|error| format!("Unable to load index {name}: {error}"))?;
            let columns = info_statement
                .query_map([], |row| row.get::<_, String>(2))
                .map_err(|error| format!("Unable to load index {name}: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Unable to load index {name}: {error}"))?;
            let definition = connection
                .query_row(
                    "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                    [&name],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(|error| format!("Unable to load index {name}: {error}"))?
                .flatten()
                .map(|sql| format!("{};", sql.trim_end_matches(';')));
            Ok(IndexMeta {
                name,
                columns,
                unique,
                primary: origin == "pk",
                definition,
            })
        })
        .collect()
}

pub fn list_foreign_keys(
    config: &DbConnection,
    table: &str,
) -> Result<Vec<ForeignKeyMeta>, String> {
    let connection = connection(config)?;
    let sql = format!("PRAGMA foreign_key_list({})", quote_identifier(table));
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("Unable to load foreign keys for {table}: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|error| format!("Unable to load foreign keys for {table}: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Unable to load foreign keys for {table}: {error}"))?;
    let mut keys = Vec::<ForeignKeyMeta>::new();
    for (id, referenced_table, column, referenced_column, on_update, on_delete) in rows {
        let name = format!("foreign_key_{id}");
        if let Some(key) = keys.iter_mut().find(|key| key.name == name) {
            key.columns.push(column);
            key.referenced_columns.push(referenced_column);
        } else {
            keys.push(ForeignKeyMeta {
                name,
                columns: vec![column],
                referenced_table,
                referenced_columns: vec![referenced_column],
                on_update: Some(on_update),
                on_delete: Some(on_delete),
            });
        }
    }
    Ok(keys)
}

pub fn fetch_rows(
    config: &DbConnection,
    table: &str,
    order_columns: &[String],
    limit: usize,
    offset: usize,
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
        "SELECT * FROM {}{order_by} LIMIT {limit} OFFSET {offset}",
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

pub fn execute_schema_statements(
    config: &DbConnection,
    statements: &[String],
) -> Result<(), String> {
    let connection = connection(config)?;
    for statement in statements {
        connection.execute_batch(statement).map_err(|error| {
            format!("Unable to execute SQLite schema SQL: {error}\n{statement}")
        })?;
    }
    Ok(())
}

pub fn execute_data_statements(config: &DbConnection, statements: &[String]) -> Result<(), String> {
    let mut connection = connection(config)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Unable to start SQLite data transaction: {error}"))?;
    for statement in statements {
        if let Err(error) = transaction.execute_batch(statement) {
            let rollback_error = transaction.rollback().err();
            let rollback_detail = rollback_error
                .map(|error| format!(" Rollback also failed: {error}."))
                .unwrap_or_default();
            return Err(format!(
                "Unable to execute SQLite data SQL: {error}.{rollback_detail}\n{statement}"
            ));
        }
    }
    transaction
        .commit()
        .map_err(|error| format!("Unable to commit SQLite data transaction: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_double_quotes_in_identifiers() {
        assert_eq!(quote_identifier("order\"detail"), "\"order\"\"detail\"");
    }

    #[test]
    fn data_statements_roll_back_the_entire_batch_on_failure() {
        let path = std::env::temp_dir().join(format!(
            "db-sync-studio-transaction-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        Connection::open(&path)
            .unwrap()
            .execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", [])
            .unwrap();
        let config = sqlite_test_connection(path.to_string_lossy().into_owned());

        let result = execute_data_statements(
            &config,
            &[
                "INSERT INTO items (id, name) VALUES (1, 'first');".into(),
                "INSERT INTO missing_table (id) VALUES (2);".into(),
            ],
        );

        assert!(result.is_err());
        let row_count: i64 = Connection::open(&path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn loads_indexes_and_foreign_keys() {
        let path = std::env::temp_dir().join(format!(
            "db-sync-studio-schema-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE parents (id INTEGER PRIMARY KEY);
                 CREATE TABLE children (
                   id INTEGER PRIMARY KEY,
                   parent_id INTEGER,
                   code TEXT,
                   FOREIGN KEY (parent_id) REFERENCES parents(id) ON DELETE CASCADE
                 );
                 CREATE UNIQUE INDEX idx_children_code ON children(code);",
            )
            .unwrap();
        let config = sqlite_test_connection(path.to_string_lossy().into_owned());

        let indexes = list_indexes(&config, "children").unwrap();
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].name, "idx_children_code");
        assert!(indexes[0].unique);
        assert_eq!(indexes[0].columns, vec!["code"]);

        let foreign_keys = list_foreign_keys(&config, "children").unwrap();
        assert_eq!(foreign_keys.len(), 1);
        assert_eq!(foreign_keys[0].columns, vec!["parent_id"]);
        assert_eq!(foreign_keys[0].referenced_table, "parents");
        assert_eq!(foreign_keys[0].referenced_columns, vec!["id"]);
        assert_eq!(foreign_keys[0].on_delete.as_deref(), Some("CASCADE"));

        std::fs::remove_file(path).unwrap();
    }

    fn sqlite_test_connection(database: String) -> DbConnection {
        DbConnection {
            id: "test".into(),
            name: "test".into(),
            db_type: "sqlite".into(),
            host: None,
            port: None,
            database,
            username: None,
            password: None,
            ssl_mode: None,
            environment: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}
