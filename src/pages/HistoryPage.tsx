import { useEffect, useState } from "react";
import { Button, Card, Checkbox, Col, DatePicker, Descriptions, Empty, Input, Pagination, Row, Select, Space, Statistic, Table, Tag, Typography } from "antd";
import type { TableColumnsType } from "antd";

import { DiffTable, SummaryStats } from "../components/schemaCompare";
import { useI18n } from "../i18n";
import type { CompareRun, DataCompareRun, HistoryFilter, HistoryRun } from "../types";
import { formatDate } from "../utils/format";

type HistoryTypeFilter = "all" | "schema" | "data";
type DatabaseTypeFilter = "all" | "mysql" | "postgresql" | "sqlite";
type TimeRange = Parameters<NonNullable<React.ComponentProps<typeof DatePicker.RangePicker>["onChange"]>>[0];

export function HistoryPage({
  history,
  total,
  page,
  pageSize,
  onCopySql,
  onLoadDetail,
  onLoadSql,
  onDelete,
  onClear,
  onSearch,
  onPageChange,
}: {
  history: HistoryRun[];
  total: number;
  page: number;
  pageSize: number;
  onCopySql: (sql: string) => void;
  onLoadDetail: (id: string) => Promise<HistoryRun>;
  onLoadSql: (id: string) => Promise<string>;
  onDelete: (ids: string[]) => void;
  onClear: () => void;
  onSearch: (filter: HistoryFilter) => void;
  onPageChange: (page: number) => void;
}) {
  const { t } = useI18n();
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [typeFilter, setTypeFilter] = useState<HistoryTypeFilter>("all");
  const [databaseTypeFilter, setDatabaseTypeFilter] = useState<DatabaseTypeFilter>("all");
  const [timeRange, setTimeRange] = useState<TimeRange>(null);
  const [searchContent, setSearchContent] = useState("");
  const [expandedIds, setExpandedIds] = useState<string[]>([]);
  const [details, setDetails] = useState<Record<string, HistoryRun>>({});
  const [loadingIds, setLoadingIds] = useState<string[]>([]);
  const pageItems = history;
  const pageIds = pageItems.map((item) => item.id);
  const allPageSelected =
    pageIds.length > 0 && pageIds.every((id) => selectedIds.includes(id));
  const pagePartiallySelected =
    pageIds.some((id) => selectedIds.includes(id)) && !allPageSelected;

  const toggleOne = (id: string, checked: boolean) => {
    setSelectedIds((current) =>
      checked ? Array.from(new Set([...current, id])) : current.filter((item) => item !== id),
    );
  };

  const togglePage = (checked: boolean) => {
    setSelectedIds((current) =>
      checked
        ? Array.from(new Set([...current, ...pageIds]))
        : current.filter((id) => !pageIds.includes(id)),
    );
  };

  useEffect(() => {
    setSelectedIds((current) =>
      current.filter((id) => history.some((item) => item.id === id)),
    );
  }, [history]);

  const submitSearch = () => {
    setSelectedIds([]);
    setExpandedIds([]);
    onSearch({
      syncType: typeFilter,
      databaseType: databaseTypeFilter,
      startTime: timeRange?.[0]?.startOf("day").toISOString(),
      endTime: timeRange?.[1]?.endOf("day").toISOString(),
      searchContent: searchContent.trim() || undefined,
    });
  };

  const loadDetail = async (id: string) => {
    if (details[id]) return details[id];
    setLoadingIds((current) => Array.from(new Set([...current, id])));
    try {
      const detail = await onLoadDetail(id);
      setDetails((current) => ({ ...current, [id]: detail }));
      return detail;
    } finally {
      setLoadingIds((current) => current.filter((item) => item !== id));
    }
  };

  const toggleDetail = async (id: string) => {
    if (expandedIds.includes(id)) {
      setExpandedIds((current) => current.filter((item) => item !== id));
      return;
    }
    await loadDetail(id);
    setExpandedIds((current) => Array.from(new Set([...current, id])));
  };

  const copyHistorySql = async (run: HistoryRun) => {
    setLoadingIds((current) => Array.from(new Set([...current, run.id])));
    try {
      onCopySql(await onLoadSql(run.id));
    } finally {
      setLoadingIds((current) => current.filter((item) => item !== run.id));
    }
  };

  const changePage = (nextPage: number) => {
    setSelectedIds([]);
    setExpandedIds([]);
    onPageChange(nextPage);
  };

  return (
    <div className="history-page">
      <section className="page-title">
        <div>
          <Typography.Title level={2}>{t("history.title")}</Typography.Title>
          <Typography.Paragraph>
            {t("history.description")}
          </Typography.Paragraph>
        </div>
      </section>
      <Card className="history-filter-card">
        <Space size={12} wrap>
          <Space size={6}>
            <Typography.Text type="secondary">{t("history.syncType")}</Typography.Text>
            <Select
              value={typeFilter}
              className="history-filter-select"
              onChange={setTypeFilter}
              options={[
                { value: "all", label: t("history.allTypes") },
                { value: "schema", label: t("menu.schemaSync") },
                { value: "data", label: t("menu.dataSync") },
              ]}
            />
          </Space>
          <Space size={6}>
            <Typography.Text type="secondary">{t("history.databaseType")}</Typography.Text>
            <Select
              value={databaseTypeFilter}
              className="history-filter-select"
              onChange={setDatabaseTypeFilter}
              options={[
                { value: "all", label: t("history.allDatabaseTypes") },
                { value: "mysql", label: "MySQL" },
                { value: "postgresql", label: "PostgreSQL" },
                { value: "sqlite", label: "SQLite" },
              ]}
            />
          </Space>
          <Space size={6}>
            <Typography.Text type="secondary">{t("history.timeRange")}</Typography.Text>
            <DatePicker.RangePicker
              value={timeRange}
              onChange={setTimeRange}
              allowClear
            />
          </Space>
          <Space size={6}>
            <Typography.Text type="secondary">{t("history.searchContent")}</Typography.Text>
            <Input
              allowClear
              className="history-search-input"
              value={searchContent}
              placeholder={t("history.searchPlaceholder")}
              onChange={(event) => setSearchContent(event.target.value)}
              onPressEnter={submitSearch}
            />
          </Space>
          <Button type="primary" onClick={submitSearch}>
            {t("common.search")}
          </Button>
          <Button
            danger
            disabled={selectedIds.length === 0}
            onClick={() => onDelete(selectedIds)}
          >
            {t("history.deleteSelected")}
          </Button>
          <Button danger disabled={total === 0} onClick={onClear}>
            {t("history.clearAll")}
          </Button>
        </Space>
      </Card>
      {history.length === 0 ? (
        <Card>
          <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t("history.emptyFiltered")} />
        </Card>
      ) : (
        <>
          <div className="history-selection-bar">
            <Checkbox
              checked={allPageSelected}
              indeterminate={pagePartiallySelected}
              onChange={(event) => togglePage(event.target.checked)}
            >
              {t("history.selectCurrentPage")}
            </Checkbox>
            <Typography.Text type="secondary">
              {t("history.selectedCount", { count: selectedIds.length })}
            </Typography.Text>
          </div>
          <Space direction="vertical" size={12} className="full-width">
            {pageItems.length > 0 ? pageItems.map((run) => (
              <Card
                key={run.id}
                title={
                  <Space>
                    <Checkbox
                      checked={selectedIds.includes(run.id)}
                      onChange={(event) => toggleOne(run.id, event.target.checked)}
                    />
                    <span>{historyTitle(run)}</span>
                    <Tag color={isExecutionHistory(run) ? (run.status === "success" ? "green" : "red") : isDataHistory(run) ? "purple" : "blue"}>
                      {isExecutionHistory(run) ? t("history.execution") : t("history.comparison")}
                    </Tag>
                    <Tag color={historySyncType(run) === "data" ? "purple" : "blue"}>
                      {historySyncType(run) === "data" ? t("menu.dataSync") : t("menu.schemaSync")}
                    </Tag>
                    {run.dbType && <Tag color="geekblue">{dbTypeLabel(run.dbType)}</Tag>}
                  </Space>
                }
                extra={
                  <Space size={8}>
                    <Button
                      size="small"
                      loading={loadingIds.includes(run.id)}
                      onClick={() => toggleDetail(run.id)}
                    >
                      {expandedIds.includes(run.id) ? t("history.hideDetails") : t("history.viewDetails")}
                    </Button>
                    <Button
                      size="small"
                      onClick={() => void copyHistorySql(run)}
                      loading={loadingIds.includes(run.id)}
                    >
                      {t("common.copySql")}
                    </Button>
                  </Space>
                }
              >
                <Descriptions
                  size="small"
                  column={3}
                  items={[
                    { key: "time", label: t("common.time"), children: formatDate(run.createdAt) },
                    { key: "dbType", label: t("history.databaseType"), children: run.dbType ? dbTypeLabel(run.dbType) : "-" },
                    { key: "source", label: t("common.source"), children: run.sourceName || "-" },
                    { key: "target", label: t("common.target"), children: run.targetName },
                  ]}
                />
                {isExecutionHistory(run) ? (
                  <ExecutionHistoryStats run={run} />
                ) : isDataHistory(run) ? (
                  <DataHistoryStats run={run} />
                ) : (
                  <SummaryStats summary={run.summary} />
                )}
                {expandedIds.includes(run.id) && details[run.id] && (
                  isExecutionHistory(details[run.id]) ? (
                    <ExecutionHistoryDetail run={details[run.id] as Extract<HistoryRun, { runType: "execution" }>} />
                  ) : isDataHistory(details[run.id]) ? (
                    <DataHistoryDetail run={details[run.id] as Extract<HistoryRun, { runType: "data" }>} />
                  ) : (
                    <DiffTable diffs={(details[run.id] as CompareRun).diffs} compact />
                  )
                )}
              </Card>
            )) : (
              <Card>
                <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t("history.emptyFiltered")} />
              </Card>
            )}
          </Space>
          <div className="history-pagination">
            <Pagination
              current={page}
              pageSize={pageSize}
              total={total}
              size="small"
              showSizeChanger={false}
              showTotal={(total) => t("history.total", { count: total })}
              onChange={changePage}
            />
          </div>
        </>
      )}
    </div>
  );
}

function dbTypeLabel(dbType: string) {
  if (dbType === "sqlite") return "SQLite";
  return dbType === "postgresql" ? "PostgreSQL" : "MySQL";
}

function isDataHistory(run: HistoryRun): run is Extract<HistoryRun, { runType: "data" }> {
  return "runType" in run && run.runType === "data";
}

function isExecutionHistory(run: HistoryRun): run is Extract<HistoryRun, { runType: "execution" }> {
  return "runType" in run && run.runType === "execution";
}

function historyTitle(run: HistoryRun) {
  if (isExecutionHistory(run) || isDataHistory(run)) return run.title;
  return run.taskName;
}

function historySyncType(run: HistoryRun) {
  if (isExecutionHistory(run)) return run.syncType;
  return isDataHistory(run) ? "data" : "schema";
}

function ExecutionHistoryStats({ run }: { run: Extract<HistoryRun, { runType: "execution" }> }) {
  const { t } = useI18n();
  return (
    <Row gutter={[8, 8]} className="summary-stats">
      <Col xs={12} md={6}>
        <Statistic title={t("history.status")} value={run.status === "success" ? t("history.success") : t("history.failed")} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("history.statements")} value={run.summary.statements} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("history.executed")} value={run.summary.executed} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("history.skipped")} value={run.summary.skipped} />
      </Col>
    </Row>
  );
}

function ExecutionHistoryDetail({ run }: { run: Extract<HistoryRun, { runType: "execution" }> }) {
  const { t } = useI18n();
  return (
    <Descriptions
      size="small"
      column={1}
      items={[
        { key: "status", label: t("history.status"), children: run.status === "success" ? t("history.success") : t("history.failed") },
        { key: "error", label: t("history.error"), children: run.error ? <Typography.Text type="danger">{run.error}</Typography.Text> : "-" },
      ]}
    />
  );
}

function DataHistoryStats({ run }: { run: Extract<HistoryRun, { runType: "data" }> }) {
  const { t } = useI18n();

  return (
    <Row gutter={[8, 8]} className="summary-stats">
      <Col xs={12} md={4}>
        <Statistic title={t("stats.tables")} value={run.summary.tables} />
      </Col>
      <Col xs={12} md={4}>
        <Statistic title={t("stats.diffs")} value={run.summary.totalDiffs} />
      </Col>
      <Col xs={12} md={4}>
        <Statistic title={t("data.insert")} value={run.summary.inserts} />
      </Col>
      <Col xs={12} md={4}>
        <Statistic title={t("data.update")} value={run.summary.updates} />
      </Col>
      <Col xs={12} md={4}>
        <Statistic title={t("data.delete")} value={run.summary.deletes} />
      </Col>
      <Col xs={12} md={4}>
        <Statistic title={t("data.same")} value={run.summary.sameRows} />
      </Col>
    </Row>
  );
}

function DataHistoryDetail({ run }: { run: Extract<HistoryRun, { runType: "data" }> }) {
  const { t } = useI18n();
  const columns: TableColumnsType<DataCompareRun> = [
    { title: t("data.sourceTable"), dataIndex: "tableName", width: 220 },
    { title: t("data.targetTable"), dataIndex: "tableName", width: 220 },
    { title: t("data.insert"), render: (_, item) => item.summary.inserts, width: 100 },
    { title: t("data.update"), render: (_, item) => item.summary.updates, width: 100 },
    { title: t("data.delete"), render: (_, item) => item.summary.deletes, width: 100 },
    { title: t("data.same"), render: (_, item) => item.summary.sameRows, width: 100 },
  ];

  return (
    <Table
      className="data-sync-table"
      rowKey="id"
      size="small"
      columns={columns}
      dataSource={run.runs}
      pagination={false}
      scroll={{ x: 840, y: 260 }}
    />
  );
}
