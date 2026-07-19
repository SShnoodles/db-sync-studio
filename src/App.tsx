import { useEffect, useMemo, useRef, useState } from "react";
import { App as AntApp, ConfigProvider, Form, Layout, Menu, Modal, message, theme as antdTheme } from "antd";
import type { MenuProps } from "antd";
import { ApiOutlined, DatabaseOutlined, HistoryOutlined, SettingOutlined, SwapOutlined, TableOutlined } from "@ant-design/icons";
import "antd/dist/reset.css";
import "./App.css";

import { dbSyncApi } from "./api/dbSyncApi";
import { ComingSoon } from "./components/ComingSoon";
import { useI18n } from "./i18n";
import { ConnectionsPage } from "./pages/ConnectionsPage";
import { DataSyncPage } from "./pages/DataSyncPage";
import { HistoryPage } from "./pages/HistoryPage";
import { OverviewPage } from "./pages/OverviewPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TasksPage } from "./pages/TasksPage";
import type { CompareRun, CompareTask, DataCompareBatchRequest, DataCompareHistoryRequest, DataCompareRequest, DataCompareRun, DataSyncTableMeta, DbConnection, HistoryCounts, HistoryFilter, HistoryRun, Page } from "./types";
import { blankConnection, now } from "./utils/defaults";

type ThemeMode = "light" | "dark";
type SchemaProgress = {
  kind: "compare" | "sync";
  completed: number;
  total: number;
  startedAt: number;
  finishedAt?: number;
  status?: "normal" | "active" | "success" | "exception";
};
type DataProgress = {
  kind: "compare" | "sync";
  completed: number;
  total: number;
  startedAt: number;
  finishedAt?: number;
  status?: "normal" | "active" | "success" | "exception";
};
type CooperativeCancellation = { cancelled: boolean };

const DATA_COMPARE_CONCURRENCY = 3;

function initialThemeMode(): ThemeMode {
  return localStorage.getItem("db-sync-studio.theme") === "dark" ? "dark" : "light";
}

function App() {
  const { antdLocale, language, setLanguage, t } = useI18n();
  const [themeMode, setThemeModeState] = useState<ThemeMode>(initialThemeMode);
  const [page, setPage] = useState<Page>("overview");
  const [connections, setConnections] = useState<DbConnection[]>([]);
  const [history, setHistory] = useState<HistoryRun[]>([]);
  const [historyTotal, setHistoryTotal] = useState(0);
  const [historyPage, setHistoryPage] = useState(1);
  const [historyCounts, setHistoryCounts] = useState<HistoryCounts>({ total: 0, schema: 0, data: 0 });
  const [historyFilter, setHistoryFilter] = useState<HistoryFilter>({ syncType: "all" });
  const [selectedId, setSelectedId] = useState<string>();
  const [taskTables, setTaskTables] = useState<string[]>([]);
  const [dataTables, setDataTables] = useState<DataSyncTableMeta[]>([]);
  const [currentRun, setCurrentRun] = useState<CompareRun>();
  const [currentDataRuns, setCurrentDataRuns] = useState<DataCompareRun[]>([]);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [loadingTaskTables, setLoadingTaskTables] = useState(false);
  const [loadingDataTables, setLoadingDataTables] = useState(false);
  const [runningCompare, setRunningCompare] = useState(false);
  const [runningDataCompare, setRunningDataCompare] = useState(false);
  const [dataCompareCancelRequested, setDataCompareCancelRequested] = useState(false);
  const [syncingSchema, setSyncingSchema] = useState(false);
  const [syncingData, setSyncingData] = useState(false);
  const [schemaProgress, setSchemaProgress] = useState<SchemaProgress>();
  const [dataProgress, setDataProgress] = useState<DataProgress>();
  const [form] = Form.useForm<DbConnection>();
  const [taskForm] = Form.useForm<CompareTask>();
  const [dataForm] = Form.useForm<DataCompareRequest>();
  const [messageApi, contextHolder] = message.useMessage();
  const dataCompareCancellationRef = useRef<CooperativeCancellation | undefined>(undefined);
  const menuItems: MenuProps["items"] = [
    { key: "overview", icon: <ApiOutlined />, label: t("menu.overview") },
    { key: "connections", icon: <DatabaseOutlined />, label: t("menu.connections") },
    { key: "schemaSync", icon: <SwapOutlined />, label: t("menu.schemaSync") },
    { key: "dataSync", icon: <TableOutlined />, label: t("menu.dataSync") },
    { key: "history", icon: <HistoryOutlined />, label: t("menu.history") },
    { key: "settings", icon: <SettingOutlined />, label: t("menu.settings") },
  ];
  const setThemeMode = (nextThemeMode: ThemeMode) => {
    localStorage.setItem("db-sync-studio.theme", nextThemeMode);
    setThemeModeState(nextThemeMode);
  };

  const selected = useMemo(
    () => connections.find((item) => item.id === selectedId),
    [connections, selectedId],
  );
  const loadConnections = async () => {
    try {
      setConnections(await dbSyncApi.listConnections());
    } catch (error) {
      messageApi.error(String(error));
    }
  };

  const historyPageSize = 3;
  const loadHistory = async (
    filter = historyFilter,
    nextPage = historyPage,
    nextPageSize = historyPageSize,
  ) => {
    try {
      const result = await dbSyncApi.listCompareHistory({
        ...filter,
        page: nextPage,
        pageSize: nextPageSize,
      });
      setHistory(result.items);
      setHistoryTotal(result.total);
      setHistoryPage(nextPage);
    } catch (error) {
      messageApi.error(String(error));
    }
  };

  const loadHistoryCounts = async () => {
    try {
      setHistoryCounts(await dbSyncApi.getCompareHistoryCounts());
    } catch (error) {
      messageApi.error(String(error));
    }
  };

  const searchHistory = async (filter: HistoryFilter) => {
    setHistoryFilter(filter);
    await loadHistory(filter, 1);
  };

  const changeHistoryPage = async (nextPage: number) => {
    await loadHistory(historyFilter, nextPage);
  };

  const loadHistoryDetail = async (id: string) => dbSyncApi.getCompareHistory(id);
  const loadHistorySql = async (id: string) => dbSyncApi.getCompareHistorySql(id);

  useEffect(() => {
    void loadConnections();
    void loadHistory();
    void loadHistoryCounts();
  }, []);

  const loadIntoForm = (connection: DbConnection) => {
    setSelectedId(connection.id);
    form.setFieldsValue(connection);
  };

  const createConnection = () => {
    setSelectedId(undefined);
    form.setFieldsValue(blankConnection());
    setPage("connections");
  };

  const testConnection = async () => {
    try {
      setTesting(true);
      const result = await dbSyncApi.testConnection(form.getFieldsValue(true));
      messageApi.success(result);
    } catch (error) {
      messageApi.error(String(error));
    } finally {
      setTesting(false);
    }
  };

  const saveConnection = async (values: DbConnection) => {
    try {
      setSaving(true);
      const connection = {
        ...values,
        id: selectedId || values.id || crypto.randomUUID(),
        dbType: values.dbType || ("mysql" as const),
        createdAt: selected?.createdAt || values.createdAt || now(),
        updatedAt: now(),
      };
      const saved = await dbSyncApi.saveConnection(connection);
      await loadConnections();
      loadIntoForm(saved);
      messageApi.success(t("messages.connectionSaved"));
    } catch (error) {
      messageApi.error(String(error));
    } finally {
      setSaving(false);
    }
  };

  const removeConnection = () =>
    Modal.confirm({
      title: t("modal.deleteConnectionTitle"),
      content: t("modal.deleteConnectionContent"),
      okText: t("common.delete"),
      okButtonProps: { danger: true },
      onOk: async () => {
        if (!selectedId) return;
        await dbSyncApi.deleteConnection(selectedId);
        createConnection();
        await loadConnections();
        messageApi.success(t("messages.connectionDeleted"));
      },
    });

  const loadTaskTables = async (task: Partial<CompareTask>) => {
    if (!task.sourceConnectionId || !task.targetConnectionId) return;
    try {
      setLoadingTaskTables(true);
      setTaskTables(await dbSyncApi.listTaskTables(task.sourceConnectionId, task.targetConnectionId));
    } catch (error) {
      messageApi.error(String(error));
    } finally {
      setLoadingTaskTables(false);
    }
  };

  const loadDataTables = async (request: Partial<DataCompareRequest>) => {
    if (!request.sourceConnectionId || !request.targetConnectionId) return;
    try {
      setLoadingDataTables(true);
      setDataTables(await dbSyncApi.listDataSyncTables(request.sourceConnectionId, request.targetConnectionId));
    } catch (error) {
      messageApi.error(String(error));
    } finally {
      setLoadingDataTables(false);
    }
  };

  const runSchemaCompare = async (values: CompareTask) => {
    try {
      setRunningCompare(true);
      const source = connections.find((item) => item.id === values.sourceConnectionId);
      const target = connections.find((item) => item.id === values.targetConnectionId);
      const compareTotal = Math.max(values.selectedTables?.length || taskTables.length || 1, 1);
      const startedAt = Date.now();
      setSchemaProgress({ kind: "compare", completed: 0, total: compareTotal, startedAt, status: "active" });
      const task = {
        ...values,
        id: crypto.randomUUID(),
        name: `${source?.name || t("common.source")} -> ${target?.name || t("common.target")} @ ${now()}`,
        compareType: "schema" as const,
        selectedTables: values.selectedTables || [],
        createdAt: now(),
        updatedAt: now(),
      };
      const run = await dbSyncApi.runSchemaCompareOnce(task);
      setCurrentRun(run);
      await loadHistory(historyFilter, historyPage);
      await loadHistoryCounts();
      setSchemaProgress({ kind: "compare", completed: compareTotal, total: compareTotal, startedAt, finishedAt: Date.now(), status: "success" });
      messageApi.success(t("messages.schemaCompareCompleted", { count: run.summary.totalDiffs }));
    } catch (error) {
      setSchemaProgress((current) => ({
        kind: "compare",
        completed: current?.completed ?? 0,
        total: current?.total ?? 1,
        startedAt: current?.startedAt ?? Date.now(),
        finishedAt: Date.now(),
        status: "exception",
      }));
      messageApi.error(String(error));
    } finally {
      setRunningCompare(false);
    }
  };

  const runDataCompare = async (values: DataCompareBatchRequest) => {
    const cancellation = { cancelled: false };
    dataCompareCancellationRef.current = cancellation;
    try {
      setRunningDataCompare(true);
      setDataCompareCancelRequested(false);
      const startedAt = Date.now();
      setDataProgress({ kind: "compare", completed: 0, total: values.tableNames.length, startedAt, status: "active" });
      const results = await mapWithConcurrency(
        values.tableNames,
        DATA_COMPARE_CONCURRENCY,
        cancellation,
        async (tableName) => {
          const result = await dbSyncApi
            .runDataCompare({
              sourceConnectionId: values.sourceConnectionId,
              targetConnectionId: values.targetConnectionId,
              tableName,
              allowDelete: values.allowDelete,
            })
            .then((run) => ({ tableName, run }))
            .catch((error) => ({ tableName, error: String(error) }));
          setDataProgress((current) => ({
            ...current,
            kind: "compare",
            status: cancellation.cancelled ? "normal" : "active",
            total: values.tableNames.length,
            startedAt,
            completed: Math.min((current?.completed ?? 0) + 1, current?.total ?? values.tableNames.length),
          }));
          return result;
        },
      );
      const runs = results.flatMap((result) => ("run" in result ? [result.run] : []));
      const errors = results.flatMap((result) => ("error" in result ? [`${result.tableName}: ${result.error}`] : []));
      setCurrentDataRuns(runs);
      const diffCount = runs.reduce((sum, run) => sum + run.summary.totalDiffs, 0);
      if (runs.length > 0) {
        await dbSyncApi.saveDataCompareHistory(buildDataCompareHistory(runs));
        await loadHistory(historyFilter, historyPage);
        await loadHistoryCounts();
      }
      setDataProgress((current) => ({
        kind: "compare",
        completed: current?.completed ?? results.length,
        total: values.tableNames.length,
        startedAt,
        finishedAt: Date.now(),
        status: cancellation.cancelled ? "normal" : "success",
      }));
      if (cancellation.cancelled) {
        messageApi.info(t("messages.dataCompareCancelled", { completed: results.length, total: values.tableNames.length }));
      } else {
        messageApi.success(t("messages.dataCompareCompleted", { count: diffCount }));
      }
      if (errors.length > 0) {
        messageApi.error(errors.join("\n"));
      }
    } catch (error) {
      setDataProgress((current) => ({
        kind: "compare",
        completed: current?.completed ?? 0,
        total: current?.total ?? 1,
        startedAt: current?.startedAt ?? Date.now(),
        finishedAt: Date.now(),
        status: "exception",
      }));
      messageApi.error(String(error));
    } finally {
      if (dataCompareCancellationRef.current === cancellation) {
        dataCompareCancellationRef.current = undefined;
      }
      setDataCompareCancelRequested(false);
      setRunningDataCompare(false);
    }
  };

  const cancelDataCompare = () => {
    const cancellation = dataCompareCancellationRef.current;
    if (!cancellation || cancellation.cancelled) return;
    cancellation.cancelled = true;
    setDataCompareCancelRequested(true);
    setDataProgress((current) => current ? { ...current, status: "normal" } : current);
  };

  const copySql = async (sql: string) => {
    if (!sql.trim()) {
      messageApi.info(t("messages.noSqlToCopy"));
      return;
    }
    await navigator.clipboard.writeText(sql);
    messageApi.success(t("messages.sqlCopied"));
  };

  const runSchemaSync = (sql: string) => {
    if (!sql.trim()) {
      messageApi.info(t("messages.noSqlToSync"));
      return;
    }
    const targetConnectionId = taskForm.getFieldValue("targetConnectionId");
    if (!targetConnectionId) {
      messageApi.error(t("schema.selectTarget"));
      return;
    }
    Modal.confirm({
      title: t("modal.schemaSyncTitle"),
      content: t("modal.schemaSyncContent"),
      okText: t("schema.sync"),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          setSyncingSchema(true);
          const syncTotal = Math.max(countSqlStatements(sql), 1);
          const startedAt = Date.now();
          setSchemaProgress({ kind: "sync", completed: 0, total: syncTotal, startedAt, status: "active" });
          const result = await dbSyncApi.runSchemaSync({
            targetConnectionId,
            sql,
          });
          await loadHistory(historyFilter, historyPage);
          await loadHistoryCounts();
          await loadTaskTables(taskForm.getFieldsValue(true));
          setSchemaProgress({ kind: "sync", completed: result.executed, total: syncTotal, startedAt, finishedAt: Date.now(), status: "success" });
          messageApi.success(t("messages.schemaSyncApplied", { count: result.executed }));
        } catch (error) {
          setSchemaProgress((current) => ({
            kind: "sync",
            completed: current?.completed ?? 0,
            total: current?.total ?? 1,
            startedAt: current?.startedAt ?? Date.now(),
            finishedAt: Date.now(),
            status: "exception",
          }));
          messageApi.error(String(error));
        } finally {
          setSyncingSchema(false);
        }
      },
    });
  };

  const runDataSync = (sql: string) => {
    if (!sql.trim()) {
      messageApi.info(t("messages.noSqlToSync"));
      return;
    }
    const targetConnectionId = dataForm.getFieldValue("targetConnectionId");
    if (!targetConnectionId) {
      messageApi.error(t("schema.selectTarget"));
      return;
    }
    Modal.confirm({
      title: t("modal.dataSyncTitle"),
      content: t("modal.dataSyncContent"),
      okText: t("schema.sync"),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          setSyncingData(true);
          const syncTotal = Math.max(countSqlStatements(sql), 1);
          const startedAt = Date.now();
          setDataProgress({ kind: "sync", completed: 0, total: syncTotal, startedAt, status: "active" });
          const result = await dbSyncApi.runDataSync({
            targetConnectionId,
            sql,
          });
          await loadHistory(historyFilter, historyPage);
          await loadHistoryCounts();
          await loadDataTables(dataForm.getFieldsValue(true));
          setDataProgress({ kind: "sync", completed: result.executed, total: syncTotal, startedAt, finishedAt: Date.now(), status: "success" });
          messageApi.success(t("messages.dataSyncApplied", { count: result.executed }));
        } catch (error) {
          setDataProgress((current) => ({
            kind: "sync",
            completed: current?.completed ?? 0,
            total: current?.total ?? 1,
            startedAt: current?.startedAt ?? Date.now(),
            finishedAt: Date.now(),
            status: "exception",
          }));
          messageApi.error(String(error));
        } finally {
          setSyncingData(false);
        }
      },
    });
  };

  const deleteHistory = (ids: string[]) =>
    Modal.confirm({
      title: t("modal.deleteHistoryTitle"),
      content: t("modal.deleteHistoryContent", { count: ids.length }),
      okText: t("common.delete"),
      okButtonProps: { danger: true },
      onOk: async () => {
        await dbSyncApi.deleteCompareHistory(ids);
        const nextPage = history.length === ids.length && historyPage > 1 ? historyPage - 1 : historyPage;
        await loadHistory(historyFilter, nextPage);
        await loadHistoryCounts();
        messageApi.success(t("messages.historyDeleted"));
      },
    });

  const clearHistory = () =>
    Modal.confirm({
      title: t("modal.clearHistoryTitle"),
      content: t("modal.clearHistoryContent"),
      okText: t("history.clearAll"),
      okButtonProps: { danger: true },
      onOk: async () => {
        await dbSyncApi.clearCompareHistory();
        await loadHistory(historyFilter, 1);
        await loadHistoryCounts();
        messageApi.success(t("messages.historyCleared"));
      },
    });

  return (
    <ConfigProvider
      locale={antdLocale}
      componentSize="small"
      theme={{
        algorithm: themeMode === "dark" ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm,
        token: {
          colorPrimary: "#1677ff",
          borderRadius: 5,
          borderRadiusLG: 6,
          fontSize: 12,
          controlHeight: 28,
          controlHeightSM: 24,
          controlHeightLG: 32,
          margin: 12,
          padding: 12,
          fontFamily: "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        },
        components: { Layout: { siderBg: themeMode === "dark" ? "#111827" : "#ffffff" } },
      }}
    >
      <AntApp>
        {contextHolder}
        <Layout className="app-layout" data-theme={themeMode}>
          <Layout.Sider width={208} className="app-sider" breakpoint="lg" collapsedWidth="0">
            <div className="brand">
              <img className="brand-icon" src="/app-icon.png" alt="DB Sync Studio" />
              <span>DB Sync Studio</span>
            </div>
            <Menu
              theme={themeMode === "dark" ? "dark" : "light"}
              mode="inline"
              selectedKeys={[page]}
              items={menuItems}
              onClick={({ key }) => setPage(key as Page)}
            />
          </Layout.Sider>
          <Layout>
            <Layout.Content className="app-content">
              {page === "overview" && (
                <OverviewPage
                  connections={connections}
                  history={history}
                  historyCounts={historyCounts}
                  onCreate={createConnection}
                  onConnections={() => setPage("connections")}
                />
              )}
              {page === "connections" && (
                <ConnectionsPage
                  connections={connections}
                  selectedId={selectedId}
                  selected={selected}
                  form={form}
                  saving={saving}
                  testing={testing}
                  onSelect={loadIntoForm}
                  onCreate={createConnection}
                  onSave={saveConnection}
                  onTest={testConnection}
                  onDelete={removeConnection}
                />
              )}
              {page === "schemaSync" && (
                <TasksPage
                  connections={connections}
                  form={taskForm}
                  tableOptions={taskTables}
                  currentRun={currentRun}
                  loadingTables={loadingTaskTables}
                  runningCompare={runningCompare}
                  syncingSchema={syncingSchema}
                  schemaProgress={schemaProgress}
                  onRun={runSchemaCompare}
                  onCopySql={copySql}
                  onSyncSql={runSchemaSync}
                  onConnectionsChanged={(task) => void loadTaskTables(task)}
                />
              )}
              {page === "dataSync" && (
                <DataSyncPage
                  connections={connections}
                  form={dataForm}
                  tableOptions={dataTables}
                  currentRuns={currentDataRuns}
                  loadingTables={loadingDataTables}
                  runningCompare={runningDataCompare}
                  syncingData={syncingData}
                  progress={dataProgress}
                  cancelRequested={dataCompareCancelRequested}
                  onRun={runDataCompare}
                  onCancel={cancelDataCompare}
                  onCopySql={copySql}
                  onSyncSql={runDataSync}
                  onConnectionsChanged={(request) => void loadDataTables(request)}
                />
              )}
              {page === "history" && (
                <HistoryPage
                  history={history}
                  total={historyTotal}
                  page={historyPage}
                  pageSize={historyPageSize}
                  onCopySql={copySql}
                  onLoadDetail={loadHistoryDetail}
                  onLoadSql={loadHistorySql}
                  onDelete={deleteHistory}
                  onClear={clearHistory}
                  onSearch={searchHistory}
                  onPageChange={changeHistoryPage}
                />
              )}
              {page === "settings" && (
                <SettingsPage
                  language={language}
                  themeMode={themeMode}
                  onLanguageChange={setLanguage}
                  onThemeModeChange={setThemeMode}
                />
              )}
              {!(["overview", "connections", "schemaSync", "dataSync", "history", "settings"] as string[]).includes(page) && (
                <ComingSoon page={page} />
              )}
            </Layout.Content>
          </Layout>
        </Layout>
      </AntApp>
    </ConfigProvider>
  );
}

function buildDataCompareHistory(runs: DataCompareRun[]): DataCompareHistoryRequest {
  const createdAt = now();
  const firstRun = runs[0];
  return {
    dbType: firstRun.dbType,
    title: `${firstRun.sourceName} -> ${firstRun.targetName} @ ${createdAt}`,
    sourceName: firstRun.sourceName,
    targetName: firstRun.targetName,
    summary: {
      tables: runs.length,
      totalDiffs: runs.reduce((sum, run) => sum + run.summary.totalDiffs, 0),
      inserts: runs.reduce((sum, run) => sum + run.summary.inserts, 0),
      updates: runs.reduce((sum, run) => sum + run.summary.updates, 0),
      deletes: runs.reduce((sum, run) => sum + run.summary.deletes, 0),
      sameRows: runs.reduce((sum, run) => sum + run.summary.sameRows, 0),
      comparedRows: runs.reduce((sum, run) => sum + run.summary.comparedRows, 0),
    },
    runs,
    syncSql: runs
      .map((run) => run.syncSql.trim())
      .filter(Boolean)
      .join("\n\n"),
    createdAt,
  };
}

function countSqlStatements(sql: string) {
  return sql
    .split(";")
    .map((statement) => statement.replace(/--.*$/gm, "").trim())
    .filter(Boolean).length;
}

async function mapWithConcurrency<T, R>(
  items: T[],
  concurrency: number,
  cancellation: CooperativeCancellation,
  mapper: (item: T) => Promise<R>,
) {
  const results: R[] = [];
  let nextIndex = 0;
  const worker = async () => {
    while (!cancellation.cancelled) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) return;
      results[index] = await mapper(items[index]);
    }
  };
  const workerCount = Math.min(Math.max(concurrency, 1), items.length);
  await Promise.all(Array.from({ length: workerCount }, () => worker()));
  return results.filter((result) => result !== undefined);
}

export default App;
