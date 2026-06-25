pub mod data_diff;

use std::collections::{BTreeSet, HashMap};

use crate::db::{mysql, ColumnMeta, DbConnection, SchemaDiff, TableMeta};

pub fn compare_schema(
    source: &DbConnection,
    target: &DbConnection,
    selected_tables: &[String],
) -> Result<Vec<SchemaDiff>, String> {
    let source_tables = mysql::list_tables(source)?;
    let target_tables = mysql::list_tables(target)?;
    let source_table_map = table_map(source_tables);
    let target_table_map = table_map(target_tables);
    let requested_tables =
        selected_table_set(selected_tables, &source_table_map, &target_table_map);
    let mut diffs = Vec::new();

    for table_name in requested_tables {
        match (
            source_table_map.get(&table_name),
            target_table_map.get(&table_name),
        ) {
            (Some(_), None) => diffs.push(SchemaDiff {
                object_type: "table".into(),
                table_name: table_name.clone(),
                column_name: None,
                diff_type: "added".into(),
                source_value: Some("exists".into()),
                target_value: None,
                sync_sql: Some(mysql::show_create_table(source, &table_name)?),
                risk_level: "medium".into(),
            }),
            (None, Some(_)) => diffs.push(SchemaDiff {
                object_type: "table".into(),
                table_name: table_name.clone(),
                column_name: None,
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some("exists".into()),
                sync_sql: Some(format!("DROP TABLE `{}`;", escape_identifier(&table_name))),
                risk_level: "high".into(),
            }),
            (Some(_), Some(_)) => {
                let source_columns = column_map(mysql::list_columns(source, &table_name)?);
                let target_columns = column_map(mysql::list_columns(target, &table_name)?);
                compare_columns(&table_name, &source_columns, &target_columns, &mut diffs);
            }
            (None, None) => {}
        }
    }

    Ok(diffs)
}

fn table_map(tables: Vec<TableMeta>) -> HashMap<String, TableMeta> {
    tables
        .into_iter()
        .map(|table| (table.name.clone(), table))
        .collect()
}

fn column_map(columns: Vec<ColumnMeta>) -> HashMap<String, ColumnMeta> {
    columns
        .into_iter()
        .map(|column| (column.name.clone(), column))
        .collect()
}

fn selected_table_set(
    selected_tables: &[String],
    source: &HashMap<String, TableMeta>,
    target: &HashMap<String, TableMeta>,
) -> BTreeSet<String> {
    if !selected_tables.is_empty() {
        return selected_tables.iter().cloned().collect();
    }

    source.keys().chain(target.keys()).cloned().collect()
}

fn compare_columns(
    table_name: &str,
    source_columns: &HashMap<String, ColumnMeta>,
    target_columns: &HashMap<String, ColumnMeta>,
    diffs: &mut Vec<SchemaDiff>,
) {
    let column_names: BTreeSet<String> = source_columns
        .keys()
        .chain(target_columns.keys())
        .cloned()
        .collect();

    for column_name in column_names {
        match (
            source_columns.get(&column_name),
            target_columns.get(&column_name),
        ) {
            (Some(source), None) => diffs.push(SchemaDiff {
                object_type: "column".into(),
                table_name: table_name.into(),
                column_name: Some(column_name.clone()),
                diff_type: "added".into(),
                source_value: Some(column_signature(source)),
                target_value: None,
                sync_sql: Some(
                    format!(
                        "ALTER TABLE `{}` ADD COLUMN {};",
                        escape_identifier(table_name),
                        column_definition(source)
                    ) + &primary_key_add_sql(table_name, source),
                ),
                risk_level: if source.nullable { "low" } else { "medium" }.into(),
            }),
            (None, Some(target)) => diffs.push(SchemaDiff {
                object_type: "column".into(),
                table_name: table_name.into(),
                column_name: Some(column_name.clone()),
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some(column_signature(target)),
                sync_sql: Some(format!(
                    "ALTER TABLE `{}` DROP COLUMN `{}`;",
                    escape_identifier(table_name),
                    escape_identifier(&column_name)
                )),
                risk_level: "high".into(),
            }),
            (Some(source), Some(target))
                if column_signature(source) != column_signature(target) =>
            {
                diffs.push(SchemaDiff {
                    object_type: "column".into(),
                    table_name: table_name.into(),
                    column_name: Some(column_name.clone()),
                    diff_type: "modified".into(),
                    source_value: Some(column_signature(source)),
                    target_value: Some(column_signature(target)),
                    sync_sql: Some(column_modify_sql(table_name, source, target)),
                    risk_level: modification_risk(source, target).into(),
                });
            }
            _ => {}
        }
    }
}

fn column_modify_sql(table_name: &str, source: &ColumnMeta, target: &ColumnMeta) -> String {
    let table = escape_identifier(table_name);
    let mut statements = Vec::new();

    if target.is_primary_key && !source.is_primary_key {
        statements.push(format!("ALTER TABLE `{table}` DROP PRIMARY KEY;"));
    }

    statements.push(format!(
        "ALTER TABLE `{table}` MODIFY COLUMN {};",
        column_definition(source)
    ));

    if source.is_primary_key && !target.is_primary_key {
        statements.push(format!(
            "ALTER TABLE `{table}` ADD PRIMARY KEY (`{}`);",
            escape_identifier(&source.name)
        ));
    }

    statements.join("\n")
}

fn primary_key_add_sql(table_name: &str, source: &ColumnMeta) -> String {
    if !source.is_primary_key {
        return String::new();
    }

    format!(
        "\nALTER TABLE `{}` ADD PRIMARY KEY (`{}`);",
        escape_identifier(table_name),
        escape_identifier(&source.name)
    )
}

fn column_signature(column: &ColumnMeta) -> String {
    format!(
        "{} {} default={} primary={} extra={}",
        column.column_type,
        if column.nullable { "NULL" } else { "NOT NULL" },
        column.default_value.as_deref().unwrap_or("NULL"),
        column.is_primary_key,
        column.extra.as_deref().unwrap_or("")
    )
}

fn column_definition(column: &ColumnMeta) -> String {
    let mut definition = format!(
        "`{}` {} {}",
        escape_identifier(&column.name),
        column.column_type,
        if column.nullable { "NULL" } else { "NOT NULL" }
    );
    if let Some(default_value) = &column.default_value {
        definition.push_str(" DEFAULT ");
        definition.push_str(&quote_default(default_value));
    }
    if let Some(extra) = &column.extra {
        definition.push(' ');
        definition.push_str(extra);
    }
    definition
}

fn escape_identifier(value: &str) -> String {
    value.replace('`', "``")
}

fn quote_default(default_value: &str) -> String {
    let upper = default_value.to_ascii_uppercase();
    if upper == "NULL"
        || upper == "CURRENT_TIMESTAMP"
        || upper.starts_with("CURRENT_TIMESTAMP(")
        || upper.starts_with("B'")
        || default_value.parse::<f64>().is_ok()
    {
        default_value.to_string()
    } else {
        format!("'{}'", default_value.replace('\'', "''"))
    }
}

fn modification_risk(source: &ColumnMeta, target: &ColumnMeta) -> &'static str {
    if source.column_type != target.column_type || source.nullable != target.nullable {
        "high"
    } else if source.is_primary_key != target.is_primary_key {
        "high"
    } else {
        "medium"
    }
}
