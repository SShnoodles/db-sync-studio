import { Badge, Button, Card, Col, Empty, Form, Input, InputNumber, List, Row, Select, Space, Typography } from "antd";
import { CloudServerOutlined, DatabaseOutlined, DeleteOutlined, PlusOutlined } from "@ant-design/icons";

import { EnvironmentTag } from "../components/EnvironmentTag";
import { useI18n } from "../i18n";
import type { DbConnection } from "../types";
import { blankConnection } from "../utils/defaults";
import { plainTextInputProps } from "../utils/input";

export function ConnectionsPage(props: {
  connections: DbConnection[];
  selectedId?: string;
  selected?: DbConnection;
  form: ReturnType<typeof Form.useForm<DbConnection>>[0];
  saving: boolean;
  testing: boolean;
  onSelect: (item: DbConnection) => void;
  onCreate: () => void;
  onSave: (values: DbConnection) => void;
  onTest: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();

  return (
    <>
      <section className="page-title">
        <div>
          <Typography.Text type="secondary">{t("connections.kicker")}</Typography.Text>
          <Typography.Title level={2}>{t("connections.title")}</Typography.Title>
          <Typography.Paragraph>
            {t("connections.description")}
          </Typography.Paragraph>
        </div>
        <Button type="primary" icon={<PlusOutlined />} onClick={props.onCreate}>
          {t("connections.new")}
        </Button>
      </section>
      <Row gutter={12} align="top">
        <Col xs={24} lg={8}>
          <Card
            className="connection-list-card"
            title={t("connections.saved")}
            extra={<Badge count={props.connections.length} showZero />}
          >
            <List
              locale={{
                emptyText: (
                  <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={t("connections.empty")} />
                ),
              }}
              dataSource={props.connections}
              renderItem={(item) => (
                <List.Item
                  className={item.id === props.selectedId ? "selected-connection" : ""}
                  onClick={() => props.onSelect(item)}
                >
                  <List.Item.Meta
                    avatar={<DatabaseOutlined className="database-avatar" />}
                    title={
                      <Space>
                        <Typography.Text strong>{item.name}</Typography.Text>
                        <EnvironmentTag value={item.environment} />
                      </Space>
                    }
                    description={`${item.username || "root"}@${item.host}:${item.port} · ${item.database}`}
                  />
                </List.Item>
              )}
            />
          </Card>
        </Col>
        <Col xs={24} lg={16}>
          <Card
            title={props.selected ? t("connections.edit") : t("connections.new")}
            extra={
              props.selected && (
                <Button danger type="text" icon={<DeleteOutlined />} onClick={props.onDelete}>
                  {t("common.delete")}
                </Button>
              )
            }
          >
            <Form
              form={props.form}
              layout="vertical"
              onFinish={props.onSave}
              initialValues={blankConnection()}
            >
              <Form.Item name="id" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
              <Form.Item name="createdAt" hidden>
                <Input {...plainTextInputProps} />
              </Form.Item>
              <Row gutter={12}>
                <Col span={24}>
                  <Form.Item
                    label={t("connections.name")}
                    name="name"
                    rules={[{ required: true, message: t("connections.nameRequired") }]}
                  >
                    <Input {...plainTextInputProps} placeholder={t("connections.namePlaceholder")} />
                  </Form.Item>
                </Col>
                <Col xs={24} md={12}>
                  <Form.Item label={t("connections.dbType")} name="dbType">
                    <Select disabled options={[{ value: "mysql", label: t("common.mysql") }]} />
                  </Form.Item>
                </Col>
                <Col xs={24} md={12}>
                  <Form.Item label={t("connections.environment")} name="environment">
                    <Select
                      options={["Development", "Testing", "Staging", "Production"].map((value) => ({
                        value,
                        label: value,
                      }))}
                    />
                  </Form.Item>
                </Col>
                <Col xs={24} md={16}>
                  <Form.Item label={t("connections.host")} name="host" rules={[{ required: true, message: t("connections.hostRequired") }]}>
                    <Input {...plainTextInputProps} prefix={<CloudServerOutlined />} />
                  </Form.Item>
                </Col>
                <Col xs={24} md={8}>
                  <Form.Item label={t("connections.port")} name="port">
                    <InputNumber min={1} max={65535} className="full-width" />
                  </Form.Item>
                </Col>
                <Col span={24}>
                  <Form.Item
                    label={t("connections.database")}
                    name="database"
                    rules={[{ required: true, message: t("connections.databaseRequired") }]}
                  >
                    <Input {...plainTextInputProps} />
                  </Form.Item>
                </Col>
                <Col xs={24} md={12}>
                  <Form.Item label={t("connections.username")} name="username">
                    <Input {...plainTextInputProps} />
                  </Form.Item>
                </Col>
                <Col xs={24} md={12}>
                  <Form.Item label={t("connections.password")} name="password">
                    <Input.Password {...plainTextInputProps} placeholder={t("connections.passwordPlaceholder")} />
                  </Form.Item>
                </Col>
                <Col span={24}>
                  <Form.Item label={t("connections.sslMode")} name="sslMode">
                    <Select
                      options={[
                        { value: "prefer", label: "Prefer" },
                        { value: "disable", label: "Disable" },
                        { value: "require", label: "Require" },
                      ]}
                    />
                  </Form.Item>
                </Col>
              </Row>
              <div className="form-actions">
                <Button onClick={props.onTest} loading={props.testing}>
                  {t("connections.test")}
                </Button>
                <Button type="primary" htmlType="submit" loading={props.saving}>
                  {t("connections.save")}
                </Button>
              </div>
            </Form>
          </Card>
        </Col>
      </Row>
    </>
  );
}
