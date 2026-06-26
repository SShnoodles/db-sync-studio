use tauri::State;

use crate::{db, storage::LocalStore};

#[tauri::command]
pub fn list_connections(store: State<'_, LocalStore>) -> Result<Vec<db::DbConnection>, String> {
    store.list_connections()
}

#[tauri::command]
pub fn save_connection(
    connection: db::DbConnection,
    store: State<'_, LocalStore>,
) -> Result<db::DbConnection, String> {
    connection.validate()?;
    store.save_connection(&connection)?;
    Ok(connection)
}

#[tauri::command]
pub fn delete_connection(id: String, store: State<'_, LocalStore>) -> Result<(), String> {
    store.delete_connection(&id)
}

#[tauri::command]
pub fn test_connection(connection: db::DbConnection) -> Result<String, String> {
    let detail = db::test_connection(&connection)?;
    Ok(detail)
}

#[tauri::command]
pub fn list_tables(id: String, store: State<'_, LocalStore>) -> Result<Vec<db::TableMeta>, String> {
    let connection = store.get_connection(&id)?;
    db::list_tables(&connection)
}
