import { useEffect, useState } from "react";
import { Alert, Button, Card, Col, Form, Input, Progress, Row, Select, Space, Table, Typography } from "antd";
import type { TableColumnsType } from "antd";
import { PlayCircleOutlined } from "@ant-design/icons";

import { CompareResultPanel } from "../components/schemaCompare";
import { useI18n } from "../i18n";
import type { CompareRun, CompareTask, DbConnection } from "../types";
import { connectionOptionLabel } from "../utils/connection";
import { blankTask } from "../utils/defaults";
import { formatDuration } from "../utils/format";
import { plainTextInputProps } from "../utils/input";

export function TasksPage(props: {
  connections: DbConnection[];
  form: ReturnType<typeof Form.useForm<CompareTask>>[0];
  tableOptions: string[];
  currentRun?: CompareRun;
  loadingTables: boolean;
  runningCompare: boolean;
  syncingSchema: boolean;
  schemaProgress?: {
    kind: "compare" | "sync";
    completed: number;
    total: number;
    startedAt: number;
    finishedAt?: number;
    status?: "normal" | "active" | "success" | "exception";
  };
  onRun: (values: CompareTask) => void;
  onCopySql: (sql: string) => void;
  onSyncSql: (sql: string) => void;
  onConnectionsChanged: (task: Partial<CompareTask>) => void;
}) {
  const { t } = useI18n();
  const [nowMs, setNowMs] = useState(Date.now());
  const schemaProgressPercent = props.schemaProgress
    ? Math.round((props.schemaProgress.completed / Math.max(props.schemaProgress.total, 1)) * 100)
    : 0;
  const schemaElapsedMs = props.schemaProgress
    ? (props.schemaProgress.finishedAt ?? nowMs) - props.schemaProgress.startedAt
    : 0;

  useEffect(() => {
    if (!props.schemaProgress || props.schemaProgress.finishedAt) return;
    const timer = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [props.schemaProgress]);
  useEffect(() => {
    props.form.setFieldValue("selectedTables", []);
  }, [props.form, props.tableOptions]);
  const connectionOptions = props.connections.map((connection) => ({
    value: connection.id,
    label: connectionOptionLabel(connection),
  }));
  const selectedTables = Form.useWatch("selectedTables", props.form) || [];
  const tableColumns: TableColumnsType<{ name: string }> = [
    {
      title: t("schema.tables"),
      dataIndex: "name",
    },
  ];
  const updateSelectedTables = (nextTables: string[]) => {
    props.form.setFieldValue("selectedTables", nextTables);
  };
  const toggleTable = (tableName: string) => {
    const selected = new Set(selectedTables);
    if (selected.has(tableName)) {
      selected.delete(tableName);
    } else {
      selected.add(tableName);
    }
    updateSelectedTables(Array.from(selected));
  };

  return (
    <>
      <section className="page-title compact-page-title">
        <div>
          <Typography.Title level={2}>{t("schema.title")}</Typography.Title>
          <Typography.Text type="secondary">{t("schema.description")}</Typography.Text>
        </div>
      </section>
      <Row gutter={12} align="top">
        <Col span={24}>
          <Card title={t("schema.compareConfig")}>
            {props.connections.length < 2 && (
              <Alert
                className="security-alert"
                type="info"
                showIcon
                message={t("schema.needTwoConnections")}
                description={t("schema.needTwoConnectionsDesc")}
              />
            )}
            <Form
              form={props.form}
              layout="vertical"
              onFinish={props.onRun}
              initialValues={blankTask()}
              onValuesChange={(_, values) => {
                if (values.sourceConnectionId && values.targetConnectionId) {
                  props.onConnectionsChanged(values);
                }
              }}
            >
              <Form.Item name="id" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
              <Form.Item name="createdAt" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
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
              <Form.Item name="name" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
              <Form.Item name="compareType" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
              <Form.Item name="selectedTables" hidden>
                <span />
              </Form.Item>
              <Form.Item
                label={t("schema.tables")}
                tooltip={t("schema.tablesTooltip")}
              >
                {props.tableOptions.length > 0 ? (
                  <div>
                    <div className="schema-table-toolbar">
                      <Typography.Text type="secondary">
                        {t("data.selectedTables", { count: selectedTables.length })}
                      </Typography.Text>
                      <Space size={8}>
                        <Button size="small" onClick={() => updateSelectedTables(props.tableOptions)}>
                          {t("schema.selectAllDiffs")}
                        </Button>
                        <Button size="small" onClick={() => updateSelectedTables([])}>
                          {t("schema.clearSelection")}
                        </Button>
                        <Button
                          size="small"
                          type="primary"
                          htmlType="submit"
                          icon={<PlayCircleOutlined />}
                          loading={props.runningCompare}
                          disabled={props.connections.length < 2 || props.syncingSchema}
                        >
                          {t("schema.run")}
                        </Button>
                      </Space>
                    </div>
                    <Table
                      className="schema-table-list"
                      rowKey="name"
                      size="small"
                      columns={tableColumns}
                      dataSource={props.tableOptions.map((name) => ({ name }))}
                      pagination={false}
                      loading={props.loadingTables}
                      scroll={{ y: 304 }}
                      rowSelection={{
                        selectedRowKeys: selectedTables,
                        onChange: (keys) => updateSelectedTables(keys.map(String)),
                      }}
                      onRow={(record) => ({
                        onClick: () => toggleTable(record.name),
                      })}
                    />
                  </div>
                ) : (
                  <div className="schema-table-toolbar">
                    <Typography.Text type="secondary">
                      {props.loadingTables ? t("schema.loadingTables") : t("schema.allTablesHint")}
                    </Typography.Text>
                    <Button
                      size="small"
                      type="primary"
                      htmlType="submit"
                      icon={<PlayCircleOutlined />}
                      loading={props.runningCompare}
                      disabled={props.connections.length < 2 || props.syncingSchema}
                    >
                      {t("schema.run")}
                    </Button>
                  </div>
                )}
              </Form.Item>
            </Form>
          </Card>
          {props.schemaProgress && (
            <div className="schema-progress">
              <div className="schema-progress-title">
                <Typography.Text type="secondary">
                  {props.schemaProgress.kind === "compare" ? t("schema.compareProgress") : t("schema.syncProgress")}
                </Typography.Text>
                <Typography.Text type="secondary">
                  {t("schema.progressCount", {
                    completed: props.schemaProgress.completed,
                    total: props.schemaProgress.total,
                    percent: schemaProgressPercent,
                    elapsed: formatDuration(schemaElapsedMs),
                  })}
                </Typography.Text>
              </div>
              <Progress
                percent={schemaProgressPercent}
                size="small"
                status={props.schemaProgress.status}
              />
            </div>
          )}
          {props.currentRun && (
            <CompareResultPanel
              run={props.currentRun}
              onCopySql={props.onCopySql}
              onSyncSql={props.onSyncSql}
              syncing={props.syncingSchema}
              syncDisabled={props.runningCompare}
            />
          )}
        </Col>
      </Row>
    </>
  );
}
