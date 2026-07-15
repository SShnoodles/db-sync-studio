use postgres::{Client, NoTls, Row};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

use super::{ColumnMeta, DbConnection, TableMeta, TypeMeta};

fn client(connection: &DbConnection) -> Result<Client, String> {
    connection.validate()?;
    let host = connection.host.as_deref().unwrap_or("127.0.0.1");
    let port = connection.port.unwrap_or(5432);
    let user = connection.username.as_deref().unwrap_or("postgres");
    let password = connection.password.as_deref().unwrap_or("");
    let config = format!(
        "host={} port={} user={} password={} dbname={}",
        host, port, user, password, connection.database
    );
    Client::connect(&config, NoTls)
        .map_err(|error| format!("Unable to connect to PostgreSQL: {error}"))
}

pub fn test_connection(connection: &DbConnection) -> Result<String, String> {
    let mut client = client(connection)?;
    let version: String = client
        .query_one("SELECT version()", &[])
        .map_err(|error| format!("Connection test failed: {error}"))?
        .get(0);
    Ok(format!("Connected to {version}"))
}

pub fn list_tables(connection: &DbConnection) -> Result<Vec<TableMeta>, String> {
    let mut client = client(connection)?;
    let rows = client
        .query(
            "SELECT
                cls.relname,
                CASE cls.relkind WHEN 'r' THEN 'BASE TABLE' WHEN 'v' THEN 'VIEW' ELSE cls.relkind::text END,
                obj_description(cls.oid, 'pg_class') AS comment
             FROM pg_class cls
             JOIN pg_namespace ns ON ns.oid = cls.relnamespace
             WHERE ns.nspname = current_schema()
               AND cls.relkind IN ('r', 'v')
             ORDER BY cls.relname",
            &[],
        )
        .map_err(|error| format!("Unable to load tables: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|row| TableMeta {
            name: row.get(0),
            schema: Some("public".into()),
            table_type: row.get(1),
            comment: row.get(2),
        })
        .collect())
}

pub fn list_columns(connection: &DbConnection, table: &str) -> Result<Vec<ColumnMeta>, String> {
    let mut client = client(connection)?;
    let rows = client
        .query(
            "SELECT
                cls.relname AS table_name,
                attr.attname AS column_name,
                CASE
                    WHEN pg_catalog.pg_get_serial_sequence(pg_catalog.quote_ident(ns.nspname) || '.' || pg_catalog.quote_ident(cls.relname), attr.attname) IS NOT NULL
                         AND attr.atttypid = 'int2'::regtype THEN 'smallserial'
                    WHEN pg_catalog.pg_get_serial_sequence(pg_catalog.quote_ident(ns.nspname) || '.' || pg_catalog.quote_ident(cls.relname), attr.attname) IS NOT NULL
                         AND attr.atttypid = 'int4'::regtype THEN 'serial'
                    WHEN pg_catalog.pg_get_serial_sequence(pg_catalog.quote_ident(ns.nspname) || '.' || pg_catalog.quote_ident(cls.relname), attr.attname) IS NOT NULL
                         AND attr.atttypid = 'int8'::regtype THEN 'bigserial'
                    ELSE pg_catalog.format_type(attr.atttypid, attr.atttypmod)
                END AS column_type,
                CASE WHEN attr.attnotnull THEN 'NO' ELSE 'YES' END AS is_nullable,
                CASE
                    WHEN pg_catalog.pg_get_serial_sequence(pg_catalog.quote_ident(ns.nspname) || '.' || pg_catalog.quote_ident(cls.relname), attr.attname) IS NOT NULL
                         AND attr.atttypid IN ('int2'::regtype, 'int4'::regtype, 'int8'::regtype) THEN NULL
                    ELSE pg_catalog.pg_get_expr(def.adbin, def.adrelid)
                END AS column_default,
                EXISTS (
                    SELECT 1
                    FROM pg_index idx
                    WHERE idx.indrelid = attr.attrelid
                      AND idx.indisprimary
                      AND attr.attnum = ANY(idx.indkey)
                ) AS is_primary_key,
                attr.attnum::int4 AS ordinal_position,
                col_description(attr.attrelid, attr.attnum) AS comment
             FROM pg_attribute attr
             JOIN pg_class cls ON cls.oid = attr.attrelid
             JOIN pg_namespace ns ON ns.oid = cls.relnamespace
             LEFT JOIN pg_attrdef def
               ON def.adrelid = attr.attrelid
              AND def.adnum = attr.attnum
             WHERE ns.nspname = current_schema()
               AND cls.relname = $1
               AND attr.attnum > 0
               AND NOT attr.attisdropped
             ORDER BY attr.attnum",
            &[&table],
        )
        .map_err(|error| format!("Unable to load columns for {table}: {error}"))?;
    Ok(rows
        .into_iter()
        .map(|row| ColumnMeta {
            table_name: row.get(0),
            name: row.get(1),
            column_type: row.get(2),
            nullable: row.get::<_, String>(3) == "YES",
            default_value: row.get(4),
            is_primary_key: row.get(5),
            extra: None,
            ordinal_position: row.get::<_, i32>(6) as u64,
            comment: row.get(7),
            spatial_srid: None,
        })
        .collect())
}

pub fn list_enum_types(connection: &DbConnection) -> Result<Vec<TypeMeta>, String> {
    let mut client = client(connection)?;
    let rows = client
        .query(
            "SELECT t.typname, e.enumlabel
             FROM pg_type t
             JOIN pg_enum e ON e.enumtypid = t.oid
             JOIN pg_namespace n ON n.oid = t.typnamespace
             WHERE n.nspname = current_schema()
             ORDER BY t.typname, e.enumsortorder",
            &[],
        )
        .map_err(|error| format!("Unable to load enum types: {error}"))?;
    let mut types = Vec::<TypeMeta>::new();
    for row in rows {
        let name: String = row.get(0);
        let value: String = row.get(1);
        if let Some(item) = types.iter_mut().find(|item| item.name == name) {
            item.values.push(value);
        } else {
            types.push(TypeMeta {
                name,
                values: vec![value],
            });
        }
    }
    Ok(types)
}

pub fn create_enum_type_sql(type_meta: &TypeMeta) -> String {
    let values = type_meta
        .values
        .iter()
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE TYPE {} AS ENUM ({});",
        quote_identifier(&type_meta.name),
        values
    )
}

pub fn show_create_table(connection: &DbConnection, table: &str) -> Result<String, String> {
    let columns = list_columns(connection, table)?;
    if columns.is_empty() {
        return Err(format!("Columns were not found for {table}"));
    }
    let mut definitions = columns.iter().map(column_definition).collect::<Vec<_>>();
    let primary_keys = columns
        .iter()
        .filter(|column| column.is_primary_key)
        .map(|column| quote_identifier(&column.name))
        .collect::<Vec<_>>();
    if !primary_keys.is_empty() {
        definitions.push(format!("PRIMARY KEY ({})", primary_keys.join(", ")));
    }
    let mut sql = format!(
        "CREATE TABLE {} (\n  {}\n);",
        quote_identifier(table),
        definitions.join(",\n  ")
    );
    append_table_comments(connection, table, &columns, &mut sql)?;
    Ok(sql)
}

pub fn primary_keys(connection: &DbConnection, table: &str) -> Result<Vec<String>, String> {
    let mut client = client(connection)?;
    client
        .query(
            "SELECT k.column_name
             FROM information_schema.key_column_usage k
             JOIN information_schema.table_constraints tc
               ON tc.constraint_name = k.constraint_name
              AND tc.table_schema = k.table_schema
              AND tc.table_name = k.table_name
             WHERE k.table_schema = current_schema()
               AND k.table_name = $1
               AND tc.constraint_type = 'PRIMARY KEY'
             ORDER BY k.ordinal_position",
            &[&table],
        )
        .map_err(|error| format!("Unable to load primary keys for {table}: {error}"))
        .map(|rows| rows.into_iter().map(|row| row.get(0)).collect())
}

pub fn fetch_rows(
    connection: &DbConnection,
    table: &str,
    order_columns: &[String],
    limit: usize,
    offset: usize,
) -> Result<Vec<BTreeMap<String, JsonValue>>, String> {
    let mut client = client(connection)?;
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
        "SELECT row_to_json(t)::text FROM (SELECT * FROM {}{order_by} LIMIT {limit} OFFSET {offset}) t",
        quote_identifier(table)
    );
    let rows = client
        .query(&sql, &[])
        .map_err(|error| format!("Unable to fetch rows from {table}: {error}"))?;
    rows.into_iter().map(row_to_map).collect()
}

pub fn execute_schema_statements(
    connection: &DbConnection,
    statements: &[String],
) -> Result<(), String> {
    let mut client = client(connection)?;
    for statement in statements {
        client.batch_execute(statement).map_err(|error| {
            format!("Unable to execute PostgreSQL schema SQL: {error}\n{statement}")
        })?;
    }
    Ok(())
}

fn row_to_map(row: Row) -> Result<BTreeMap<String, JsonValue>, String> {
    let json: String = row.get(0);
    let value: JsonValue = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    match value {
        JsonValue::Object(map) => Ok(map.into_iter().collect()),
        _ => Err("PostgreSQL row JSON was not an object".into()),
    }
}

fn column_definition(column: &ColumnMeta) -> String {
    let mut definition = format!(
        "{} {} {}",
        quote_identifier(&column.name),
        column.column_type,
        if column.nullable { "NULL" } else { "NOT NULL" }
    );
    if let Some(default_value) = &column.default_value {
        definition.push_str(" DEFAULT ");
        definition.push_str(default_value);
    }
    definition
}

pub fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn append_table_comments(
    connection: &DbConnection,
    table: &str,
    columns: &[ColumnMeta],
    sql: &mut String,
) -> Result<(), String> {
    let table_comment = list_tables(connection)?
        .into_iter()
        .find(|item| item.name == table)
        .and_then(|item| item.comment);
    if let Some(comment) = table_comment {
        sql.push('\n');
        sql.push_str(&comment_on_table_sql(table, Some(&comment)));
    }
    for column in columns.iter().filter(|column| column.comment.is_some()) {
        sql.push('\n');
        sql.push_str(&comment_on_column_sql(
            table,
            &column.name,
            column.comment.as_deref(),
        ));
    }
    Ok(())
}

pub fn comment_on_table_sql(table: &str, comment: Option<&str>) -> String {
    format!(
        "COMMENT ON TABLE {} IS {};",
        quote_identifier(table),
        comment_literal(comment)
    )
}

pub fn comment_on_column_sql(table: &str, column: &str, comment: Option<&str>) -> String {
    format!(
        "COMMENT ON COLUMN {}.{} IS {};",
        quote_identifier(table),
        quote_identifier(column),
        comment_literal(comment)
    )
}

fn comment_literal(comment: Option<&str>) -> String {
    comment
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".into())
}
