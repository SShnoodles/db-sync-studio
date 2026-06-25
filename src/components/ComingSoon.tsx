import { Card, Empty, Typography } from "antd";

import { useI18n } from "../i18n";
import type { Page } from "../types";

export function ComingSoon({ page }: { page: Page }) {
  const { t } = useI18n();
  const names: Record<string, string> = {
    schemaSync: t("menu.schemaSync"),
    dataSync: t("menu.dataSync"),
    history: t("menu.history"),
    settings: t("menu.settings"),
    overview: t("menu.overview"),
    connections: t("menu.connections"),
  };
  const description =
    page === "dataSync"
      ? t("comingSoon.dataSyncDescription")
      : t("comingSoon.defaultDescription");

  return (
    <Card className="coming-soon">
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={
          <>
            <Typography.Title level={3}>{names[page]}</Typography.Title>
            <Typography.Paragraph type="secondary">
              {description}
            </Typography.Paragraph>
          </>
        }
      />
    </Card>
  );
}
