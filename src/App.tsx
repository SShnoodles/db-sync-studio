import { useEffect, useMemo, useState } from "react";
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
import type { CompareRun, CompareTask, DataCompareBatchRequest, DataCompareHistoryRun, DataCompareRequest, DataCompareRun, DataSyncTableMeta, DbConnection, HistoryFilter, HistoryRun, Page } from "./types";
import { blankConnection, now } from "./utils/defaults";

type ThemeMode = "light" | "dark";
type SchemaProgress = {
  kind: "compare" | "sync";
  completed: number;
  total: number;
  status?: "normal" | "active" | "success" | "exception";
};

function initialThemeMode(): ThemeMode {
  return localStorage.getItem("db-sync-studio.theme") === "dark" ? "dark" : "light";
}

function App() {
  const { antdLocale, language, setLanguage, t } = useI18n();
  const [themeMode, setThemeModeState] = useState<ThemeMode>(initialThemeMode);
  const [page, setPage] = useState<Page>("overview");
  const [connections, setConnections] = useState<DbConnection[]>([]);
  const [history, setHistory] = useState<HistoryRun[]>([]);
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
  const [syncingSchema, setSyncingSchema] = useState(false);
  const [schemaProgress, setSchemaProgress] = useState<SchemaProgress>();
  const [dataCompareProgress, setDataCompareProgress] = useState({ completed: 0, total: 0 });
  const [form] = Form.useForm<DbConnection>();
  const [taskForm] = Form.useForm<CompareTask>();
  const [dataForm] = Form.useForm<DataCompareRequest>();
  const [messageApi, contextHolder] = message.useMessage();
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

  const loadHistory = async (filter = historyFilter) => {
    try {
      setHistory(await dbSyncApi.listCompareHistory(filter));
    } catch (error) {
      messageApi.error(String(error));
    }
  };

  const searchHistory = async (filter: HistoryFilter) => {
    setHistoryFilter(filter);
    await loadHistory(filter);
  };

  useEffect(() => {
    void loadConnections();
    void loadHistory();
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
      setSchemaProgress({ kind: "compare", completed: 0, total: compareTotal, status: "active" });
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
      await loadHistory();
      setSchemaProgress({ kind: "compare", completed: compareTotal, total: compareTotal, status: "success" });
      messageApi.success(t("messages.schemaCompareCompleted", { count: run.summary.totalDiffs }));
    } catch (error) {
      setSchemaProgress((current) => ({
        kind: "compare",
        completed: current?.completed ?? 0,
        total: current?.total ?? 1,
        status: "exception",
      }));
      messageApi.error(String(error));
    } finally {
      setRunningCompare(false);
    }
  };

  const runDataCompare = async (values: DataCompareBatchRequest) => {
    try {
      setRunningDataCompare(true);
      setDataCompareProgress({ completed: 0, total: values.tableNames.length });
      const results = await Promise.all(
        values.tableNames.map((tableName) =>
          dbSyncApi
            .runDataCompare({
              id: crypto.randomUUID(),
              sourceConnectionId: values.sourceConnectionId,
              targetConnectionId: values.targetConnectionId,
              tableName,
              allowDelete: true,
              createdAt: now(),
            })
            .then((run) => ({ tableName, run }))
            .catch((error) => ({ tableName, error: String(error) }))
            .finally(() =>
              setDataCompareProgress((current) => ({
                ...current,
                completed: Math.min(current.completed + 1, current.total),
              })),
            ),
        ),
      );
      const runs = results.flatMap((result) => ("run" in result ? [result.run] : []));
      const errors = results.flatMap((result) => ("error" in result ? [`${result.tableName}: ${result.error}`] : []));
      setCurrentDataRuns(runs);
      const diffCount = runs.reduce((sum, run) => sum + run.summary.totalDiffs, 0);
      if (runs.length > 0) {
        await dbSyncApi.saveDataCompareHistory(buildDataCompareHistory(runs));
        await loadHistory();
      }
      messageApi.success(t("messages.dataCompareCompleted", { count: diffCount }));
      if (errors.length > 0) {
        messageApi.error(errors.join("\n"));
      }
    } catch (error) {
      messageApi.error(String(error));
    } finally {
      setRunningDataCompare(false);
    }
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
          setSchemaProgress({ kind: "sync", completed: 0, total: syncTotal, status: "active" });
          const result = await dbSyncApi.runSchemaSync({
            targetConnectionId,
            sql,
          });
          await loadHistory();
          await loadTaskTables(taskForm.getFieldsValue(true));
          setSchemaProgress({ kind: "sync", completed: result.executed, total: syncTotal, status: "success" });
          messageApi.success(t("messages.schemaSyncApplied", { count: result.executed }));
        } catch (error) {
          setSchemaProgress((current) => ({
            kind: "sync",
            completed: current?.completed ?? 0,
            total: current?.total ?? 1,
            status: "exception",
          }));
          messageApi.error(String(error));
        } finally {
          setSyncingSchema(false);
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
        await loadHistory(historyFilter);
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
        await loadHistory(historyFilter);
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
                  compareProgress={dataCompareProgress}
                  onRun={runDataCompare}
                  onCopySql={copySql}
                  onConnectionsChanged={(request) => void loadDataTables(request)}
                />
              )}
              {page === "history" && (
                <HistoryPage
                  history={history}
                  onCopySql={copySql}
                  onDelete={deleteHistory}
                  onClear={clearHistory}
                  onSearch={searchHistory}
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

function buildDataCompareHistory(runs: DataCompareRun[]): DataCompareHistoryRun {
  const createdAt = now();
  const firstRun = runs[0];
  return {
    runType: "data",
    id: `${firstRun.sourceName} -> ${firstRun.targetName} @ ${createdAt}`,
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

export default App;
