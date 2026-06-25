import type { CompareTask, DataCompareRequest, DbConnection } from "../types";

export const now = () => new Date().toISOString();

export const blankConnection = (): DbConnection => ({
  id: crypto.randomUUID(),
  name: "",
  dbType: "mysql",
  host: "127.0.0.1",
  port: 3306,
  database: "",
  username: "root",
  password: "",
  sslMode: "prefer",
  environment: "Development",
  createdAt: now(),
  updatedAt: now(),
});

export const blankTask = (): CompareTask => ({
  id: crypto.randomUUID(),
  name: "",
  sourceConnectionId: "",
  targetConnectionId: "",
  compareType: "schema",
  selectedTables: [],
  createdAt: now(),
  updatedAt: now(),
});

export const blankDataCompare = (): DataCompareRequest => ({
  id: crypto.randomUUID(),
  sourceConnectionId: "",
  targetConnectionId: "",
  tableName: "",
  allowDelete: false,
  createdAt: now(),
});
