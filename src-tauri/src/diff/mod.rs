pub mod data_diff;

use std::collections::{BTreeSet, HashMap};

use crate::db::{self, ColumnMeta, DbConnection, ForeignKeyMeta, IndexMeta, SchemaDiff, TableMeta};

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
            (Some(source_table), None) => diffs.push(SchemaDiff {
                object_type: schema_object_type(source_table).into(),
                table_name: table_name.clone(),
                column_name: None,
                diff_type: "added".into(),
                source_value: Some(source_table.table_type.clone()),
                target_value: None,
                sync_sql: Some(show_create_object(source, source_table)?),
                risk_level: "medium".into(),
            }),
            (None, Some(target_table)) => diffs.push(SchemaDiff {
                object_type: schema_object_type(target_table).into(),
                table_name: table_name.clone(),
                column_name: None,
                diff_type: "removed".into(),
                source_value: None,
                target_value: Some(target_table.table_type.clone()),
                sync_sql: Some(drop_object_sql(target, target_table)),
                risk_level: "high".into(),
            }),
            (Some(source_table), Some(target_table)) => {
                if source_table.table_type != target_table.table_type {
                    diffs.push(SchemaDiff {
                        object_type: schema_object_type(source_table).into(),
                        table_name: table_name.clone(),
                        column_name: None,
                        diff_type: "modified".into(),
                        source_value: Some(source_table.table_type.clone()),
                        target_value: Some(target_table.table_type.clone()),
                        sync_sql: None,
                        risk_level: "high".into(),
                    });
                    continue;
                }
                if is_view(source_table) {
                    compare_view(source, target, source_table, &mut diffs)?;
                    continue;
                }
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
                compare_indexes(
                    target,
                    &table_name,
                    db::list_indexes(source, &table_name)?,
                    db::list_indexes(target, &table_name)?,
                    &mut diffs,
                );
                compare_foreign_keys(
                    target,
                    &table_name,
                    db::list_foreign_keys(source, &table_name)?,
                    db::list_foreign_keys(target, &table_name)?,
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

fn schema_object_type(table: &TableMeta) -> &'static str {
    if is_view(table) {
        "view"
    } else {
        "table"
    }
}

fn is_view(table: &TableMeta) -> bool {
    table.table_type.eq_ignore_ascii_case("VIEW")
}

fn show_create_object(connection: &DbConnection, object: &TableMeta) -> Result<String, String> {
    if is_view(object) {
        db::show_create_view(connection, &object.name)
    } else {
        db::show_create_table(connection, &object.name)
    }
}

fn drop_object_sql(connection: &DbConnection, object: &TableMeta) -> String {
    format!(
        "DROP {} {};",
        if is_view(object) { "VIEW" } else { "TABLE" },
        db::quote_identifier(connection, &object.name)
    )
}

fn compare_view(
    source: &DbConnection,
    target: &DbConnection,
    source_view: &TableMeta,
    diffs: &mut Vec<SchemaDiff>,
) -> Result<(), String> {
    let source_definition = db::show_create_view(source, &source_view.name)?;
    let target_definition = db::show_create_view(target, &source_view.name)?;
    if normalize_ddl(&source_definition) == normalize_ddl(&target_definition) {
        return Ok(());
    }
    diffs.push(SchemaDiff {
        object_type: "view".into(),
        table_name: source_view.name.clone(),
        column_name: None,
        diff_type: "modified".into(),
        source_value: Some(source_definition.clone()),
        target_value: Some(target_definition),
        sync_sql: (target.db_type != "sqlite").then_some(source_definition),
        risk_level: "medium".into(),
    });
    Ok(())
}

fn normalize_ddl(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(';')
        .to_ascii_lowercase()
}

fn compare_indexes(
    target: &DbConnection,
    table_name: &str,
    source_indexes: Vec<IndexMeta>,
    target_indexes: Vec<IndexMeta>,
    diffs: &mut Vec<SchemaDiff>,
) {
    let source = source_indexes
        .into_iter()
        .map(|index| (index.name.clone(), index))
        .collect::<HashMap<_, _>>();
    let target_indexes = target_indexes
        .into_iter()
        .map(|index| (index.name.clone(), index))
        .collect::<HashMap<_, _>>();
    let names = source
        .keys()
        .chain(target_indexes.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for name in names {
        match (source.get(&name), target_indexes.get(&name)) {
            (Some(source_index), None) => diffs.push(index_diff(
                table_name,
                &name,
                "added",
                Some(source_index),
                None,
                source_index.definition.clone(),
                "low",
            )),
            (None, Some(target_index)) => diffs.push(index_diff(
                table_name,
                &name,
                "removed",
                None,
                Some(target_index),
                drop_index_sql(target, table_name, target_index),
                "medium",
            )),
            (Some(source_index), Some(target_index))
                if index_signature(source_index) != index_signature(target_index) =>
            {
                let sync_sql = drop_index_sql(target, table_name, target_index).and_then(|drop| {
                    source_index
                        .definition
                        .as_ref()
                        .map(|create| format!("{drop}\n{create}"))
                });
                diffs.push(index_diff(
                    table_name,
                    &name,
                    "modified",
                    Some(source_index),
                    Some(target_index),
                    sync_sql,
                    "medium",
                ));
            }
            _ => {}
        }
    }
}

fn index_diff(
    table_name: &str,
    name: &str,
    diff_type: &str,
    source: Option<&IndexMeta>,
    target: Option<&IndexMeta>,
    sync_sql: Option<String>,
    risk_level: &str,
) -> SchemaDiff {
    SchemaDiff {
        object_type: "index".into(),
        table_name: table_name.into(),
        column_name: Some(name.into()),
        diff_type: diff_type.into(),
        source_value: source.map(index_signature),
        target_value: target.map(index_signature),
        sync_sql,
        risk_level: risk_level.into(),
    }
}

fn index_signature(index: &IndexMeta) -> String {
    if let Some(definition) = &index.definition {
        return normalize_ddl(definition);
    }
    format!(
        "{} ({})",
        if index.unique { "UNIQUE" } else { "INDEX" },
        index.columns.join(", ")
    )
}

fn drop_index_sql(
    connection: &DbConnection,
    table_name: &str,
    index: &IndexMeta,
) -> Option<String> {
    if connection.db_type == "sqlite" && index.definition.is_none() {
        return None;
    }
    Some(if connection.db_type == "mysql" {
        format!(
            "DROP INDEX {} ON {};",
            db::quote_identifier(connection, &index.name),
            db::quote_identifier(connection, table_name)
        )
    } else {
        format!(
            "DROP INDEX {};",
            db::quote_identifier(connection, &index.name)
        )
    })
}

fn compare_foreign_keys(
    target: &DbConnection,
    table_name: &str,
    source_keys: Vec<ForeignKeyMeta>,
    target_keys: Vec<ForeignKeyMeta>,
    diffs: &mut Vec<SchemaDiff>,
) {
    let source = source_keys
        .into_iter()
        .map(|key| (key.name.clone(), key))
        .collect::<HashMap<_, _>>();
    let target_keys = target_keys
        .into_iter()
        .map(|key| (key.name.clone(), key))
        .collect::<HashMap<_, _>>();
    let names = source
        .keys()
        .chain(target_keys.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for name in names {
        match (source.get(&name), target_keys.get(&name)) {
            (Some(source_key), None) => diffs.push(foreign_key_diff(
                table_name,
                &name,
                "added",
                Some(source_key),
                None,
                add_foreign_key_sql(target, table_name, source_key),
            )),
            (None, Some(target_key)) => diffs.push(foreign_key_diff(
                table_name,
                &name,
                "removed",
                None,
                Some(target_key),
                drop_foreign_key_sql(target, table_name, target_key),
            )),
            (Some(source_key), Some(target_key))
                if foreign_key_signature(source_key) != foreign_key_signature(target_key) =>
            {
                let sync_sql =
                    drop_foreign_key_sql(target, table_name, target_key).and_then(|drop| {
                        add_foreign_key_sql(target, table_name, source_key)
                            .map(|add| format!("{drop}\n{add}"))
                    });
                diffs.push(foreign_key_diff(
                    table_name,
                    &name,
                    "modified",
                    Some(source_key),
                    Some(target_key),
                    sync_sql,
                ));
            }
            _ => {}
        }
    }
}

fn foreign_key_diff(
    table_name: &str,
    name: &str,
    diff_type: &str,
    source: Option<&ForeignKeyMeta>,
    target: Option<&ForeignKeyMeta>,
    sync_sql: Option<String>,
) -> SchemaDiff {
    SchemaDiff {
        object_type: "foreignKey".into(),
        table_name: table_name.into(),
        column_name: Some(name.into()),
        diff_type: diff_type.into(),
        source_value: source.map(foreign_key_signature),
        target_value: target.map(foreign_key_signature),
        sync_sql,
        risk_level: "high".into(),
    }
}

fn foreign_key_signature(key: &ForeignKeyMeta) -> String {
    format!(
        "FOREIGN KEY ({}) REFERENCES {} ({}) ON UPDATE {} ON DELETE {}",
        key.columns.join(", "),
        key.referenced_table,
        key.referenced_columns.join(", "),
        key.on_update.as_deref().unwrap_or("NO ACTION"),
        key.on_delete.as_deref().unwrap_or("NO ACTION")
    )
}

fn add_foreign_key_sql(
    connection: &DbConnection,
    table_name: &str,
    key: &ForeignKeyMeta,
) -> Option<String> {
    if connection.db_type == "sqlite" {
        return None;
    }
    let columns = key
        .columns
        .iter()
        .map(|column| db::quote_identifier(connection, column))
        .collect::<Vec<_>>()
        .join(", ");
    let referenced_columns = key
        .referenced_columns
        .iter()
        .map(|column| db::quote_identifier(connection, column))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON UPDATE {} ON DELETE {};",
        db::quote_identifier(connection, table_name),
        db::quote_identifier(connection, &key.name),
        columns,
        db::quote_identifier(connection, &key.referenced_table),
        referenced_columns,
        key.on_update.as_deref().unwrap_or("NO ACTION"),
        key.on_delete.as_deref().unwrap_or("NO ACTION")
    ))
}

fn drop_foreign_key_sql(
    connection: &DbConnection,
    table_name: &str,
    key: &ForeignKeyMeta,
) -> Option<String> {
    if connection.db_type == "sqlite" {
        return None;
    }
    Some(format!(
        "ALTER TABLE {} DROP {} {};",
        db::quote_identifier(connection, table_name),
        if connection.db_type == "mysql" {
            "FOREIGN KEY"
        } else {
            "CONSTRAINT"
        },
        db::quote_identifier(connection, &key.name)
    ))
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
    if connection.db_type == "sqlite" && source.is_primary_key {
        return format!(
            "-- SQLite cannot add primary key column {} to {} directly. Recreate the table manually.",
            db::quote_identifier(connection, &source.name),
            db::quote_identifier(connection, table_name)
        );
    }

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

    if connection.db_type == "sqlite" {
        statements.push(format!(
            "-- SQLite cannot modify column {column_name} on {table} directly. Recreate the table manually."
        ));
        if target.is_primary_key != source.is_primary_key {
            statements.push(format!(
                "-- Primary key changes for {table} require table rebuild in SQLite."
            ));
        }
        return statements.join("\n");
    }

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
        if connection.db_type == "postgresql" || connection.db_type == "sqlite" {
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
    if source.column_type != target.column_type
        || source.nullable != target.nullable
        || source.is_primary_key != target.is_primary_key
    {
        "high"
    } else {
        "medium"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use uuid::Uuid;

    #[test]
    fn sqlite_schema_diff_matches_snapshot() {
        let source_path =
            std::env::temp_dir().join(format!("schema-source-{}.sqlite", Uuid::new_v4()));
        let target_path =
            std::env::temp_dir().join(format!("schema-target-{}.sqlite", Uuid::new_v4()));
        Connection::open(&source_path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE parents (id INTEGER PRIMARY KEY);
                 CREATE TABLE items (
                   id INTEGER PRIMARY KEY,
                   name TEXT NOT NULL DEFAULT 'new',
                   parent_id INTEGER,
                   new_col TEXT,
                   FOREIGN KEY (parent_id) REFERENCES parents(id) ON DELETE CASCADE
                 );
                 CREATE UNIQUE INDEX idx_items_name ON items(name);
                 CREATE TABLE source_only (id INTEGER PRIMARY KEY);
                 CREATE VIEW active_items AS SELECT id, name FROM items;",
            )
            .unwrap();
        Connection::open(&target_path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE parents (id INTEGER PRIMARY KEY);
                 CREATE TABLE items (
                   id INTEGER PRIMARY KEY,
                   name TEXT,
                   parent_id INTEGER,
                   legacy TEXT,
                   FOREIGN KEY (parent_id) REFERENCES parents(id) ON DELETE RESTRICT
                 );
                 CREATE INDEX idx_items_name ON items(name);
                 CREATE TABLE target_only (id INTEGER PRIMARY KEY);
                 CREATE VIEW active_items AS SELECT id FROM items;",
            )
            .unwrap();

        let diffs = compare_schema(
            &sqlite_connection(source_path.to_string_lossy().into_owned()),
            &sqlite_connection(target_path.to_string_lossy().into_owned()),
            &[],
        )
        .unwrap();
        let snapshot = serde_json::to_string_pretty(&diffs).unwrap();
        assert_eq!(snapshot, SCHEMA_DIFF_SNAPSHOT);

        std::fs::remove_file(source_path).unwrap();
        std::fs::remove_file(target_path).unwrap();
    }

    fn sqlite_connection(database: String) -> DbConnection {
        DbConnection {
            id: Uuid::new_v4().to_string(),
            name: "snapshot".into(),
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

    const SCHEMA_DIFF_SNAPSHOT: &str = r#"[
  {
    "objectType": "view",
    "tableName": "active_items",
    "columnName": null,
    "diffType": "modified",
    "sourceValue": "CREATE VIEW active_items AS SELECT id, name FROM items;",
    "targetValue": "CREATE VIEW active_items AS SELECT id FROM items;",
    "syncSql": null,
    "riskLevel": "medium"
  },
  {
    "objectType": "column",
    "tableName": "items",
    "columnName": "legacy",
    "diffType": "removed",
    "sourceValue": null,
    "targetValue": "TEXT NULL default=NULL primary=false extra=",
    "syncSql": "ALTER TABLE \"items\" DROP COLUMN \"legacy\";",
    "riskLevel": "high"
  },
  {
    "objectType": "column",
    "tableName": "items",
    "columnName": "name",
    "diffType": "modified",
    "sourceValue": "TEXT NOT NULL default='new' primary=false extra=",
    "targetValue": "TEXT NULL default=NULL primary=false extra=",
    "syncSql": "-- SQLite cannot modify column \"name\" on \"items\" directly. Recreate the table manually.",
    "riskLevel": "high"
  },
  {
    "objectType": "column",
    "tableName": "items",
    "columnName": "new_col",
    "diffType": "added",
    "sourceValue": "TEXT NULL default=NULL primary=false extra=",
    "targetValue": null,
    "syncSql": "ALTER TABLE \"items\" ADD COLUMN \"new_col\" TEXT NULL;",
    "riskLevel": "low"
  },
  {
    "objectType": "index",
    "tableName": "items",
    "columnName": "idx_items_name",
    "diffType": "modified",
    "sourceValue": "create unique index idx_items_name on items(name)",
    "targetValue": "create index idx_items_name on items(name)",
    "syncSql": "DROP INDEX \"idx_items_name\";\nCREATE UNIQUE INDEX idx_items_name ON items(name);",
    "riskLevel": "medium"
  },
  {
    "objectType": "foreignKey",
    "tableName": "items",
    "columnName": "foreign_key_0",
    "diffType": "modified",
    "sourceValue": "FOREIGN KEY (parent_id) REFERENCES parents (id) ON UPDATE NO ACTION ON DELETE CASCADE",
    "targetValue": "FOREIGN KEY (parent_id) REFERENCES parents (id) ON UPDATE NO ACTION ON DELETE RESTRICT",
    "syncSql": null,
    "riskLevel": "high"
  },
  {
    "objectType": "table",
    "tableName": "source_only",
    "columnName": null,
    "diffType": "added",
    "sourceValue": "BASE TABLE",
    "targetValue": null,
    "syncSql": "CREATE TABLE source_only (id INTEGER PRIMARY KEY);",
    "riskLevel": "medium"
  },
  {
    "objectType": "table",
    "tableName": "target_only",
    "columnName": null,
    "diffType": "removed",
    "sourceValue": null,
    "targetValue": "BASE TABLE",
    "syncSql": "DROP TABLE \"target_only\";",
    "riskLevel": "high"
  }
]"#;
}
