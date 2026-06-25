mod commands;
mod db;
mod diff;
mod sqlgen;
mod storage;

use storage::LocalStore;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = LocalStore::open(data_dir.join("db-sync-studio.sqlite"))
                .map_err(|error| std::io::Error::other(error))?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connection_commands::list_connections,
            commands::connection_commands::save_connection,
            commands::connection_commands::delete_connection,
            commands::connection_commands::test_connection,
            commands::connection_commands::list_tables,
            commands::compare_commands::list_compare_tasks,
            commands::compare_commands::save_compare_task,
            commands::compare_commands::delete_compare_task,
            commands::compare_commands::list_task_tables,
            commands::compare_commands::list_data_sync_tables,
            commands::compare_commands::run_schema_compare,
            commands::compare_commands::run_schema_compare_once,
            commands::compare_commands::list_compare_history,
            commands::compare_commands::save_data_compare_history,
            commands::compare_commands::delete_compare_history,
            commands::compare_commands::clear_compare_history,
            commands::compare_commands::run_data_compare,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
