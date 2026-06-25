import { Card, Col, Form, Row, Select, Typography } from "antd";

import type { Language } from "../i18n";
import { useI18n } from "../i18n";

type ThemeMode = "light" | "dark";

export function SettingsPage({
  language,
  themeMode,
  onLanguageChange,
  onThemeModeChange,
}: {
  language: Language;
  themeMode: ThemeMode;
  onLanguageChange: (language: Language) => void;
  onThemeModeChange: (themeMode: ThemeMode) => void;
}) {
  const { t } = useI18n();

  return (
    <>
      <section className="page-title compact-page-title">
        <div>
          <Typography.Title level={2}>{t("settings.title")}</Typography.Title>
          <Typography.Text type="secondary">{t("settings.description")}</Typography.Text>
        </div>
      </section>
      <Card title={t("settings.appearance")}>
        <Form layout="vertical" className="settings-form">
          <Row gutter={12}>
            <Col xs={24} md={8}>
              <Form.Item label={t("common.language")}>
                <Select
                  value={language}
                  aria-label={t("common.language")}
                  onChange={onLanguageChange}
                  options={[
                    { value: "en", label: t("common.english") },
                    { value: "zh-CN", label: t("common.chinese") },
                  ]}
                />
              </Form.Item>
            </Col>
            <Col xs={24} md={8}>
              <Form.Item label={t("common.theme")}>
                <Select
                  value={themeMode}
                  aria-label={t("common.theme")}
                  onChange={onThemeModeChange}
                  options={[
                    { value: "light", label: t("common.light") },
                    { value: "dark", label: t("common.dark") },
                  ]}
                />
              </Form.Item>
            </Col>
          </Row>
        </Form>
      </Card>
    </>
  );
}
