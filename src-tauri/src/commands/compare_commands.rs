use chrono::Utc;
use serde_json::Value;
use std::collections::HashSet;
use tauri::State;
use uuid::Uuid;

use crate::{
    db::{
        self, CompareRun, CompareSummary, CompareTask, DataCompareHistoryRun, DataCompareRequest,
        DataCompareRun,
    },
    diff,
    storage::LocalStore,
};

#[tauri::command]
pub fn list_compare_tasks(store: State<'_, LocalStore>) -> Result<Vec<CompareTask>, String> {
    store.list_tasks()
}

#[tauri::command]
pub fn save_compare_task(
    task: CompareTask,
    store: State<'_, LocalStore>,
) -> Result<CompareTask, String> {
    task.validate()?;
    store.get_connection(&task.source_connection_id)?;
    store.get_connection(&task.target_connection_id)?;
    store.save_task(&task)?;
    Ok(task)
}

#[tauri::command]
pub fn delete_compare_task(id: String, store: State<'_, LocalStore>) -> Result<(), String> {
    store.delete_task(&id)
}

#[tauri::command]
pub fn list_task_tables(
    source_id: String,
    target_id: String,
    store: State<'_, LocalStore>,
) -> Result<Vec<String>, String> {
    let source = store.get_connection(&source_id)?;
    let target = store.get_connection(&target_id)?;
    db::ensure_same_db_type(&source, &target)?;
    let mut names = db::list_tables(&source)?
        .into_iter()
        .map(|table| table.name)
        .chain(
            db::list_tables(&target)?
                .into_iter()
                .map(|table| table.name),
        )
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

#[tauri::command]
pub fn list_data_sync_tables(
    source_id: String,
    target_id: String,
    store: State<'_, LocalStore>,
) -> Result<Vec<db::DataSyncTableMeta>, String> {
    let source = store.get_connection(&source_id)?;
    let target = store.get_connection(&target_id)?;
    db::ensure_same_db_type(&source, &target)?;
    let source_names = db::list_tables(&source)?
        .into_iter()
        .map(|table| table.name)
        .collect::<HashSet<_>>();
    let target_names = db::list_tables(&target)?
        .into_iter()
        .map(|table| table.name)
        .collect::<HashSet<_>>();
    let mut names = source_names
        .union(&target_names)
        .cloned()
        .collect::<Vec<_>>();
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| db::DataSyncTableMeta {
            source_exists: source_names.contains(&name),
            target_exists: target_names.contains(&name),
            name,
        })
        .collect())
}

#[tauri::command]
pub fn run_schema_compare(
    task_id: String,
    store: State<'_, LocalStore>,
) -> Result<CompareRun, String> {
    let task = store.get_task(&task_id)?;
    task.validate()?;
    let source = store.get_connection(&task.source_connection_id)?;
    let target = store.get_connection(&task.target_connection_id)?;
    db::ensure_same_db_type(&source, &target)?;
    let diffs = diff::compare_schema(&source, &target, &task.selected_tables)?;
    let sync_sql = schema_sync_sql(&diffs);
    let run = CompareRun {
        id: Uuid::new_v4().to_string(),
        db_type: Some(source.db_type.clone()),
        task_id: task.id.clone(),
        task_name: task.name.clone(),
        source_name: source.name,
        target_name: target.name,
        summary: summarize(&diffs),
        diffs,
        sync_sql,
        created_at: Utc::now().to_rfc3339(),
    };
    store.save_history(&run)?;
    Ok(run)
}

#[tauri::command]
pub fn run_schema_compare_once(
    task: CompareTask,
    store: State<'_, LocalStore>,
) -> Result<CompareRun, String> {
    task.validate()?;
    let source = store.get_connection(&task.source_connection_id)?;
    let target = store.get_connection(&task.target_connection_id)?;
    db::ensure_same_db_type(&source, &target)?;
    let diffs = diff::compare_schema(&source, &target, &task.selected_tables)?;
    let sync_sql = schema_sync_sql(&diffs);
    let created_at = Utc::now().to_rfc3339();
    let source_label = format!("{} ({})", source.name, source.database);
    let target_label = format!("{} ({})", target.name, target.database);
    let run = CompareRun {
        id: format!(
            "{} -> {} @ {}",
            source.database, target.database, created_at
        ),
        db_type: Some(source.db_type.clone()),
        task_id: task.id,
        task_name: format!("{source_label} -> {target_label} @ {created_at}"),
        source_name: source_label,
        target_name: target_label,
        summary: summarize(&diffs),
        diffs,
        sync_sql,
        created_at,
    };
    store.save_history(&run)?;
    Ok(run)
}

#[tauri::command]
pub fn list_compare_history(
    sync_type: Option<String>,
    database_type: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    search_content: Option<String>,
    store: State<'_, LocalStore>,
) -> Result<Vec<Value>, String> {
    let sync_type = match sync_type.as_deref() {
        Some("schema") | Some("data") => sync_type,
        _ => None,
    };
    let database_type = match database_type.as_deref() {
        Some("mysql") | Some("postgresql") => database_type,
        _ => None,
    };
    let search_content = search_content
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    store.list_history(
        sync_type,
        database_type,
        start_time,
        end_time,
        search_content,
    )
}

#[tauri::command]
pub fn save_data_compare_history(
    run: DataCompareHistoryRun,
    store: State<'_, LocalStore>,
) -> Result<DataCompareHistoryRun, String> {
    store.save_data_history(&run)?;
    Ok(run)
}

#[tauri::command]
pub fn delete_compare_history(
    ids: Vec<String>,
    store: State<'_, LocalStore>,
) -> Result<(), String> {
    store.delete_history(&ids)
}

#[tauri::command]
pub fn clear_compare_history(store: State<'_, LocalStore>) -> Result<(), String> {
    store.clear_history()
}

#[tauri::command]
pub fn run_data_compare(
    request: DataCompareRequest,
    store: State<'_, LocalStore>,
) -> Result<DataCompareRun, String> {
    request.validate()?;
    let source = store.get_connection(&request.source_connection_id)?;
    let target = store.get_connection(&request.target_connection_id)?;
    db::ensure_same_db_type(&source, &target)?;
    let (key_columns, summary, diffs, _) =
        diff::data_diff::compare_data(&source, &target, &request)?;
    let sync_sql = data_sync_sql(&diffs);
    let created_at = Utc::now().to_rfc3339();
    Ok(DataCompareRun {
        id: format!(
            "{} -> {}.{} @ {}",
            source.database, target.database, request.table_name, created_at
        ),
        db_type: Some(source.db_type.clone()),
        table_name: request.table_name,
        source_name: format!("{} ({})", source.name, source.database),
        target_name: format!("{} ({})", target.name, target.database),
        key_columns,
        summary,
        diffs,
        sync_sql,
        created_at,
    })
}

fn summarize(diffs: &[db::SchemaDiff]) -> CompareSummary {
    CompareSummary {
        total_diffs: diffs.len(),
        table_diffs: diffs
            .iter()
            .filter(|diff| diff.object_type == "table")
            .count(),
        column_diffs: diffs
            .iter()
            .filter(|diff| diff.object_type == "column")
            .count(),
        added: diffs
            .iter()
            .filter(|diff| diff.diff_type == "added")
            .count(),
        modified: diffs
            .iter()
            .filter(|diff| diff.diff_type == "modified")
            .count(),
        removed: diffs
            .iter()
            .filter(|diff| diff.diff_type == "removed")
            .count(),
        same: 0,
        low_risk: diffs.iter().filter(|diff| diff.risk_level == "low").count(),
        medium_risk: diffs
            .iter()
            .filter(|diff| diff.risk_level == "medium")
            .count(),
        high_risk: diffs
            .iter()
            .filter(|diff| diff.risk_level == "high")
            .count(),
    }
}

fn schema_sync_sql(diffs: &[db::SchemaDiff]) -> String {
    let mut object_keys = diffs
        .iter()
        .filter(|diff| diff.sync_sql.is_some())
        .map(|diff| (diff.object_type.clone(), diff.table_name.clone()))
        .collect::<Vec<_>>();
    object_keys.sort_by(|left, right| {
        let left_order = schema_object_order(&left.0);
        let right_order = schema_object_order(&right.0);
        left_order
            .cmp(&right_order)
            .then_with(|| left.1.cmp(&right.1))
    });
    object_keys.dedup();

    object_keys
        .into_iter()
        .filter_map(|(object_type, object_name)| {
            let sections = [
                ("added", "Added"),
                ("modified", "Modified"),
                ("removed", "Removed"),
            ]
            .into_iter()
            .filter_map(|(diff_type, label)| {
                let statements = diffs
                    .iter()
                    .filter(|diff| {
                        diff.object_type == object_type
                            && diff.table_name == object_name
                            && diff.diff_type == diff_type
                    })
                    .filter_map(|diff| diff.sync_sql.as_deref())
                    .collect::<Vec<_>>();
                (!statements.is_empty()).then(|| {
                    format!(
                        "-- {label}: {}\n{}",
                        statements.len(),
                        statements.join("\n")
                    )
                })
            })
            .collect::<Vec<_>>();
            let object_label = if object_type == "type" {
                "Type"
            } else {
                "Table"
            };
            (!sections.is_empty())
                .then(|| format!("-- {object_label}: {object_name}\n{}", sections.join("\n")))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn schema_object_order(object_type: &str) -> u8 {
    match object_type {
        "type" => 0,
        "table" | "column" => 1,
        _ => 2,
    }
}

fn data_sync_sql(diffs: &[db::DataDiff]) -> String {
    let mut table_names = diffs
        .iter()
        .filter(|diff| diff.sync_sql.is_some())
        .map(|diff| diff.table_name.clone())
        .collect::<Vec<_>>();
    table_names.sort();
    table_names.dedup();

    table_names
        .into_iter()
        .filter_map(|table_name| {
            let sections = [
                ("insert", "Insert"),
                ("update", "Update"),
                ("delete", "Delete"),
            ]
            .into_iter()
            .filter_map(|(diff_type, label)| {
                let statements = diffs
                    .iter()
                    .filter(|diff| diff.table_name == table_name && diff.diff_type == diff_type)
                    .filter_map(|diff| diff.sync_sql.as_deref())
                    .collect::<Vec<_>>();
                (!statements.is_empty()).then(|| {
                    format!(
                        "-- {label}: {}\n{}",
                        statements.len(),
                        statements.join("\n")
                    )
                })
            })
            .collect::<Vec<_>>();
            (!sections.is_empty())
                .then(|| format!("-- Table: {table_name}\n{}", sections.join("\n")))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
