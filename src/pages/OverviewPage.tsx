import { Button, Card, Col, Row, Space, Statistic, Tag, Typography } from "antd";
import { DatabaseOutlined, DiffOutlined, HistoryOutlined, PlusOutlined, TableOutlined } from "@ant-design/icons";

import { useI18n } from "../i18n";
import type { DbConnection, HistoryCounts, HistoryRun } from "../types";

export function OverviewPage({
  connections,
  history,
  historyCounts,
  onCreate,
  onConnections,
}: {
  connections: DbConnection[];
  history: HistoryRun[];
  historyCounts: HistoryCounts;
  onCreate: () => void;
  onConnections: () => void;
}) {
  const { t } = useI18n();
  const schemaHistoryCount = historyCounts.schema;
  const dataHistoryCount = historyCounts.data;
  const historyCount = historyCounts.total || history.length;

  return (
    <>
      <section className="page-title">
        <div>
          <Typography.Text type="secondary">{t("overview.kicker")}</Typography.Text>
          <Typography.Title level={2}>{t("overview.title")}</Typography.Title>
          <Typography.Paragraph>
            {t("overview.description")}
          </Typography.Paragraph>
        </div>
        <Button type="primary" icon={<PlusOutlined />} onClick={onCreate}>
          {t("overview.addConnection")}
        </Button>
      </section>
      <Row gutter={[12, 12]}>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title={t("overview.connections")} value={connections.length} prefix={<DatabaseOutlined />} />
            <Typography.Text type="secondary">{t("overview.savedMysqlEndpoints")}</Typography.Text>
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title={t("overview.schemaSyncRecords")} value={schemaHistoryCount} prefix={<DiffOutlined />} />
            <Typography.Text type="secondary">{t("overview.schemaSyncRecords")}</Typography.Text>
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title={t("overview.dataSyncRecords")} value={dataHistoryCount} prefix={<TableOutlined />} />
            <Typography.Text type="secondary">{t("overview.dataSyncRecords")}</Typography.Text>
          </Card>
        </Col>
        <Col xs={24} md={6}>
          <Card>
            <Statistic title={t("overview.historyRecords")} value={historyCount} prefix={<HistoryOutlined />} />
            <Typography.Text type="secondary">{t("overview.history")}</Typography.Text>
          </Card>
        </Col>
      </Row>
      <Card
        className="get-started"
        title={t("overview.getStarted")}
        extra={
          <Button type="link" onClick={onConnections}>
            {t("overview.manageConnections")}
          </Button>
        }
      >
        <Row align="middle" gutter={18}>
          <Col flex="auto">
            <Typography.Title level={4}>{t("overview.workflowTitle")}</Typography.Title>
            <Typography.Paragraph type="secondary">
              {t("overview.workflowDescription")}
            </Typography.Paragraph>
          </Col>
          <Col>
            <Space direction="vertical">
              <Typography.Text>
                <Tag color="blue">1</Tag> {t("overview.stepAddSource")}
              </Typography.Text>
              <Typography.Text>
                <Tag color="blue">2</Tag> {t("overview.stepAddTarget")}
              </Typography.Text>
              <Typography.Text>
                <Tag>3</Tag> {t("overview.stepCreateTask")}
              </Typography.Text>
              <Typography.Text>
                <Tag>4</Tag> {t("overview.stepReview")}
              </Typography.Text>
            </Space>
          </Col>
        </Row>
      </Card>
    </>
  );
}
