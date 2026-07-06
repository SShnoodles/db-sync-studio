use crate::db::{CompareRun, CompareTask, DataCompareHistoryRun, DbConnection};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, sync::Mutex};

pub struct LocalStore {
    connection: Mutex<Connection>,
}

fn ensure_compare_history_sync_type(connection: &Connection) -> Result<(), String> {
    let has_column = connection
        .prepare("PRAGMA table_info(compare_history)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .any(|column| column == "sync_type");

    if !has_column {
        connection
            .execute(
                "ALTER TABLE compare_history ADD COLUMN sync_type TEXT NOT NULL DEFAULT 'schema'",
                [],
            )
            .map_err(|error| error.to_string())?;
    }

    connection
        .execute(
            "UPDATE compare_history
            SET sync_type = CASE WHEN task_id = 'data' THEN 'data' ELSE 'schema' END
            WHERE sync_type IS NULL OR sync_type = '' OR sync_type = 'schema'",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn ensure_compare_history_columns(connection: &Connection) -> Result<(), String> {
    ensure_compare_history_sync_type(connection)?;
    let columns = connection
        .prepare("PRAGMA table_info(compare_history)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    for (name, ddl) in [
        (
            "db_type",
            "ALTER TABLE compare_history ADD COLUMN db_type TEXT",
        ),
        (
            "source_name",
            "ALTER TABLE compare_history ADD COLUMN source_name TEXT",
        ),
        (
            "target_name",
            "ALTER TABLE compare_history ADD COLUMN target_name TEXT",
        ),
        ("title", "ALTER TABLE compare_history ADD COLUMN title TEXT"),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute(ddl, [])
                .map_err(|error| error.to_string())?;
        }
    }

    backfill_compare_history_columns(connection)?;
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_compare_history_created_at ON compare_history(created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_compare_history_sync_type ON compare_history(sync_type);
             CREATE INDEX IF NOT EXISTS idx_compare_history_db_type ON compare_history(db_type);
             CREATE INDEX IF NOT EXISTS idx_compare_history_source_name ON compare_history(source_name);
             CREATE INDEX IF NOT EXISTS idx_compare_history_target_name ON compare_history(target_name);
             CREATE INDEX IF NOT EXISTS idx_compare_history_title ON compare_history(title);",
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn backfill_compare_history_columns(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT id, sync_type, result_json FROM compare_history
             WHERE db_type IS NULL OR source_name IS NULL OR target_name IS NULL OR title IS NULL",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    for (id, sync_type, result_json) in rows {
        let db_type = extract_json_string(&result_json, "dbType");
        let source_name = extract_json_string(&result_json, "sourceName");
        let target_name = extract_json_string(&result_json, "targetName");
        let title = if sync_type == "data" {
            extract_json_string(&result_json, "title")
        } else {
            extract_json_string(&result_json, "taskName")
        };
        connection
            .execute(
                "UPDATE compare_history
                 SET db_type = COALESCE(db_type, ?1),
                     source_name = COALESCE(source_name, ?2),
                     target_name = COALESCE(target_name, ?3),
                     title = COALESCE(title, ?4)
                 WHERE id = ?5",
                params![db_type, source_name, target_name, title, id],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

impl LocalStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS connections (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, config_json TEXT NOT NULL,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS compare_tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                source_connection_id TEXT NOT NULL,
                target_connection_id TEXT NOT NULL,
                compare_type TEXT NOT NULL,
                config_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS compare_history (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                sync_type TEXT NOT NULL DEFAULT 'schema',
                db_type TEXT,
                source_name TEXT,
                target_name TEXT,
                title TEXT,
                result_summary_json TEXT NOT NULL,
                result_json TEXT NOT NULL,
                report_path TEXT,
                created_at TEXT NOT NULL
            );",
            )
            .map_err(|error| error.to_string())?;
        ensure_compare_history_columns(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_connections(&self) -> Result<Vec<DbConnection>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let mut statement = connection
            .prepare("SELECT config_json FROM connections ORDER BY updated_at DESC")
            .map_err(|error| error.to_string())?;
        let items = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .map(|item| {
                item.map_err(|error| error.to_string())
                    .and_then(|json| serde_json::from_str(&json).map_err(|error| error.to_string()))
            })
            .collect::<Result<Vec<DbConnection>, String>>()?;
        Ok(items)
    }

    pub fn get_connection(&self, id: &str) -> Result<DbConnection, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let json: String = connection
            .query_row(
                "SELECT config_json FROM connections WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| "Connection was not found".to_string())?;
        serde_json::from_str(&json).map_err(|error| error.to_string())
    }

    pub fn save_connection(&self, item: &DbConnection) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let json = serde_json::to_string(item).map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO connections (id, name, config_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET name = excluded.name, config_json = excluded.config_json, updated_at = excluded.updated_at",
            params![item.id, item.name, json, item.created_at, item.updated_at]
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn delete_connection(&self, id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        connection
            .execute("DELETE FROM connections WHERE id = ?1", [id])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_tasks(&self) -> Result<Vec<CompareTask>, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let mut statement = connection
            .prepare("SELECT config_json FROM compare_tasks ORDER BY updated_at DESC")
            .map_err(|error| error.to_string())?;
        let items = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .map(|item| {
                item.map_err(|error| error.to_string())
                    .and_then(|json| serde_json::from_str(&json).map_err(|error| error.to_string()))
            })
            .collect::<Result<Vec<CompareTask>, String>>()?;
        Ok(items)
    }

    pub fn get_task(&self, id: &str) -> Result<CompareTask, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let json: String = connection
            .query_row(
                "SELECT config_json FROM compare_tasks WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| "Compare task was not found".to_string())?;
        serde_json::from_str(&json).map_err(|error| error.to_string())
    }

    pub fn save_task(&self, task: &CompareTask) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let json = serde_json::to_string(task).map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO compare_tasks (id, name, source_connection_id, target_connection_id, compare_type, config_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET name = excluded.name, source_connection_id = excluded.source_connection_id, target_connection_id = excluded.target_connection_id, compare_type = excluded.compare_type, config_json = excluded.config_json, updated_at = excluded.updated_at",
            params![task.id, task.name, task.source_connection_id, task.target_connection_id, task.compare_type, json, task.created_at, task.updated_at]
        ).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn delete_task(&self, id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        connection
            .execute("DELETE FROM compare_tasks WHERE id = ?1", [id])
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn save_history(&self, run: &CompareRun) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let summary_json =
            serde_json::to_string(&run.summary).map_err(|error| error.to_string())?;
        let result_json = serde_json::to_string(run).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO compare_history
                 (id, task_id, sync_type, db_type, source_name, target_name, title, result_summary_json, result_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run.id,
                    run.task_id,
                    "schema",
                    run.db_type,
                    run.source_name,
                    run.target_name,
                    run.task_name,
                    summary_json,
                    result_json,
                    run.created_at
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn save_data_history(&self, run: &DataCompareHistoryRun) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let summary_json =
            serde_json::to_string(&run.summary).map_err(|error| error.to_string())?;
        let result_json = serde_json::to_string(run).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO compare_history
                 (id, task_id, sync_type, db_type, source_name, target_name, title, result_summary_json, result_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run.id,
                    run.run_type,
                    "data",
                    run.db_type,
                    run.source_name,
                    run.target_name,
                    run.title,
                    summary_json,
                    result_json,
                    run.created_at
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn list_history(
        &self,
        sync_type: Option<String>,
        database_type: Option<String>,
        start_time: Option<String>,
        end_time: Option<String>,
        search_content: Option<String>,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<Value>, usize), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let search_pattern = search_content.map(|value| format!("%{}%", escape_like(&value)));
        let safe_page = page.max(1);
        let safe_page_size = page_size.clamp(1, 100);
        let offset = (safe_page - 1) * safe_page_size;
        let total = connection
            .query_row(
                "SELECT COUNT(*) FROM compare_history
                WHERE (?1 IS NULL OR sync_type = ?1)
                AND (?2 IS NULL OR created_at >= ?2)
                AND (?3 IS NULL OR created_at <= ?3)
                AND (?4 IS NULL OR db_type = ?4)
                AND (?5 IS NULL
                    OR title LIKE ?5 ESCAPE '\\'
                    OR source_name LIKE ?5 ESCAPE '\\'
                    OR target_name LIKE ?5 ESCAPE '\\'
                    OR db_type LIKE ?5 ESCAPE '\\')",
                params![
                    sync_type,
                    start_time,
                    end_time,
                    database_type,
                    search_pattern
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())? as usize;
        let mut statement = connection
            .prepare(
                "SELECT sync_type, result_summary_json, id, db_type, source_name, target_name, title, created_at FROM compare_history
                WHERE (?1 IS NULL OR sync_type = ?1)
                AND (?2 IS NULL OR created_at >= ?2)
                AND (?3 IS NULL OR created_at <= ?3)
                AND (?4 IS NULL OR db_type = ?4)
                AND (?5 IS NULL
                    OR title LIKE ?5 ESCAPE '\\'
                    OR source_name LIKE ?5 ESCAPE '\\'
                    OR target_name LIKE ?5 ESCAPE '\\'
                    OR db_type LIKE ?5 ESCAPE '\\')
                ORDER BY created_at DESC
                LIMIT ?6 OFFSET ?7",
            )
            .map_err(|error| error.to_string())?;
        let items = statement
            .query_map(
                params![
                    sync_type,
                    start_time,
                    end_time,
                    database_type,
                    search_pattern,
                    safe_page_size,
                    offset
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .map_err(|error| error.to_string())?
            .map(|item| {
                item.map_err(|error| error.to_string()).and_then(
                    |(
                        sync_type,
                        summary_json,
                        id,
                        db_type,
                        source_name,
                        target_name,
                        title,
                        created_at,
                    )| {
                        history_summary_value(
                            &sync_type,
                            &summary_json,
                            HistorySummaryFields {
                                id,
                                db_type,
                                source_name,
                                target_name,
                                title,
                                created_at,
                            },
                        )
                    },
                )
            })
            .collect::<Result<Vec<Value>, String>>()?;
        Ok((items, total))
    }

    pub fn history_counts(&self) -> Result<Value, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let total = connection
            .query_row("SELECT COUNT(*) FROM compare_history", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| error.to_string())?;
        let schema = connection
            .query_row(
                "SELECT COUNT(*) FROM compare_history WHERE sync_type = 'schema'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let data = connection
            .query_row(
                "SELECT COUNT(*) FROM compare_history WHERE sync_type = 'data'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        Ok(serde_json::json!({
            "total": total,
            "schema": schema,
            "data": data,
        }))
    }

    pub fn get_history(&self, id: &str) -> Result<Value, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let (sync_type, json): (String, String) = connection
            .query_row(
                "SELECT sync_type, result_json FROM compare_history WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| "Comparison history was not found".to_string())?;
        if sync_type == "data" {
            return slim_data_history_value(&json);
        }
        serde_json::from_str(&json).map_err(|error| error.to_string())
    }

    pub fn get_history_sql(&self, id: &str) -> Result<String, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let json: String = connection
            .query_row(
                "SELECT result_json FROM compare_history WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| "Comparison history was not found".to_string())?;
        if let Some(sql) = extract_json_string(&json, "syncSql") {
            return Ok(sql);
        }
        let value: Value = serde_json::from_str(&json).map_err(|error| error.to_string())?;
        Ok(value
            .get("syncSql")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    pub fn delete_history(&self, ids: &[String]) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        {
            let mut statement = transaction
                .prepare("DELETE FROM compare_history WHERE id = ?1")
                .map_err(|error| error.to_string())?;
            for id in ids {
                statement.execute([id]).map_err(|error| error.to_string())?;
            }
        }
        transaction.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "Storage lock failed".to_string())?;
        connection
            .execute("DELETE FROM compare_history", [])
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

struct HistorySummaryFields {
    id: String,
    db_type: Option<String>,
    source_name: Option<String>,
    target_name: Option<String>,
    title: Option<String>,
    created_at: String,
}

fn history_summary_value(
    sync_type: &str,
    summary_json: &str,
    fields: HistorySummaryFields,
) -> Result<Value, String> {
    let summary: Value = serde_json::from_str(summary_json).map_err(|error| error.to_string())?;
    let mut item = serde_json::Map::new();

    item.insert("id".into(), Value::String(fields.id));
    if let Some(value) = fields.db_type {
        item.insert("dbType".into(), Value::String(value));
    }
    if let Some(value) = fields.source_name {
        item.insert("sourceName".into(), Value::String(value));
    }
    if let Some(value) = fields.target_name {
        item.insert("targetName".into(), Value::String(value));
    }
    item.insert("summary".into(), summary);
    item.insert("syncSql".into(), Value::String(String::new()));
    item.insert("createdAt".into(), Value::String(fields.created_at));

    if sync_type == "data" {
        item.insert("runType".into(), Value::String("data".into()));
        if let Some(value) = fields.title {
            item.insert("title".into(), Value::String(value));
        }
        item.insert("runs".into(), Value::Array(Vec::new()));
    } else {
        item.insert("taskId".into(), Value::String(String::new()));
        if let Some(value) = fields.title {
            item.insert("taskName".into(), Value::String(value));
        }
        item.insert("diffs".into(), Value::Array(Vec::new()));
    }

    Ok(Value::Object(item))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlimDataHistoryRun {
    run_type: String,
    id: String,
    #[serde(default)]
    db_type: Option<String>,
    title: String,
    source_name: String,
    target_name: String,
    summary: Value,
    #[serde(default)]
    runs: Vec<SlimDataCompareRun>,
    created_at: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlimDataCompareRun {
    id: String,
    #[serde(default)]
    db_type: Option<String>,
    table_name: String,
    source_name: String,
    target_name: String,
    #[serde(default)]
    key_columns: Vec<String>,
    summary: Value,
    created_at: String,
}

fn slim_data_history_value(json: &str) -> Result<Value, String> {
    let run: SlimDataHistoryRun = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let mut value = serde_json::to_value(run).map_err(|error| error.to_string())?;
    if let Some(object) = value.as_object_mut() {
        object.insert("syncSql".into(), Value::String(String::new()));
        if let Some(runs) = object.get_mut("runs").and_then(Value::as_array_mut) {
            for item in runs {
                if let Some(run_object) = item.as_object_mut() {
                    run_object.insert("diffs".into(), Value::Array(Vec::new()));
                    run_object.insert("syncSql".into(), Value::String(String::new()));
                }
            }
        }
    }
    Ok(value)
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", key);
    let start = json.find(&pattern)? + pattern.len();
    let mut value = String::new();
    let mut escaped = false;

    for character in json[start..].chars() {
        if escaped {
            match character {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                '/' => value.push('/'),
                'b' => value.push('\u{0008}'),
                'f' => value.push('\u{000c}'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                _ => {
                    value.push('\\');
                    value.push(character);
                }
            }
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            return Some(value);
        }
        value.push(character);
    }
    None
}
