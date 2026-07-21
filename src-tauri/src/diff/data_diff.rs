use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::db::{
    self, ChangedColumn, DataCompareRequest, DataCompareSummary, DataDiff, DbConnection,
};

const ROW_BATCH_SIZE: usize = 2_000;
const MAX_COMPARE_ROWS: usize = 100_000;

pub fn compare_data(
    source: &DbConnection,
    target: &DbConnection,
    request: &DataCompareRequest,
) -> Result<(Vec<String>, DataCompareSummary, Vec<DataDiff>, String), String> {
    let key_columns = db::primary_keys(source, &request.table_name)?;
    if key_columns.is_empty() {
        return Err(
            "The selected table has no primary key. Manual business keys are not supported yet."
                .into(),
        );
    }
    let target_key_columns = db::primary_keys(target, &request.table_name)?;

    let source_columns = db::list_columns(source, &request.table_name)?;
    let target_columns = db::list_columns(target, &request.table_name)?;
    validate_schema_compatibility(
        source,
        &source_columns,
        &target_columns,
        &key_columns,
        &target_key_columns,
    )?;
    let target_column_map = target_columns
        .iter()
        .map(|column| (column.name.clone(), column.clone()))
        .collect::<HashMap<_, _>>();
    let source_column_order = source_columns
        .iter()
        .filter(|column| {
            target_column_map
                .get(&column.name)
                .is_none_or(|target_column| !is_generated_column(target_column))
        })
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let target_column_order = target_columns
        .iter()
        .filter(|column| !is_generated_column(column))
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let (source_rows, source_truncated) =
        fetch_rows_in_batches(source, &request.table_name, &key_columns)?;
    let (target_rows, target_truncated) =
        fetch_rows_in_batches(target, &request.table_name, &key_columns)?;
    let truncated = source_truncated || target_truncated;
    let source_map = rows_by_key(&source_rows, &key_columns)?;
    let target_map = rows_by_key(&target_rows, &key_columns)?;
    let mut diffs = Vec::new();
    let mut same_rows = 0;

    for (key, source_row) in &source_map {
        match target_map.get(key) {
            None => diffs.push(insert_diff(
                target,
                &request.table_name,
                &key_columns,
                source_row,
                &target_column_order,
                &target_column_map,
            )),
            Some(target_row) => {
                let changes =
                    changed_columns(source_row, target_row, &key_columns, &source_column_order);
                if !changes.is_empty() {
                    diffs.push(update_diff(
                        &request.table_name,
                        target,
                        &key_columns,
                        source_row,
                        target_row,
                        changes,
                        &target_column_map,
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
                target,
                &target_column_map,
            ));
        }
    }

    if truncated {
        for diff in &mut diffs {
            diff.sync_sql = None;
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
        source_rows: source_rows.len(),
        target_rows: target_rows.len(),
        truncated,
    };

    Ok((key_columns, summary, diffs, sync_sql))
}

fn validate_schema_compatibility(
    connection: &DbConnection,
    source_columns: &[db::ColumnMeta],
    target_columns: &[db::ColumnMeta],
    source_keys: &[String],
    target_keys: &[String],
) -> Result<(), String> {
    let mut issues = Vec::new();
    if source_keys != target_keys {
        issues.push(format!(
            "primary keys differ (source: [{}], target: [{}])",
            source_keys.join(", "),
            target_keys.join(", ")
        ));
    }

    let source_map = source_columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();
    let target_map = target_columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect::<HashMap<_, _>>();

    for source_column in source_columns {
        let Some(target_column) = target_map.get(source_column.name.as_str()) else {
            issues.push(format!(
                "source column `{}` does not exist in the target",
                source_column.name
            ));
            continue;
        };
        if canonical_column_type(&connection.db_type, &source_column.column_type)
            != canonical_column_type(&connection.db_type, &target_column.column_type)
        {
            issues.push(format!(
                "column `{}` has incompatible types (source: {}, target: {})",
                source_column.name, source_column.column_type, target_column.column_type
            ));
        }
    }

    for target_column in target_columns {
        if source_map.contains_key(target_column.name.as_str())
            || target_column.nullable
            || target_column.default_value.is_some()
            || target_column
                .extra
                .as_deref()
                .is_some_and(|extra| extra.to_ascii_lowercase().contains("auto_increment"))
            || is_generated_column(target_column)
        {
            continue;
        }
        issues.push(format!(
            "required target column `{}` has no source value or default",
            target_column.name
        ));
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Data synchronization preflight failed: {}",
            issues.join("; ")
        ))
    }
}

fn canonical_column_type(db_type: &str, column_type: &str) -> String {
    let normalized = column_type
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if db_type == "sqlite" {
        if normalized.contains("int") {
            return "integer".into();
        }
        if normalized.contains("char") || normalized.contains("clob") || normalized.contains("text")
        {
            return "text".into();
        }
        if normalized.contains("blob") || normalized.is_empty() {
            return "blob".into();
        }
        if normalized.contains("real") || normalized.contains("floa") || normalized.contains("doub")
        {
            return "real".into();
        }
        return "numeric".into();
    }
    if db_type == "mysql" {
        for integer_type in [
            "tinyint",
            "smallint",
            "mediumint",
            "int",
            "integer",
            "bigint",
        ] {
            if normalized.starts_with(&format!("{integer_type}(")) {
                let suffix = normalized
                    .split_once(')')
                    .map(|(_, suffix)| suffix.trim())
                    .unwrap_or_default();
                return format!("{integer_type} {suffix}").trim().to_string();
            }
        }
    }
    normalized
}

fn is_generated_column(column: &db::ColumnMeta) -> bool {
    column
        .extra
        .as_deref()
        .is_some_and(|extra| extra.to_ascii_lowercase().contains("generated"))
}

fn fetch_rows_in_batches(
    connection: &DbConnection,
    table: &str,
    key_columns: &[String],
) -> Result<(Vec<BTreeMap<String, Value>>, bool), String> {
    let mut rows = Vec::new();
    let mut offset = 0;

    loop {
        let remaining = MAX_COMPARE_ROWS.saturating_sub(rows.len());
        let fetch_limit = remaining.min(ROW_BATCH_SIZE) + usize::from(remaining <= ROW_BATCH_SIZE);
        let mut batch = db::fetch_rows(connection, table, key_columns, fetch_limit, offset)?;

        if batch.len() > remaining {
            batch.truncate(remaining);
            rows.extend(batch);
            return Ok((rows, true));
        }

        let batch_len = batch.len();
        rows.extend(batch);
        if batch_len < fetch_limit {
            return Ok((rows, false));
        }
        if rows.len() >= MAX_COMPARE_ROWS {
            return Ok((rows, true));
        }
        offset += batch_len;
    }
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
    target: &DbConnection,
    table_name: &str,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
    column_order: &[String],
    column_map: &HashMap<String, db::ColumnMeta>,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(source_row, key_columns),
        diff_type: "insert".into(),
        source_row: Some(row_pairs(source_row)),
        target_row: None,
        changed_columns: Vec::new(),
        sync_sql: Some(insert_sql(
            target,
            table_name,
            source_row,
            column_order,
            column_map,
        )),
    }
}

fn update_diff(
    table_name: &str,
    target: &DbConnection,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
    target_row: &BTreeMap<String, Value>,
    changed_columns: Vec<ChangedColumn>,
    column_map: &HashMap<String, db::ColumnMeta>,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(source_row, key_columns),
        diff_type: "update".into(),
        source_row: Some(row_pairs(source_row)),
        target_row: Some(row_pairs(target_row)),
        sync_sql: Some(update_sql(
            target,
            table_name,
            key_columns,
            source_row,
            &changed_columns,
            column_map,
        )),
        changed_columns,
    }
}

fn delete_diff(
    table_name: &str,
    key_columns: &[String],
    target_row: &BTreeMap<String, Value>,
    allow_delete: bool,
    target: &DbConnection,
    column_map: &HashMap<String, db::ColumnMeta>,
) -> DataDiff {
    DataDiff {
        table_name: table_name.into(),
        key: key_pairs(target_row, key_columns),
        diff_type: "delete".into(),
        source_row: None,
        target_row: Some(row_pairs(target_row)),
        changed_columns: Vec::new(),
        sync_sql: allow_delete
            .then(|| delete_sql(target, table_name, key_columns, target_row, column_map)),
    }
}

fn changed_columns(
    source_row: &BTreeMap<String, Value>,
    target_row: &BTreeMap<String, Value>,
    key_columns: &[String],
    column_order: &[String],
) -> Vec<ChangedColumn> {
    column_order
        .iter()
        .filter(|column| !key_columns.contains(column))
        .filter_map(|column| {
            let source_value = source_row.get(column).unwrap_or(&Value::Null);
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

fn insert_sql(
    target: &DbConnection,
    table_name: &str,
    row: &BTreeMap<String, Value>,
    column_order: &[String],
    column_map: &HashMap<String, db::ColumnMeta>,
) -> String {
    let selected_columns = column_order
        .iter()
        .filter(|column| row.contains_key(*column))
        .collect::<Vec<_>>();
    let columns = selected_columns
        .iter()
        .map(|column| db::quote_identifier(target, column))
        .collect::<Vec<_>>()
        .join(", ");
    let values = selected_columns
        .iter()
        .map(|column| {
            sql_value(
                target,
                row.get(*column).unwrap_or(&Value::Null),
                column_map.get(*column),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO {} ({columns}) VALUES ({values});",
        db::quote_identifier(target, table_name)
    )
}

fn update_sql(
    target: &DbConnection,
    table_name: &str,
    key_columns: &[String],
    source_row: &BTreeMap<String, Value>,
    changed_columns: &[ChangedColumn],
    column_map: &HashMap<String, db::ColumnMeta>,
) -> String {
    let sets = changed_columns
        .iter()
        .map(|change| {
            format!(
                "{} = {}",
                db::quote_identifier(target, &change.column_name),
                sql_value(
                    target,
                    &change.source_value,
                    column_map.get(&change.column_name),
                )
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "UPDATE {} SET {sets} WHERE {};",
        db::quote_identifier(target, table_name),
        where_clause(target, key_columns, source_row, column_map)
    )
}

fn delete_sql(
    target: &DbConnection,
    table_name: &str,
    key_columns: &[String],
    target_row: &BTreeMap<String, Value>,
    column_map: &HashMap<String, db::ColumnMeta>,
) -> String {
    format!(
        "DELETE FROM {} WHERE {};",
        db::quote_identifier(target, table_name),
        where_clause(target, key_columns, target_row, column_map)
    )
}

fn where_clause(
    target: &DbConnection,
    key_columns: &[String],
    row: &BTreeMap<String, Value>,
    column_map: &HashMap<String, db::ColumnMeta>,
) -> String {
    key_columns
        .iter()
        .map(|column| {
            format!(
                "{} {} {}",
                db::quote_identifier(target, column),
                db::null_safe_eq_operator(target),
                sql_value(
                    target,
                    row.get(column).unwrap_or(&Value::Null),
                    column_map.get(column),
                )
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn sql_value(target: &DbConnection, value: &Value, column: Option<&db::ColumnMeta>) -> String {
    if target.db_type == "postgresql" {
        if let Some(column) = column {
            return postgres_sql_value(value, &column.column_type);
        }
    }
    if target.db_type == "mysql" {
        if let Some(column) = column {
            return mysql_sql_value(value, column);
        }
    }
    if target.db_type == "sqlite" {
        if let Some(column) = column {
            return sqlite_sql_value(value, column);
        }
    }

    match value {
        Value::Null => "NULL".into(),
        Value::Bool(value) => {
            if target.db_type == "postgresql" {
                if *value {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            } else if *value {
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

fn sqlite_sql_value(value: &Value, column: &db::ColumnMeta) -> String {
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
        Value::Array(_) | Value::Object(_) => quoted_sqlite_string(&value.to_string()),
        Value::String(value) => {
            let lower_type = column.column_type.to_ascii_lowercase();
            if is_sqlite_blob_type(&lower_type) {
                sqlite_hex_literal(value)
            } else if is_sqlite_number_type(&lower_type) && is_plain_number(value) {
                value.clone()
            } else {
                quoted_sqlite_string(value)
            }
        }
    }
}

fn quoted_sqlite_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sqlite_hex_literal(value: &str) -> String {
    let hex = value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    if hex.is_empty() {
        "X''".into()
    } else {
        format!("X'{}'", hex.to_ascii_uppercase())
    }
}

fn is_sqlite_blob_type(column_type: &str) -> bool {
    column_type.contains("blob")
}

fn is_sqlite_number_type(column_type: &str) -> bool {
    let base_type = sqlite_base_type(column_type);
    matches!(
        base_type.as_str(),
        "integer"
            | "int"
            | "bigint"
            | "smallint"
            | "tinyint"
            | "real"
            | "double"
            | "float"
            | "numeric"
            | "decimal"
            | "boolean"
    )
}

fn sqlite_base_type(column_type: &str) -> String {
    column_type
        .split(|character: char| character == '(' || character.is_whitespace())
        .next()
        .unwrap_or(column_type)
        .to_string()
}

fn mysql_sql_value(value: &Value, column: &db::ColumnMeta) -> String {
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
        Value::Array(_) | Value::Object(_) => quoted_mysql_string(&value.to_string()),
        Value::String(value) => {
            let lower_type = column.column_type.to_ascii_lowercase();
            if is_mysql_bit_type(&lower_type) {
                mysql_bit_literal(value)
            } else if is_mysql_binary_type(&lower_type) {
                mysql_hex_literal(value)
            } else if is_mysql_spatial_type(&lower_type) {
                mysql_spatial_literal(value, column.spatial_srid)
            } else if is_mysql_number_type(&lower_type) && is_plain_number(value) {
                value.clone()
            } else {
                quoted_mysql_string(value)
            }
        }
    }
}

fn mysql_spatial_literal(value: &str, spatial_srid: Option<u32>) -> String {
    let wkb = mysql_hex_literal(value);
    match spatial_srid {
        Some(srid) => format!("ST_GeomFromWKB({wkb}, {srid})"),
        None => format!("ST_GeomFromWKB({wkb})"),
    }
}

fn quoted_mysql_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "''"))
}

fn mysql_hex_literal(value: &str) -> String {
    let hex = value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    if hex.is_empty() {
        "X''".into()
    } else {
        format!("X'{}'", hex.to_ascii_uppercase())
    }
}

fn mysql_bit_literal(value: &str) -> String {
    let hex = value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    if hex.is_empty() {
        return "b''".into();
    }
    let bits = hex
        .as_bytes()
        .chunks(2)
        .filter_map(|chunk| std::str::from_utf8(chunk).ok())
        .filter_map(|chunk| u8::from_str_radix(chunk, 16).ok())
        .map(|byte| format!("{byte:08b}"))
        .collect::<Vec<_>>()
        .join("");
    format!("b'{bits}'")
}

fn is_plain_number(value: &str) -> bool {
    value.parse::<i128>().is_ok() || value.parse::<f64>().is_ok()
}

fn is_mysql_number_type(column_type: &str) -> bool {
    let base_type = mysql_base_type(column_type);
    matches!(
        base_type.as_str(),
        "tinyint"
            | "smallint"
            | "mediumint"
            | "int"
            | "integer"
            | "bigint"
            | "decimal"
            | "numeric"
            | "float"
            | "double"
            | "real"
            | "year"
    )
}

fn is_mysql_bit_type(column_type: &str) -> bool {
    mysql_base_type(column_type) == "bit"
}

fn is_mysql_binary_type(column_type: &str) -> bool {
    matches!(
        mysql_base_type(column_type).as_str(),
        "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob"
    )
}

fn is_mysql_spatial_type(column_type: &str) -> bool {
    matches!(
        mysql_base_type(column_type).as_str(),
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

fn mysql_base_type(column_type: &str) -> String {
    column_type
        .split(|character: char| character == '(' || character.is_whitespace())
        .next()
        .unwrap_or(column_type)
        .to_string()
}

fn postgres_sql_value(value: &Value, column_type: &str) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::Array(items) => postgres_array_literal(items, column_type),
        Value::Object(_) => format!(
            "{}::{}",
            quoted_postgres_string(&value.to_string()),
            column_type
        ),
        Value::Bool(value) => {
            let literal = if *value { "TRUE" } else { "FALSE" };
            cast_postgres_literal(literal.into(), column_type)
        }
        Value::Number(value) => cast_postgres_literal(value.to_string(), column_type),
        Value::String(value) => {
            if is_plain_postgres_string_type(column_type) {
                quoted_postgres_string(value)
            } else {
                format!("{}::{}", quoted_postgres_string(value), column_type)
            }
        }
    }
}

fn postgres_array_literal(items: &[Value], column_type: &str) -> String {
    let values = items
        .iter()
        .map(|item| match item {
            Value::Null => "NULL".into(),
            Value::Bool(value) => {
                if *value {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            }
            Value::Number(value) => value.to_string(),
            Value::String(value) => quoted_postgres_string(value),
            Value::Array(_) | Value::Object(_) => quoted_postgres_string(&item.to_string()),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("ARRAY[{values}]::{column_type}")
}

fn cast_postgres_literal(literal: String, column_type: &str) -> String {
    if is_plain_postgres_number_type(column_type) || column_type == "boolean" {
        literal
    } else {
        format!("{literal}::{column_type}")
    }
}

fn quoted_postgres_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn is_plain_postgres_string_type(column_type: &str) -> bool {
    column_type == "text"
        || column_type.starts_with("character varying")
        || column_type.starts_with("character(")
        || column_type.starts_with("char(")
        || column_type == "name"
}

fn is_plain_postgres_number_type(column_type: &str) -> bool {
    matches!(
        column_type,
        "smallint"
            | "integer"
            | "bigint"
            | "smallserial"
            | "serial"
            | "bigserial"
            | "real"
            | "double precision"
            | "numeric"
            | "decimal"
            | "money"
            | "oid"
    ) || column_type.starts_with("numeric(")
        || column_type.starts_with("decimal(")
}

fn value_key(value: &Value) -> String {
    match value {
        Value::Null => "<NULL>".into(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_reports_primary_key_type_and_required_column_issues() {
        let source = vec![column("id", "integer", false, true, None)];
        let target = vec![
            column("id", "text", false, true, None),
            column("tenant_id", "integer", false, true, None),
        ];
        let error = validate_schema_compatibility(
            &connection("sqlite"),
            &source,
            &target,
            &["id".into()],
            &["tenant_id".into(), "id".into()],
        )
        .unwrap_err();

        assert!(error.contains("primary keys differ"));
        assert!(error.contains("column `id` has incompatible types"));
        assert!(error.contains("required target column `tenant_id`"));
    }

    #[test]
    fn sqlite_affinity_compatible_types_pass_preflight() {
        let source = vec![column("id", "INT", false, true, None)];
        let target = vec![column("id", "INTEGER", false, true, None)];

        validate_schema_compatibility(
            &connection("sqlite"),
            &source,
            &target,
            &["id".into()],
            &["id".into()],
        )
        .unwrap();
    }

    #[test]
    fn escapes_string_literals_for_each_database_dialect() {
        let injection = "O'Reilly'); DROP TABLE users; --";
        assert_eq!(
            quoted_mysql_string(&format!("path\\{injection}")),
            "'path\\\\O''Reilly''); DROP TABLE users; --'"
        );
        assert_eq!(
            quoted_postgres_string(injection),
            "'O''Reilly''); DROP TABLE users; --'"
        );
        assert_eq!(
            quoted_sqlite_string(injection),
            "'O''Reilly''); DROP TABLE users; --'"
        );
    }

    #[test]
    fn emits_safe_binary_and_null_literals() {
        assert_eq!(mysql_hex_literal("00 af-10"), "X'00AF10'");
        assert_eq!(sqlite_hex_literal("de:ad:be:ef"), "X'DEADBEEF'");
        assert_eq!(
            sql_value(&connection("postgresql"), &Value::Null, None),
            "NULL"
        );
    }

    fn column(
        name: &str,
        column_type: &str,
        nullable: bool,
        primary: bool,
        default_value: Option<&str>,
    ) -> db::ColumnMeta {
        db::ColumnMeta {
            table_name: "items".into(),
            name: name.into(),
            column_type: column_type.into(),
            nullable,
            default_value: default_value.map(Into::into),
            is_primary_key: primary,
            extra: None,
            ordinal_position: 1,
            comment: None,
            spatial_srid: None,
        }
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
