pub mod mysql;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        if self.db_type != "mysql" {
            return Err("This first version supports MySQL connections only".into());
        }
        if self.host.as_deref().unwrap_or("").trim().is_empty() {
            return Err("Host is required".into());
        }
        if self.database.trim().is_empty() {
            return Err("Database is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub name: String,
    pub schema: Option<String>,
    pub table_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSyncTableMeta {
    pub name: String,
    pub source_exists: bool,
    pub target_exists: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareRequest {
    pub id: String,
    pub source_connection_id: String,
    pub target_connection_id: String,
    pub table_name: String,
    pub allow_delete: bool,
    pub created_at: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCompareRun {
    pub id: String,
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
    pub task_id: String,
    pub task_name: String,
    pub source_name: String,
    pub target_name: String,
    pub summary: CompareSummary,
    pub diffs: Vec<SchemaDiff>,
    pub sync_sql: String,
    pub created_at: String,
}
