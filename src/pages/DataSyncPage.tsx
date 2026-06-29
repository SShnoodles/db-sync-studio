import { useEffect, useMemo, useState } from "react";
import { Alert, Button, Card, Checkbox, Col, Empty, Form, Progress, Row, Select, Space, Table, Typography } from "antd";
import type { TableColumnsType } from "antd";
import { PlayCircleOutlined } from "@ant-design/icons";

import { SqlCodePreview } from "../components/SqlCodePreview";
import { useI18n } from "../i18n";
import type { DataCompareBatchRequest, DataCompareRequest, DataCompareRun, DataSyncTableMeta, DbConnection } from "../types";
import { connectionOptionLabel } from "../utils/connection";
import { blankDataCompare } from "../utils/defaults";

type DataOperation = "insert" | "update" | "delete";

type OperationSelection = Record<DataOperation, boolean>;

type DataSyncRow = {
  tableName: string;
  sourceExists: boolean;
  targetExists: boolean;
  run?: DataCompareRun;
};

const emptyOperationSelection: OperationSelection = {
  insert: false,
  update: false,
  delete: false,
};

export function DataSyncPage({
  connections,
  form,
  tableOptions,
  currentRuns,
  loadingTables,
  runningCompare,
  compareProgress,
  onReset,
  onRun,
  onCopySql,
  onConnectionsChanged,
}: {
  connections: DbConnection[];
  form: ReturnType<typeof Form.useForm<DataCompareRequest>>[0];
  tableOptions: DataSyncTableMeta[];
  currentRuns: DataCompareRun[];
  loadingTables: boolean;
  runningCompare: boolean;
  compareProgress: { completed: number; total: number };
  onReset: () => void;
  onRun: (values: DataCompareBatchRequest) => void;
  onCopySql: (sql: string) => void;
  onConnectionsChanged: (request: Partial<DataCompareRequest>) => void;
}) {
  const { t } = useI18n();
  const [selectedTables, setSelectedTables] = useState<React.Key[]>([]);
  const [operationSelection, setOperationSelection] = useState<Record<string, OperationSelection>>({});

  const connectionOptions = connections.map((connection) => ({
    value: connection.id,
    label: connectionOptionLabel(connection),
  }));
  const selectableTables = useMemo(
    () => tableOptions.filter((table) => table.sourceExists && table.targetExists).map((table) => table.name),
    [tableOptions],
  );
  const tableMetaByName = useMemo(
    () => new Map(tableOptions.map((table) => [table.name, table])),
    [tableOptions],
  );

  useEffect(() => {
    setSelectedTables(selectableTables);
  }, [selectableTables]);

  useEffect(() => {
    setOperationSelection((current) => {
      const next = { ...current };
      currentRuns.forEach((run) => {
        if (next[run.tableName]) return;
        next[run.tableName] = {
          insert: run.summary.inserts > 0,
          update: run.summary.updates > 0,
          delete: run.summary.deletes > 0,
        };
      });
      return next;
    });
  }, [currentRuns]);

  const runByTable = useMemo(
    () => new Map(currentRuns.map((run) => [run.tableName, run])),
    [currentRuns],
  );

  const rows = useMemo<DataSyncRow[]>(() => {
    const tableNames = Array.from(new Set([...tableOptions.map((table) => table.name), ...currentRuns.map((run) => run.tableName)]));
    return tableNames.map((tableName) => {
      const meta = tableMetaByName.get(tableName);
      return {
        tableName,
        sourceExists: meta?.sourceExists ?? true,
        targetExists: meta?.targetExists ?? true,
        run: runByTable.get(tableName),
      };
    });
  }, [currentRuns, runByTable, tableMetaByName, tableOptions]);

  const selectedSql = useMemo(() => {
    const selected = new Set(selectedTables.map(String));
    return dataSqlWithComments(
      currentRuns
        .filter((run) => selected.has(run.tableName))
        .flatMap((run) => {
          const selectedOperations = operationSelection[run.tableName] || emptyOperationSelection;
          return run.diffs.filter((diff) => selectedOperations[diff.diffType] && diff.syncSql);
        }),
    );
  }, [currentRuns, operationSelection, selectedTables]);

  const selectedSqlCount = useMemo(() => {
    const selected = new Set(selectedTables.map(String));
    return currentRuns
      .filter((run) => selected.has(run.tableName))
      .reduce((count, run) => {
        const selectedOperations = operationSelection[run.tableName] || emptyOperationSelection;
        return count + run.diffs.filter((diff) => selectedOperations[diff.diffType] && diff.syncSql).length;
      }, 0);
  }, [currentRuns, operationSelection, selectedTables]);

  const progressPercent = compareProgress.total > 0
    ? Math.round((compareProgress.completed / compareProgress.total) * 100)
    : 0;

  const selectedOperationSummary = useMemo(() => {
    const selected = new Set(selectedTables.map(String));
    return currentRuns.reduce(
      (summary, run) => {
        if (!selected.has(run.tableName)) return summary;
        const selectedOperations = operationSelection[run.tableName] || emptyOperationSelection;
        return {
          insert: summary.insert + (selectedOperations.insert ? run.summary.inserts : 0),
          update: summary.update + (selectedOperations.update ? run.summary.updates : 0),
          delete: summary.delete + (selectedOperations.delete ? run.summary.deletes : 0),
          same: summary.same + run.summary.sameRows,
        };
      },
      { insert: 0, update: 0, delete: 0, same: 0 },
    );
  }, [currentRuns, operationSelection, selectedTables]);

  const toggleOperation = (tableName: string, operation: DataOperation, checked: boolean) => {
    setOperationSelection((current) => ({
      ...current,
      [tableName]: {
        ...(current[tableName] || emptyOperationSelection),
        [operation]: checked,
      },
    }));
  };

  const renderOperation = (row: DataSyncRow, operation: DataOperation, count: number) => (
    <Space size={6} onClick={(event) => event.stopPropagation()}>
      <Checkbox
        checked={Boolean(operationSelection[row.tableName]?.[operation])}
        disabled={!row.run || count === 0}
        onChange={(event) => toggleOperation(row.tableName, operation, event.target.checked)}
      />
      <span className={`operation-count operation-count-${operation} ${count === 0 ? "muted-count" : ""}`}>
        {count}
      </span>
    </Space>
  );

  const renderTableName = (exists: boolean, tableName: string) => (
    <span className={!exists ? "muted-count" : undefined}>{exists ? tableName : "-"}</span>
  );

  const columns: TableColumnsType<DataSyncRow> = [
    {
      title: t("data.sourceTable"),
      dataIndex: "tableName",
      width: 220,
      render: (value, row) => renderTableName(row.sourceExists, value),
    },
    {
      title: t("data.targetTable"),
      dataIndex: "tableName",
      width: 220,
      render: (value, row) => renderTableName(row.targetExists, value),
    },
    {
      title: t("data.insert"),
      width: 130,
      render: (_, row) => renderOperation(row, "insert", row.run?.summary.inserts || 0),
    },
    {
      title: t("data.update"),
      width: 130,
      render: (_, row) => renderOperation(row, "update", row.run?.summary.updates || 0),
    },
    {
      title: t("data.delete"),
      width: 130,
      render: (_, row) => renderOperation(row, "delete", row.run?.summary.deletes || 0),
    },
    {
      title: t("data.same"),
      width: 130,
      render: (_, row) => {
        const count = row.run?.summary.sameRows || 0;
        return <span className={count === 0 ? "muted-count" : undefined}>{count}</span>;
      },
    },
  ];

  return (
    <div className="data-sync-page">
      <section className="page-title compact-page-title">
        <div>
          <Typography.Title level={2}>{t("data.title")}</Typography.Title>
          <Typography.Text type="secondary">{t("data.description")}</Typography.Text>
        </div>
      </section>
      <Card title={t("data.compareConfig")}>
        {connections.length < 2 && (
          <Alert
            className="security-alert"
            type="info"
            showIcon
            message={t("schema.needTwoConnections")}
            description={t("data.needTwoConnectionsDesc")}
          />
        )}
        <Form
          form={form}
          layout="vertical"
          initialValues={blankDataCompare()}
          onFinish={(values) => {
            onRun({
              sourceConnectionId: values.sourceConnectionId,
              targetConnectionId: values.targetConnectionId,
              tableNames: selectedTables.map(String),
            });
          }}
          onValuesChange={(_, values) => {
            if (values.sourceConnectionId && values.targetConnectionId) {
              onConnectionsChanged(values);
            }
          }}
        >
          <Row gutter={12}>
            <Col xs={24} md={12}>
              <Form.Item
                label={t("common.source")}
                name="sourceConnectionId"
                rules={[{ required: true, message: t("schema.selectSource") }]}
              >
                <Select showSearch optionFilterProp="label" options={connectionOptions} />
              </Form.Item>
            </Col>
            <Col xs={24} md={12}>
              <Form.Item
                label={t("common.target")}
                name="targetConnectionId"
                rules={[{ required: true, message: t("schema.selectTarget") }]}
              >
                <Select showSearch optionFilterProp="label" options={connectionOptions} />
              </Form.Item>
            </Col>
          </Row>
          <div className="data-sync-toolbar">
            <span />
            <Space size={8}>
              <Button onClick={onReset} disabled={runningCompare}>
                {t("schema.reset")}
              </Button>
              <Button
                type="primary"
                htmlType="submit"
                icon={<PlayCircleOutlined />}
                loading={runningCompare || loadingTables}
                disabled={connections.length < 2 || selectedTables.length === 0}
              >
                {t("data.run")}
              </Button>
            </Space>
          </div>
        </Form>
        {rows.length > 0 ? (
          <>
            <div className="data-sync-selected-summary">
              <div className="data-sync-selected-stat data-sync-selected-tables-stat">
                <span className="data-sync-selected-stat-title">{t("data.selectedTablesTitle")}</span>
                <span className="data-sync-selected-stat-value">{selectedTables.length}</span>
              </div>
              <div className="data-sync-selected-stat data-sync-selected-diffs-stat">
                <span className="data-sync-selected-stat-title">{t("stats.diffs")}</span>
                <span className="data-sync-selected-stat-value">{selectedSqlCount}</span>
              </div>
              <div className="data-sync-selected-stat data-sync-selected-insert-stat">
                <span className="data-sync-selected-stat-title">{t("data.insert")}</span>
                <span className="data-sync-selected-stat-value operation-count-insert">{selectedOperationSummary.insert}</span>
              </div>
              <div className="data-sync-selected-stat data-sync-selected-update-stat">
                <span className="data-sync-selected-stat-title">{t("data.update")}</span>
                <span className="data-sync-selected-stat-value operation-count-update">{selectedOperationSummary.update}</span>
              </div>
              <div className="data-sync-selected-stat data-sync-selected-delete-stat">
                <span className="data-sync-selected-stat-title">{t("data.delete")}</span>
                <span className="data-sync-selected-stat-value operation-count-delete">{selectedOperationSummary.delete}</span>
              </div>
              <div className="data-sync-selected-stat data-sync-selected-same-stat">
                <span className="data-sync-selected-stat-title">{t("data.same")}</span>
                <span className="data-sync-selected-stat-value">{selectedOperationSummary.same}</span>
              </div>
            </div>
            <div className="data-sync-table-actions">
              <Button size="small" onClick={() => setSelectedTables(selectableTables)} disabled={selectableTables.length === 0}>
                {t("schema.selectAllDiffs")}
              </Button>
              <Button size="small" onClick={() => setSelectedTables([])} disabled={selectedTables.length === 0}>
                {t("schema.clearSelection")}
              </Button>
            </div>
            <Table
              className="data-sync-table"
              rowKey="tableName"
              size="small"
              columns={columns}
              dataSource={rows}
              pagination={false}
              scroll={{ y: "clamp(260px, 34vh, 460px)", x: 970 }}
              rowSelection={{
                selectedRowKeys: selectedTables,
                onChange: setSelectedTables,
                getCheckboxProps: (row) => ({
                  disabled: !row.sourceExists || !row.targetExists,
                }),
              }}
            />
          </>
        ) : (
          <Empty className="compact-empty" description={loadingTables ? t("schema.loadingTables") : t("data.noTables")} />
        )}
      </Card>
      {currentRuns.length > 0 && (
        <Card
          className="compare-result-card"
          title={t("schema.generatedSql")}
          extra={
            <Space size={8}>
              <Button size="small" type="primary" onClick={() => onCopySql(selectedSql)}>
                {t("common.copySql")}
              </Button>
            </Space>
          }
        >
          <section className="sql-preview data-sql-preview">
            <SqlCodePreview sql={selectedSql || t("data.noGeneratedSql")} />
          </section>
        </Card>
      )}
      {runningCompare && compareProgress.total > 0 && (
        <div className="data-sync-progress">
          <div className="data-sync-progress-title">
            <Typography.Text type="secondary">
              {t("data.compareProgress", {
                completed: compareProgress.completed,
                total: compareProgress.total,
              })}
            </Typography.Text>
          </div>
          <Progress percent={progressPercent} size="small" />
        </div>
      )}
    </div>
  );
}

type DataSqlDiff = DataCompareRun["diffs"][number];

function dataSqlWithComments(diffs: DataSqlDiff[]) {
  const tableNames = Array.from(new Set(diffs.map((diff) => diff.tableName))).sort();
  return tableNames
    .map((tableName) => {
      const sections = ([
        ["insert", "Insert"],
        ["update", "Update"],
        ["delete", "Delete"],
      ] as const)
        .map(([diffType, label]) => {
          const statements = diffs
            .filter((diff) => diff.tableName === tableName && diff.diffType === diffType)
            .map((diff) => diff.syncSql)
            .filter(Boolean);
          return statements.length > 0 ? `-- ${label}: ${statements.length}\n${statements.join("\n")}` : "";
        })
        .filter(Boolean);
      return sections.length > 0 ? `-- Table: ${tableName}\n${sections.join("\n")}` : "";
    })
    .filter(Boolean)
    .join("\n\n");
}
