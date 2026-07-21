pub mod mysql;
pub mod postgres;
pub mod sqlite;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbConnection {
    pub id: String,
    pub name: String,
    pub db_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub environment: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl DbConnection {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Connection name is required".into());
        }
        if !matches!(self.db_type.as_str(), "mysql" | "postgresql" | "sqlite") {
            return Err("Supported database types are MySQL, PostgreSQL and SQLite".into());
        }
        if self.db_type != "sqlite" && self.host.as_deref().unwrap_or("").trim().is_empty() {
            return Err("Host is required".into());
        }
        if self.database.trim().is_empty() {
            return Err("Database is required".into());
        }
        if self.db_type != "sqlite"
            && !matches!(
                self.ssl_mode.as_deref().unwrap_or("require"),
                "disable" | "prefer" | "require"
            )
        {
            return Err("SSL mode must be disable, prefer or require".into());
        }
        Ok(())
    }
}

pub fn ensure_same_db_type(source: &DbConnection, target: &DbConnection) -> Result<(), String> {
    if source.db_type != target.db_type {
        return Err("Source and target must use the same database type".into());
    }
    Ok(())
}

pub fn test_connection(connection: &DbConnection) -> Result<String, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::test_connection(connection),
        "postgresql" => postgres::test_connection(connection),
        "sqlite" => sqlite::test_connection(connection),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn list_tables(connection: &DbConnection) -> Result<Vec<TableMeta>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::list_tables(connection),
        "postgresql" => postgres::list_tables(connection),
        "sqlite" => sqlite::list_tables(connection),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn list_columns(connection: &DbConnection, table: &str) -> Result<Vec<ColumnMeta>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::list_columns(connection, table),
        "postgresql" => postgres::list_columns(connection, table),
        "sqlite" => sqlite::list_columns(connection, table),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn show_create_table(connection: &DbConnection, table: &str) -> Result<String, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::show_create_table(connection, table),
        "postgresql" => postgres::show_create_table(connection, table),
        "sqlite" => sqlite::show_create_table(connection, table),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn show_create_view(connection: &DbConnection, view: &str) -> Result<String, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::show_create_view(connection, view),
        "postgresql" => postgres::show_create_view(connection, view),
        "sqlite" => sqlite::show_create_view(connection, view),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn primary_keys(connection: &DbConnection, table: &str) -> Result<Vec<String>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::primary_keys(connection, table),
        "postgresql" => postgres::primary_keys(connection, table),
        "sqlite" => sqlite::primary_keys(connection, table),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn list_indexes(connection: &DbConnection, table: &str) -> Result<Vec<IndexMeta>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::list_indexes(connection, table),
        "postgresql" => postgres::list_indexes(connection, table),
        "sqlite" => sqlite::list_indexes(connection, table),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn list_foreign_keys(
    connection: &DbConnection,
    table: &str,
) -> Result<Vec<ForeignKeyMeta>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::list_foreign_keys(connection, table),
        "postgresql" => postgres::list_foreign_keys(connection, table),
        "sqlite" => sqlite::list_foreign_keys(connection, table),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn fetch_rows(
    connection: &DbConnection,
    table: &str,
    order_columns: &[String],
    limit: usize,
    offset: usize,
) -> Result<Vec<BTreeMap<String, Value>>, String> {
    match connection.db_type.as_str() {
        "mysql" => mysql::fetch_rows(connection, table, order_columns, limit, offset),
        "postgresql" => postgres::fetch_rows(connection, table, order_columns, limit, offset),
        "sqlite" => sqlite::fetch_rows(connection, table, order_columns, limit, offset),
        _ => Err("Unsupported database type".into()),
    }
}

pub fn execute_schema_sql(
    connection: &DbConnection,
    sql: &str,
) -> Result<SchemaSyncResult, String> {
    let executed = execute_sql(connection, sql, false)?;
    Ok(SchemaSyncResult {
        executed,
        skipped: 0,
    })
}

pub fn execute_data_sql(connection: &DbConnection, sql: &str) -> Result<DataSyncResult, String> {
    let executed = execute_sql(connection, sql, true)?;
    Ok(DataSyncResult {
        executed,
        skipped: 0,
    })
}

pub fn quote_identifier(connection: &DbConnection, value: &str) -> String {
    match connection.db_type.as_str() {
        "postgresql" => postgres::quote_identifier(value),
        "sqlite" => sqlite::quote_identifier(value),
        _ => format!("`{}`", value.replace('`', "``")),
    }
}

pub fn null_safe_eq_operator(connection: &DbConnection) -> &'static str {
    match connection.db_type.as_str() {
        "postgresql" => "IS NOT DISTINCT FROM",
        "sqlite" => "IS",
        _ => "<=>",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub name: String,
    pub schema: Option<String>,
    pub table_type: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSyncTableMeta {
    pub name: String,
    pub source_exists: bool,
    pub target_exists: bool,
    pub source_object_type: Option<String>,
    pub target_object_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub table_name: String,
    pub name: String,
    pub column_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub extra: Option<String>,
    pub ordinal_position: u64,
    pub comment: Option<String>,
    pub spatial_srid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeMeta {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexMeta {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    pub definition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyMeta {
    pub name: String,
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSyncRequest {
    pub target_connection_id: String,
    pub sql: String,
}

impl SchemaSyncRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_connection_id.trim().is_empty() {
            return Err("Target connection is required".into());
        }
        if self.sql.trim().is_empty() {
            return Err("SQL is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSyncResult {
    pub executed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSyncRequest {
    pub target_connection_id: String,
    pub sql: String,
}

impl DataSyncRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.target_connection_id.trim().is_empty() {
            return Err("Target connection is required".into());
        }
        if self.sql.trim().is_empty() {
            return Err("SQL is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSyncResult {
    pub executed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionSummary {
    pub statements: usize,
    pub executed: usize,
    pub skipped: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionHistoryRun {
    pub run_type: String,
    pub sync_type: String,
    pub id: String,
    pub db_type: Option<String>,
    pub title: String,
    pub source_name: String,
    pub target_name: String,
    pub target_connection_id: String,
    pub status: String,
    pub summary: ExecutionSummary,
    pub error: Option<String>,
    pub sync_sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareTask {
    pub id: String,
    pub name: String,
    pub source_connection_id: String,
    pub target_connection_id: String,
    pub compare_type: String,
    pub selected_tables: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub sync_type: Option<String>,
    pub database_type: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub search_content: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

impl CompareTask {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Task name is required".into());
        }
        if self.source_connection_id.trim().is_empty() {
            return Err("Source connection is required".into());
        }
        if self.target_connection_id.trim().is_empty() {
            return Err("Target connection is required".into());
        }
        if self.source_connection_id == self.target_connection_id {
            return Err("Source and target must be different connections".into());
        }
        if self.compare_type != "schema" {
            return Err("This version supports schema compare tasks only".into());
        }
        Ok(())
    }
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut dollar_quote: Option<String> = None;
    let chars = sql.char_indices().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let (byte_index, character) = chars[index];
        let next = chars.get(index + 1).map(|(_, value)| *value);
        let previous = if index > 0 {
            chars.get(index - 1).map(|(_, value)| *value)
        } else {
            None
        };

        if let Some(delimiter) = dollar_quote.as_deref() {
            if sql[byte_index..].starts_with(delimiter) {
                index += delimiter.chars().count();
                dollar_quote = None;
            } else {
                index += 1;
            }
            continue;
        }

        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if in_block_comment {
            if character == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if !in_single && !in_double && !in_backtick {
            if character == '-' && next == Some('-') {
                in_line_comment = true;
                index += 2;
                continue;
            }
            if character == '/' && next == Some('*') {
                in_block_comment = true;
                index += 2;
                continue;
            }
            if character == '$' {
                if let Some(delimiter) = dollar_quote_delimiter(sql, byte_index) {
                    dollar_quote = Some(delimiter.to_string());
                    index += delimiter.chars().count();
                    continue;
                }
            }
        }

        match character {
            '\'' if !in_double && !in_backtick => {
                if in_single && next == Some('\'') {
                    index += 2;
                    continue;
                }
                if previous != Some('\\') {
                    in_single = !in_single;
                }
            }
            '"' if !in_single && !in_backtick => in_double = !in_double,
            '`' if !in_single && !in_double => in_backtick = !in_backtick,
            ';' if !in_single && !in_double && !in_backtick => {
                let statement = sql[start..=byte_index].trim();
                if !statement.is_empty() {
                    statements.push(statement.to_string());
                }
                start = byte_index + character.len_utf8();
            }
            _ => {}
        }
        index += 1;
    }

    let trailing = sql[start..].trim();
    if !trailing.is_empty() {
        statements.push(trailing.to_string());
    }
    statements
}

pub fn sql_statement_count(sql: &str) -> usize {
    split_sql_statements(sql)
        .into_iter()
        .filter(|statement| has_executable_sql(statement))
        .count()
}

fn execute_sql(connection: &DbConnection, sql: &str, transactional: bool) -> Result<usize, String> {
    let statements = split_sql_statements(sql)
        .into_iter()
        .filter(|statement| has_executable_sql(statement))
        .collect::<Vec<_>>();
    if statements.is_empty() {
        return Ok(0);
    }

    match (connection.db_type.as_str(), transactional) {
        ("mysql", true) => mysql::execute_data_statements(connection, &statements)?,
        ("postgresql", true) => postgres::execute_data_statements(connection, &statements)?,
        ("sqlite", true) => sqlite::execute_data_statements(connection, &statements)?,
        ("mysql", false) => mysql::execute_schema_statements(connection, &statements)?,
        ("postgresql", false) => postgres::execute_schema_statements(connection, &statements)?,
        ("sqlite", false) => sqlite::execute_schema_statements(connection, &statements)?,
        _ => return Err("Unsupported database type".into()),
    }

    Ok(statements.len())
}

fn has_executable_sql(statement: &str) -> bool {
    !strip_sql_comments(statement)
        .trim()
        .trim_matches(';')
        .trim()
        .is_empty()
}

fn strip_sql_comments(sql: &str) -> String {
    let mut output = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut in_backtick = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut dollar_quote: Option<String> = None;
    let chars = sql.char_indices().collect::<Vec<_>>();
    let mut index = 0;

    while index < chars.len() {
        let (byte_index, character) = chars[index];
        let next = chars.get(index + 1).map(|(_, value)| *value);
        let previous = if index > 0 {
            chars.get(index - 1).map(|(_, value)| *value)
        } else {
            None
        };

        if let Some(delimiter) = dollar_quote.as_deref() {
            if sql[byte_index..].starts_with(delimiter) {
                output.push_str(delimiter);
                index += delimiter.chars().count();
                dollar_quote = None;
            } else {
                output.push(character);
                index += 1;
            }
            continue;
        }

        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
                output.push('\n');
            }
            index += 1;
            continue;
        }
        if in_block_comment {
            if character == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if !in_single && !in_double && !in_backtick {
            if character == '-' && next == Some('-') {
                in_line_comment = true;
                index += 2;
                continue;
            }
            if character == '/' && next == Some('*') {
                in_block_comment = true;
                index += 2;
                continue;
            }
            if character == '$' {
                if let Some(delimiter) = dollar_quote_delimiter(sql, byte_index) {
                    output.push_str(delimiter);
                    dollar_quote = Some(delimiter.to_string());
                    index += delimiter.chars().count();
                    continue;
                }
            }
        }

        match character {
            '\'' if !in_double && !in_backtick => {
                if in_single && next == Some('\'') {
                    output.push(character);
                    output.push('\'');
                    index += 2;
                    continue;
                }
                if previous != Some('\\') {
                    in_single = !in_single;
                }
            }
            '"' if !in_single && !in_backtick => in_double = !in_double,
            '`' if !in_single && !in_double => in_backtick = !in_backtick,
            _ => {}
        }
        output.push(character);
        index += 1;
    }
    output
}

fn dollar_quote_delimiter(sql: &str, start: usize) -> Option<&str> {
    let rest = sql.get(start..)?;
    if !rest.starts_with('$') {
        return None;
    }
    let closing = rest.get(1..)?.find('$')? + 1;
    let tag = rest.get(1..closing)?;
    if !tag
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    rest.get(..=closing)
}

#[cfg(test)]
mod sql_tests {
    use super::{
        quote_identifier, split_sql_statements, sql_statement_count, strip_sql_comments,
        DbConnection,
    };

    #[test]
    fn splits_plain_statements_and_keeps_trailing_statement() {
        assert_eq!(
            split_sql_statements("SELECT 1; INSERT INTO items VALUES (2)"),
            vec!["SELECT 1;", "INSERT INTO items VALUES (2)"]
        );
    }

    #[test]
    fn ignores_semicolons_inside_strings_and_quoted_identifiers() {
        let sql = "INSERT INTO `semi;table` (\"semi;column\", value) VALUES ('it''s;ok', '反斜杠\\';仍在字符串'); SELECT 2;";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("it''s;ok"));
        assert_eq!(statements[1], "SELECT 2;");
    }

    #[test]
    fn ignores_semicolons_in_comments_and_comment_only_fragments() {
        let sql = "-- first; comment\nSELECT 1; /* second; comment */ ; -- only comment;";
        assert_eq!(sql_statement_count(sql), 1);
        assert_eq!(
            strip_sql_comments("SELECT '--not-comment'; -- comment"),
            "SELECT '--not-comment'; "
        );
    }

    #[test]
    fn keeps_postgres_dollar_quoted_blocks_together() {
        let sql = "CREATE FUNCTION demo() RETURNS void AS $body$\nBEGIN\n  PERFORM 'a;b'; -- body comment\n  PERFORM 2;\nEND;\n$body$ LANGUAGE plpgsql;\nSELECT 1;";
        let statements = split_sql_statements(sql);
        assert_eq!(statements.len(), 2);
        assert!(statements[0].contains("PERFORM 2;"));
        assert_eq!(statements[1], "SELECT 1;");
        assert_eq!(sql_statement_count(sql), 2);
    }

    #[test]
    fn quotes_identifiers_for_each_database_dialect() {
        assert_eq!(
            quote_identifier(&connection("mysql"), "order`item"),
            "`order``item`"
        );
        assert_eq!(
            quote_identifier(&connection("postgresql"), "order\"item"),
            "\"order\"\"item\""
        );
        assert_eq!(
            quote_identifier(&connection("sqlite"), "订单\"明细"),
            "\"订单\"\"明细\""
        );
    }

    fn connection(db_type: &str) -> DbConnection {
        DbConnection {
            id: "test".into(),
            name: "test".into(),
            db_type: db_type.into(),
            host: None,
            port: None,
            database: "test".into(),
            username: None,
            password: None,
            ssl_mode: None,
            environment: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareRequest {
    pub source_connection_id: String,
    pub target_connection_id: String,
    pub table_name: String,
    pub allow_delete: bool,
}

impl DataCompareRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_connection_id.trim().is_empty() {
            return Err("Source connection is required".into());
        }
        if self.target_connection_id.trim().is_empty() {
            return Err("Target connection is required".into());
        }
        if self.source_connection_id == self.target_connection_id {
            return Err("Source and target must be different connections".into());
        }
        if self.table_name.trim().is_empty() {
            return Err("Table is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedColumn {
    pub column_name: String,
    pub source_value: Value,
    pub target_value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDiff {
    pub table_name: String,
    pub key: Vec<(String, Value)>,
    pub diff_type: String,
    pub source_row: Option<Vec<(String, Value)>>,
    pub target_row: Option<Vec<(String, Value)>>,
    pub changed_columns: Vec<ChangedColumn>,
    pub sync_sql: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareSummary {
    pub total_diffs: usize,
    pub inserts: usize,
    pub updates: usize,
    pub deletes: usize,
    pub same_rows: usize,
    pub compared_rows: usize,
    #[serde(default)]
    pub source_rows: usize,
    #[serde(default)]
    pub target_rows: usize,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareRun {
    pub id: String,
    pub db_type: Option<String>,
    pub table_name: String,
    pub source_name: String,
    pub target_name: String,
    pub key_columns: Vec<String>,
    pub summary: DataCompareSummary,
    pub diffs: Vec<DataDiff>,
    pub sync_sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareHistorySummary {
    pub tables: usize,
    pub total_diffs: usize,
    pub inserts: usize,
    pub updates: usize,
    pub deletes: usize,
    pub same_rows: usize,
    pub compared_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareHistoryRun {
    pub run_type: String,
    pub id: String,
    pub db_type: Option<String>,
    pub title: String,
    pub source_name: String,
    pub target_name: String,
    pub summary: DataCompareHistorySummary,
    pub runs: Vec<DataCompareRun>,
    pub sync_sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareHistoryRequest {
    pub db_type: Option<String>,
    pub title: String,
    pub source_name: String,
    pub target_name: String,
    pub summary: DataCompareHistorySummary,
    pub runs: Vec<DataCompareRun>,
    pub sync_sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    pub object_type: String,
    pub table_name: String,
    pub column_name: Option<String>,
    pub diff_type: String,
    pub source_value: Option<String>,
    pub target_value: Option<String>,
    pub sync_sql: Option<String>,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareSummary {
    pub total_diffs: usize,
    pub table_diffs: usize,
    pub column_diffs: usize,
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub same: usize,
    pub low_risk: usize,
    pub medium_risk: usize,
    pub high_risk: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareRun {
    pub id: String,
    pub db_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub task_name: String,
    pub source_name: String,
    pub target_name: String,
    pub summary: CompareSummary,
    pub diffs: Vec<SchemaDiff>,
    pub sync_sql: String,
    pub created_at: String,
}
