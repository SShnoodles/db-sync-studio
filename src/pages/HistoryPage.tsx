import { useEffect, useMemo, useState } from "react";
import { Button, Card, Checkbox, Col, DatePicker, Descriptions, Empty, Pagination, Row, Select, Space, Statistic, Table, Tag, Typography } from "antd";
import type { TableColumnsType } from "antd";

import { DiffTable, SqlPreview, SummaryStats } from "../components/schemaCompare";
import { useI18n } from "../i18n";
import type { DataCompareRun, HistoryFilter, HistoryRun } from "../types";
import { formatDate } from "../utils/format";

const pageSize = 3;
type HistoryTypeFilter = "all" | "schema" | "data";
type TimeRange = Parameters<NonNullable<React.ComponentProps<typeof DatePicker.RangePicker>["onChange"]>>[0];

export function HistoryPage({
  history,
  onCopySql,
  onDelete,
  onClear,
  onSearch,
}: {
  history: HistoryRun[];
  onCopySql: (sql: string) => void;
  onDelete: (ids: string[]) => void;
  onClear: () => void;
  onSearch: (filter: HistoryFilter) => void;
}) {
  const { t } = useI18n();
  const [page, setPage] = useState(1);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [typeFilter, setTypeFilter] = useState<HistoryTypeFilter>("all");
  const [timeRange, setTimeRange] = useState<TimeRange>(null);
  const pageItems = useMemo(
    () => history.slice((page - 1) * pageSize, page * pageSize),
    [history, page],
  );
  const maxPage = Math.max(1, Math.ceil(history.length / pageSize));
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
    if (page > maxPage) {
      setPage(maxPage);
    }
    setSelectedIds((current) =>
      current.filter((id) => history.some((item) => item.id === id)),
    );
  }, [history, maxPage, page]);

  const submitSearch = () => {
    setPage(1);
    setSelectedIds([]);
    onSearch({
      syncType: typeFilter,
      startTime: timeRange?.[0]?.startOf("day").toISOString(),
      endTime: timeRange?.[1]?.endOf("day").toISOString(),
    });
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
            <Typography.Text type="secondary">{t("history.timeRange")}</Typography.Text>
            <DatePicker.RangePicker
              value={timeRange}
              onChange={setTimeRange}
              allowClear
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
          <Button danger disabled={history.length === 0} onClick={onClear}>
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
                    <span>{isDataHistory(run) ? run.title : run.taskName}</span>
                    <Tag color={isDataHistory(run) ? "purple" : "blue"}>
                      {isDataHistory(run) ? t("menu.dataSync") : t("menu.schemaSync")}
                    </Tag>
                  </Space>
                }
                extra={
                  <Button size="small" onClick={() => onCopySql(run.syncSql)}>
                    {t("common.copySql")}
                  </Button>
                }
              >
                <Descriptions
                  size="small"
                  column={3}
                  items={[
                    { key: "time", label: t("common.time"), children: formatDate(run.createdAt) },
                    { key: "source", label: t("common.source"), children: run.sourceName },
                    { key: "target", label: t("common.target"), children: run.targetName },
                  ]}
                />
                {isDataHistory(run) ? (
                  <DataHistorySummary run={run} />
                ) : (
                  <>
                    <SummaryStats summary={run.summary} diffs={run.diffs} />
                    <DiffTable diffs={run.diffs} compact />
                  </>
                )}
                <SqlPreview sql={run.syncSql} />
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
              total={history.length}
              size="small"
              showSizeChanger={false}
              showTotal={(total) => t("history.total", { count: total })}
              onChange={setPage}
            />
          </div>
        </>
      )}
    </div>
  );
}

function isDataHistory(run: HistoryRun): run is Extract<HistoryRun, { runType: "data" }> {
  return "runType" in run && run.runType === "data";
}

function DataHistorySummary({ run }: { run: Extract<HistoryRun, { runType: "data" }> }) {
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
    <>
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
      <Table
        className="data-sync-table"
        rowKey="id"
        size="small"
        columns={columns}
        dataSource={run.runs}
        pagination={false}
        scroll={{ x: 840 }}
      />
    </>
  );
}
