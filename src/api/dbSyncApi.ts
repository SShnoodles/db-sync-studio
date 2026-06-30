import { invoke } from "@tauri-apps/api/core";

import type { CompareRun, CompareTask, DataCompareHistoryRun, DataCompareRequest, DataCompareRun, DataSyncTableMeta, DbConnection, HistoryFilter, HistoryRun, SchemaSyncRequest, SchemaSyncResult, TableMeta } from "../types";

export const dbSyncApi = {
  listConnections: () => invoke<DbConnection[]>("list_connections"),
  saveConnection: (connection: DbConnection) =>
    invoke<DbConnection>("save_connection", { connection }),
  deleteConnection: (id: string) => invoke("delete_connection", { id }),
  testConnection: (connection: DbConnection) =>
    invoke<string>("test_connection", { connection }),
  listTables: (id: string) => invoke<TableMeta[]>("list_tables", { id }),

  listTaskTables: (sourceId: string, targetId: string) =>
    invoke<string[]>("list_task_tables", { sourceId, targetId }),
  listDataSyncTables: (sourceId: string, targetId: string) =>
    invoke<DataSyncTableMeta[]>("list_data_sync_tables", { sourceId, targetId }),
  runSchemaCompareOnce: (task: CompareTask) =>
    invoke<CompareRun>("run_schema_compare_once", { task }),
  runSchemaSync: (request: SchemaSyncRequest) =>
    invoke<SchemaSyncResult>("run_schema_sync", { request }),
  listCompareHistory: (filter: HistoryFilter = {}) => invoke<HistoryRun[]>("list_compare_history", filter),
  deleteCompareHistory: (ids: string[]) =>
    invoke("delete_compare_history", { ids }),
  clearCompareHistory: () => invoke("clear_compare_history"),
  runDataCompare: (request: DataCompareRequest) =>
    invoke<DataCompareRun>("run_data_compare", { request }),
  saveDataCompareHistory: (run: DataCompareHistoryRun) =>
    invoke<DataCompareHistoryRun>("save_data_compare_history", { run }),
};
