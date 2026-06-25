use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::db::{
    mysql, ChangedColumn, DataCompareRequest, DataCompareSummary, DataDiff, DbConnection,
};

const ROW_LIMIT: usize = 5000;

pub fn compare_data(
    source: &DbConnection,
    target: &DbConnection,
    request: &DataCompareRequest,
) -> Result<(Vec<String>, DataCompareSummary, Vec<DataDiff>, String), String> {
    let key_columns = mysql::primary_keys(source, &request.table_name)?;
    if key_columns.is_empty() {
        return Err(
            "The selected table has no primary key. Manual business keys are not supported yet."
                .into(),
        );
    }

    let source_rows = mysql::fetch_rows(source, &request.table_name, &key_columns, ROW_LIMIT)?;
    let target_rows = mysql::fetch_rows(target, &request.table_name, &key_columns, ROW_LIMIT)?;
    let source_map = rows_by_key(&source_rows, &key_columns)?;
    let target_map = rows_by_key(&target_rows, &key_columns)?;
    let mut diffs = Vec::new();
    let mut same_rows = 0;

    for (key, source_row) in &source_map {
        match target_map.get(key) {
            None => diffs.push(insert_diff(&request.table_name, &key_columns, source_row)),
            Some(target_row) => {
                let changes = changed_columns(source_row, target_row, &key_columns);
                if !changes.is_empty() {
                    diffs.push(update_diff(
                        &request.table_name,
                        &key_columns,
                        source_row,
                        target_row,
                        changes,
                    ));
                } else {
                    same_rows += 1;
                }
            }
        }
    }

    for (key, target_row) in &target_map {
        if !source_map.contains_key(key) {
            diffs.push(delete_diff(
                &request.table_name,
                &key_columns,
                target_row,
                request.allow_delete,
            ));
        }
    }

    let sync_sql = diffs
        .iter()
        .filter_map(|diff| diff.sync_sql.as_deref())
        .collect::<Vec<_>>()
        .join("\n");
    let summary = DataCompareSummary {
        total_diffs: diffs.len(),
        inserts: diffs
            .iter()
            .filter(|diff| diff.diff_type == "insert")
            .count(),
        updates: diffs
            .iter()
            .filter(|diff| diff.diff_type == "update")
            .count(),
        deletes: diffs
            .iter()
            .filter(|diff| diff.diff_type == "delete")
            .count(),
        same_rows,
        compared_rows: source_rows.len().max(target_rows.len()),
    };

    Ok((key_columns, summary, diffs, sync_sql))
}

fn rows_by_key(
    rows: &[BTreeMap<String, Value>],
    key_columns: &[String],
) -> Result<HashMap<String, BTreeMap<String, Value>>, String> {
    let mut map = HashMap::new();
    for row in rows {
        let key = key_string(row, key_columns)?;
        map.insert(key, row.clone());
    }
    Ok(map)
}

fn insert_diff(
    table_name: &str,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(source_row, key_columns),
        diff_type: "insert".into(),
        source_row: Some(row_pairs(source_row)),
        target_row: None,
        changed_columns: Vec::new(),
        sync_sql: Some(insert_sql(table_name, source_row)),
    }
}

fn update_diff(
    table_name: &str,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
    target_row: &BTreeMap<String, Value>,
    changed_columns: Vec<ChangedColumn>,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(source_row, key_columns),
        diff_type: "update".into(),
        source_row: Some(row_pairs(source_row)),
        target_row: Some(row_pairs(target_row)),
        sync_sql: Some(update_sql(
            table_name,
            key_columns,
            source_row,
            &changed_columns,
        )),
        changed_columns,
    }
}

fn delete_diff(
    table_name: &str,
    key_columns: &[String],
    target_row: &BTreeMap<String, Value>,
    allow_delete: bool,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(target_row, key_columns),
        diff_type: "delete".into(),
        source_row: None,
        target_row: Some(row_pairs(target_row)),
        changed_columns: Vec::new(),
        sync_sql: allow_delete.then(|| delete_sql(table_name, key_columns, target_row)),
    }
}

fn changed_columns(
    source_row: &BTreeMap<String, Value>,
    target_row: &BTreeMap<String, Value>,
    key_columns: &[String],
) -> Vec<ChangedColumn> {
    source_row
        .iter()
        .filter(|(column, _)| !key_columns.contains(column))
        .filter_map(|(column, source_value)| {
            let target_value = target_row.get(column).unwrap_or(&Value::Null);
            (source_value != target_value).then(|| ChangedColumn {
                column_name: column.clone(),
                source_value: source_value.clone(),
                target_value: target_value.clone(),
            })
        })
        .collect()
}

fn key_string(row: &BTreeMap<String, Value>, key_columns: &[String]) -> Result<String, String> {
    key_columns
        .iter()
        .map(|column| {
            row.get(column)
                .map(value_key)
                .ok_or_else(|| format!("Primary key column `{column}` was not found in row"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("\u{1f}"))
}

fn key_pairs(row: &BTreeMap<String, Value>, key_columns: &[String]) -> Vec<(String, Value)> {
    key_columns
        .iter()
        .map(|column| {
            (
                column.clone(),
                row.get(column).cloned().unwrap_or(Value::Null),
            )
        })
        .collect()
}

fn row_pairs(row: &BTreeMap<String, Value>) -> Vec<(String, Value)> {
    row.iter()
        .map(|(column, value)| (column.clone(), value.clone()))
        .collect()
}

fn insert_sql(table_name: &str, row: &BTreeMap<String, Value>) -> String {
    let columns = row
        .keys()
        .map(|column| format!("`{}`", escape_identifier(column)))
        .collect::<Vec<_>>()
        .join(", ");
    let values = row.values().map(sql_value).collect::<Vec<_>>().join(", ");
    format!(
        "INSERT INTO `{}` ({columns}) VALUES ({values});",
        escape_identifier(table_name)
    )
}

fn update_sql(
    table_name: &str,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
    changed_columns: &[ChangedColumn],
) -> String {
    let sets = changed_columns
        .iter()
        .map(|change| {
            format!(
                "`{}` = {}",
                escape_identifier(&change.column_name),
                sql_value(&change.source_value)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "UPDATE `{}` SET {sets} WHERE {};",
        escape_identifier(table_name),
        where_clause(key_columns, source_row)
    )
}

fn delete_sql(
    table_name: &str,
    key_columns: &[String],
    target_row: &BTreeMap<String, Value>,
) -> String {
    format!(
        "DELETE FROM `{}` WHERE {};",
        escape_identifier(table_name),
        where_clause(key_columns, target_row)
    )
}

fn where_clause(key_columns: &[String], row: &BTreeMap<String, Value>) -> String {
    key_columns
        .iter()
        .map(|column| {
            format!(
                "`{}` <=> {}",
                escape_identifier(column),
                sql_value(row.get(column).unwrap_or(&Value::Null))
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn sql_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::Bool(value) => {
            if *value {
                "1".into()
            } else {
                "0".into()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''")),
        Value::Array(_) | Value::Object(_) => {
            format!("'{}'", value.to_string().replace('\'', "''"))
        }
    }
}

fn value_key(value: &Value) -> String {
    match value {
        Value::Null => "<NULL>".into(),
        _ => value.to_string(),
    }
}

fn escape_identifier(value: &str) -> String {
    value.replace('`', "``")
}
