pub mod data_diff;

use std::collections::{BTreeSet, HashMap};

use crate::db::{self, ColumnMeta, DbConnection, SchemaDiff, TableMeta};

pub fn compare_schema(
    source: &DbConnection,
    target: &DbConnection,
    selected_tables: &[String],
) -> Result<Vec<SchemaDiff>, String> {
    let source_tables = db::list_tables(source)?;
    let target_tables = db::list_tables(target)?;
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
                sync_sql: Some(db::show_create_table(source, &table_name)?),
                risk_level: "medium".into(),
            }),
            (None, Some(_)) => diffs.push(SchemaDiff {
                object_type: "table".into(),
                table_name: table_name.clone(),
                column_name: None,
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some("exists".into()),
                sync_sql: Some(format!(
                    "DROP TABLE {};",
                    db::quote_identifier(target, &table_name)
                )),
                risk_level: "high".into(),
            }),
            (Some(source_table), Some(target_table)) => {
                compare_table_comment(target, source_table, target_table, &mut diffs);
                let source_columns = column_map(db::list_columns(source, &table_name)?);
                let target_columns = column_map(db::list_columns(target, &table_name)?);
                compare_columns(
                    target,
                    &table_name,
                    &source_columns,
                    &target_columns,
                    &mut diffs,
                );
            }
            (None, None) => {}
        }
    }
    if source.db_type == "postgresql" {
        compare_postgres_enum_types(source, target, &mut diffs)?;
    }

    Ok(diffs)
}

fn compare_table_comment(
    target: &DbConnection,
    source_table: &TableMeta,
    target_table: &TableMeta,
    diffs: &mut Vec<SchemaDiff>,
) {
    if target.db_type != "postgresql" || source_table.comment == target_table.comment {
        return;
    }
    diffs.push(SchemaDiff {
        object_type: "table".into(),
        table_name: source_table.name.clone(),
        column_name: Some("(comment)".into()),
        diff_type: "modified".into(),
        source_value: source_table.comment.clone(),
        target_value: target_table.comment.clone(),
        sync_sql: Some(db::postgres::comment_on_table_sql(
            &source_table.name,
            source_table.comment.as_deref(),
        )),
        risk_level: "low".into(),
    });
}

fn compare_postgres_enum_types(
    source: &DbConnection,
    target: &DbConnection,
    diffs: &mut Vec<SchemaDiff>,
) -> Result<(), String> {
    let source_types = db::postgres::list_enum_types(source)?
        .into_iter()
        .map(|type_meta| (type_meta.name.clone(), type_meta))
        .collect::<HashMap<_, _>>();
    let target_types = db::postgres::list_enum_types(target)?
        .into_iter()
        .map(|type_meta| (type_meta.name.clone(), type_meta))
        .collect::<HashMap<_, _>>();
    let type_names: BTreeSet<String> = source_types
        .keys()
        .chain(target_types.keys())
        .cloned()
        .collect();

    for type_name in type_names {
        match (source_types.get(&type_name), target_types.get(&type_name)) {
            (Some(source_type), None) => diffs.push(SchemaDiff {
                object_type: "type".into(),
                table_name: type_name.clone(),
                column_name: None,
                diff_type: "added".into(),
                source_value: Some(enum_signature(&source_type.values)),
                target_value: None,
                sync_sql: Some(db::postgres::create_enum_type_sql(source_type)),
                risk_level: "low".into(),
            }),
            (None, Some(target_type)) => diffs.push(SchemaDiff {
                object_type: "type".into(),
                table_name: type_name.clone(),
                column_name: None,
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some(enum_signature(&target_type.values)),
                sync_sql: Some(format!(
                    "DROP TYPE {};",
                    db::quote_identifier(target, &type_name)
                )),
                risk_level: "high".into(),
            }),
            (Some(source_type), Some(target_type)) if source_type.values != target_type.values => {
                let missing_values = source_type
                    .values
                    .iter()
                    .filter(|value| !target_type.values.contains(value))
                    .collect::<Vec<_>>();
                let sync_sql = if missing_values.is_empty() {
                    Some(format!(
                        "-- Enum type {} has values removed or reordered. Review manually.",
                        db::quote_identifier(target, &type_name)
                    ))
                } else {
                    Some(
                        missing_values
                            .into_iter()
                            .map(|value| {
                                format!(
                                    "ALTER TYPE {} ADD VALUE IF NOT EXISTS '{}';",
                                    db::quote_identifier(target, &type_name),
                                    value.replace('\'', "''")
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                };
                diffs.push(SchemaDiff {
                    object_type: "type".into(),
                    table_name: type_name.clone(),
                    column_name: None,
                    diff_type: "modified".into(),
                    source_value: Some(enum_signature(&source_type.values)),
                    target_value: Some(enum_signature(&target_type.values)),
                    sync_sql,
                    risk_level: "medium".into(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn enum_signature(values: &[String]) -> String {
    format!("ENUM({})", values.join(", "))
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
    db_target: &DbConnection,
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
                sync_sql: Some(column_add_sql(db_target, table_name, source)),
                risk_level: if source.nullable { "low" } else { "medium" }.into(),
            }),
            (None, Some(target_column)) => diffs.push(SchemaDiff {
                object_type: "column".into(),
                table_name: table_name.into(),
                column_name: Some(column_name.clone()),
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some(column_signature(target_column)),
                sync_sql: Some(format!(
                    "ALTER TABLE {} DROP COLUMN {};",
                    db::quote_identifier(db_target, table_name),
                    db::quote_identifier(db_target, &column_name)
                )),
                risk_level: "high".into(),
            }),
            (Some(source), Some(target_column)) => {
                if column_signature(source) != column_signature(target_column) {
                    diffs.push(SchemaDiff {
                        object_type: "column".into(),
                        table_name: table_name.into(),
                        column_name: Some(column_name.clone()),
                        diff_type: "modified".into(),
                        source_value: Some(column_signature(source)),
                        target_value: Some(column_signature(target_column)),
                        sync_sql: Some(column_modify_sql(
                            db_target,
                            table_name,
                            source,
                            target_column,
                        )),
                        risk_level: modification_risk(source, target_column).into(),
                    });
                }
                if db_target.db_type == "postgresql" && source.comment != target_column.comment {
                    diffs.push(SchemaDiff {
                        object_type: "column".into(),
                        table_name: table_name.into(),
                        column_name: Some(column_name.clone()),
                        diff_type: "modified".into(),
                        source_value: source.comment.clone(),
                        target_value: target_column.comment.clone(),
                        sync_sql: Some(db::postgres::comment_on_column_sql(
                            table_name,
                            &column_name,
                            source.comment.as_deref(),
                        )),
                        risk_level: "low".into(),
                    });
                }
            }
            _ => {}
        }
    }
}

fn column_add_sql(connection: &DbConnection, table_name: &str, source: &ColumnMeta) -> String {
    let mut statements = vec![format!(
        "ALTER TABLE {} ADD COLUMN {};",
        db::quote_identifier(connection, table_name),
        column_definition(connection, source)
    )];
    if source.is_primary_key {
        statements.push(format!(
            "ALTER TABLE {} ADD PRIMARY KEY ({});",
            db::quote_identifier(connection, table_name),
            db::quote_identifier(connection, &source.name)
        ));
    }
    if connection.db_type == "postgresql" && source.comment.is_some() {
        statements.push(db::postgres::comment_on_column_sql(
            table_name,
            &source.name,
            source.comment.as_deref(),
        ));
    }
    statements.join("\n")
}

fn column_modify_sql(
    connection: &DbConnection,
    table_name: &str,
    source: &ColumnMeta,
    target: &ColumnMeta,
) -> String {
    let table = db::quote_identifier(connection, table_name);
    let column_name = db::quote_identifier(connection, &source.name);
    let mut statements = Vec::new();

    if target.is_primary_key && !source.is_primary_key {
        statements.push(if connection.db_type == "postgresql" {
            format!("-- Drop primary key for {table} manually before modifying this column;")
        } else {
            format!("ALTER TABLE {table} DROP PRIMARY KEY;")
        });
    }

    if connection.db_type == "postgresql" {
        if source.column_type != target.column_type {
            statements.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column_name} TYPE {};",
                source.column_type
            ));
        }
        if source.nullable != target.nullable {
            statements.push(format!(
                "ALTER TABLE {table} ALTER COLUMN {column_name} {};",
                if source.nullable {
                    "DROP NOT NULL"
                } else {
                    "SET NOT NULL"
                }
            ));
        }
        if source.default_value != target.default_value {
            statements.push(match &source.default_value {
                Some(default_value) => format!(
                    "ALTER TABLE {table} ALTER COLUMN {column_name} SET DEFAULT {};",
                    default_value
                ),
                None => {
                    format!("ALTER TABLE {table} ALTER COLUMN {column_name} DROP DEFAULT;")
                }
            });
        }
    } else {
        statements.push(format!(
            "ALTER TABLE {table} MODIFY COLUMN {};",
            column_definition(connection, source)
        ));
    }

    if source.is_primary_key && !target.is_primary_key {
        statements.push(format!(
            "ALTER TABLE {table} ADD PRIMARY KEY ({});",
            db::quote_identifier(connection, &source.name)
        ));
    }

    statements.join("\n")
}

fn column_signature(column: &ColumnMeta) -> String {
    format!(
        "{}{} {} default={} primary={} extra={}",
        column.column_type,
        column
            .spatial_srid
            .map(|srid| format!(" SRID {srid}"))
            .unwrap_or_default(),
        if column.nullable { "NULL" } else { "NOT NULL" },
        column.default_value.as_deref().unwrap_or("NULL"),
        column.is_primary_key,
        column.extra.as_deref().unwrap_or("")
    )
}

fn column_definition(connection: &DbConnection, column: &ColumnMeta) -> String {
    let mut definition = format!(
        "{} {}{} {}",
        db::quote_identifier(connection, &column.name),
        column.column_type,
        if connection.db_type == "mysql" {
            column
                .spatial_srid
                .map(|srid| format!(" SRID {srid}"))
                .unwrap_or_default()
        } else {
            String::new()
        },
        if column.nullable { "NULL" } else { "NOT NULL" }
    );
    if let Some(default_value) = &column.default_value {
        definition.push_str(" DEFAULT ");
        if connection.db_type == "postgresql" {
            definition.push_str(default_value);
        } else {
            definition.push_str(&quote_default(default_value));
        }
    }
    if let Some(extra) = &column.extra {
        definition.push(' ');
        definition.push_str(extra);
    }
    definition
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
