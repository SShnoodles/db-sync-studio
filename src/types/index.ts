export type DbConnection = {
  id: string;
  name: string;
  dbType: "mysql";
  host?: string;
  port?: number;
  database: string;
  username?: string;
  password?: string;
  sslMode?: string;
  environment?: string;
  createdAt: string;
  updatedAt: string;
};

export type TableMeta = { name: string; schema?: string; tableType: string };

export type DataSyncTableMeta = {
  name: string;
  sourceExists: boolean;
  targetExists: boolean;
};

export type CompareTask = {
  id: string;
  name: string;
  sourceConnectionId: string;
  targetConnectionId: string;
  compareType: "schema";
  selectedTables: string[];
  createdAt: string;
  updatedAt: string;
};

export type SchemaDiff = {
  objectType: "table" | "column";
  tableName: string;
  columnName?: string;
  diffType: "added" | "removed" | "modified";
  sourceValue?: string;
  targetValue?: string;
  syncSql?: string;
  riskLevel: "low" | "medium" | "high";
};

export type CompareSummary = {
  totalDiffs: number;
  tableDiffs: number;
  columnDiffs: number;
  added: number;
  modified: number;
  removed: number;
  same: number;
  lowRisk: number;
  mediumRisk: number;
  highRisk: number;
};

export type CompareRun = {
  id: string;
  taskId: string;
  taskName: string;
  sourceName: string;
  targetName: string;
  summary: CompareSummary;
  diffs: SchemaDiff[];
  syncSql: string;
  createdAt: string;
};

export type HistoryRun = CompareRun | DataCompareHistoryRun;

export type HistoryFilter = {
  syncType?: "all" | "schema" | "data";
  startTime?: string;
  endTime?: string;
};

export type DataCompareRequest = {
  id: string;
  sourceConnectionId: string;
  targetConnectionId: string;
  tableName: string;
  allowDelete: boolean;
  createdAt: string;
};

export type DataCompareBatchRequest = {
  sourceConnectionId: string;
  targetConnectionId: string;
  tableNames: string[];
};

export type ChangedColumn = {
  columnName: string;
  sourceValue: unknown;
  targetValue: unknown;
};

export type DataDiff = {
  tableName: string;
  key: [string, unknown][];
  diffType: "insert" | "update" | "delete";
  sourceRow?: [string, unknown][];
  targetRow?: [string, unknown][];
  changedColumns: ChangedColumn[];
  syncSql?: string;
};

export type DataCompareSummary = {
  totalDiffs: number;
  inserts: number;
  updates: number;
  deletes: number;
  sameRows: number;
  comparedRows: number;
};

export type DataCompareRun = {
  id: string;
  tableName: string;
  sourceName: string;
  targetName: string;
  keyColumns: string[];
  summary: DataCompareSummary;
  diffs: DataDiff[];
  syncSql: string;
  createdAt: string;
};

export type DataCompareHistorySummary = {
  tables: number;
  totalDiffs: number;
  inserts: number;
  updates: number;
  deletes: number;
  sameRows: number;
  comparedRows: number;
};

export type DataCompareHistoryRun = {
  runType: "data";
  id: string;
  title: string;
  sourceName: string;
  targetName: string;
  summary: DataCompareHistorySummary;
  runs: DataCompareRun[];
  syncSql: string;
  createdAt: string;
};

export type Page =
  | "overview"
  | "connections"
  | "schemaSync"
  | "dataSync"
  | "history"
  | "settings";
