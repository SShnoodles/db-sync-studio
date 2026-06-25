import { useMemo, useState } from "react";
import type { Key } from "react";
import { Button, Card, Col, Empty, Row, Space, Statistic, Tag, Tree, Typography } from "antd";
import type { TreeDataNode } from "antd";

import { SqlCodePreview } from "./SqlCodePreview";
import { useI18n } from "../i18n";
import type { CompareRun, CompareSummary, SchemaDiff } from "../types";
import { diffColor } from "../utils/format";

type DiffEntry = SchemaDiff & { key: string };

export function CompareResultPanel({
  run,
  onCopySql,
}: {
  run: CompareRun;
  onCopySql: (sql: string) => void;
}) {
  const { t } = useI18n();
  const entries = useMemo(() => buildEntries(run.diffs), [run.diffs]);
  const leafKeys = useMemo(() => entries.map((entry) => entry.key), [entries]);
  const [checkedKeys, setCheckedKeys] = useState<string[]>([]);
  const selectedSql = useMemo(
    () =>
      sqlWithTableComments(
        entries
        .filter((entry) => checkedKeys.includes(entry.key))
        .filter((entry) => entry.syncSql),
      ),
    [checkedKeys, entries],
  );

  return (
    <Card
      className="compare-result-card"
      title={t("schema.syncCompare")}
      extra={
        <Space>
          <Button size="small" onClick={() => setCheckedKeys(leafKeys)}>
            {t("schema.selectAllDiffs")}
          </Button>
          <Button size="small" onClick={() => setCheckedKeys([])}>
            {t("schema.clearSelection")}
          </Button>
          <Button size="small" type="primary" onClick={() => onCopySql(selectedSql)}>
            {t("common.copySql")}
          </Button>
        </Space>
      }
    >
      <SummaryStats summary={run.summary} diffs={run.diffs} />
      <SchemaDiffTrees
        diffs={entries}
        checkedKeys={checkedKeys}
        onCheckedKeysChange={setCheckedKeys}
      />
      <SqlPreview sql={selectedSql} />
    </Card>
  );
}

export function SummaryStats({ summary, diffs = [] }: { summary: CompareSummary; diffs?: SchemaDiff[] }) {
  const { t } = useI18n();
  const normalized = {
    added: summary.added ?? diffs.filter((diff) => diff.diffType === "added").length,
    modified: summary.modified ?? diffs.filter((diff) => diff.diffType === "modified").length,
    removed: summary.removed ?? diffs.filter((diff) => diff.diffType === "removed").length,
    same: summary.same ?? 0,
  };

  return (
    <Row gutter={[6, 6]} className="summary-stats compact-summary-stats">
      <Col xs={12} md={6}>
        <Statistic title={t("stats.added")} value={normalized.added} valueStyle={{ color: "#389e0d" }} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("stats.modified")} value={normalized.modified} valueStyle={{ color: "#d48806" }} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("stats.removed")} value={normalized.removed} valueStyle={{ color: "#cf1322" }} />
      </Col>
      <Col xs={12} md={6}>
        <Statistic title={t("stats.same")} value={normalized.same} />
      </Col>
    </Row>
  );
}

export function DiffTable({ diffs }: { diffs: SchemaDiff[]; compact?: boolean }) {
  return (
    <SchemaDiffTrees
      diffs={buildEntries(diffs)}
      checkedKeys={[]}
      checkable={false}
      onCheckedKeysChange={() => {}}
    />
  );
}

function SchemaDiffTrees({
  diffs,
  checkedKeys,
  checkable = true,
  onCheckedKeysChange,
}: {
  diffs: DiffEntry[];
  checkedKeys: string[];
  checkable?: boolean;
  onCheckedKeysChange: (keys: string[]) => void;
}) {
  const { t } = useI18n();
  const sourceTree = useMemo(() => buildTree(diffs, "source"), [diffs]);
  const targetTree = useMemo(() => buildTree(diffs, "target"), [diffs]);

  if (diffs.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={t("schema.noDiffs")}
      />
    );
  }

  const handleCheck = (
    keys:
      | Key[]
      | {
          checked: Key[];
          halfChecked: Key[];
        },
  ) => {
    const nextKeys = Array.isArray(keys) ? keys : keys.checked;
    onCheckedKeysChange(nextKeys.map(String).filter((key) => key.startsWith("diff:")));
  };

  return (
    <Row gutter={10} className="schema-diff-trees">
      <Col xs={24} lg={12}>
        <Card size="small" title={t("schema.sourceTree")}>
          <Tree
            checkable={checkable}
            defaultExpandAll
            checkedKeys={checkedKeys}
            treeData={sourceTree}
            onCheck={handleCheck}
          />
        </Card>
      </Col>
      <Col xs={24} lg={12}>
        <Card size="small" title={t("schema.targetTree")}>
          <Tree
            checkable={checkable}
            defaultExpandAll
            checkedKeys={checkedKeys}
            treeData={targetTree}
            onCheck={handleCheck}
          />
        </Card>
      </Col>
    </Row>
  );
}

export function SqlPreview({ sql }: { sql: string }) {
  const { t } = useI18n();

  return (
    <section className="sql-preview">
      <Typography.Title level={5}>{t("schema.generatedSql")}</Typography.Title>
      <SqlCodePreview sql={sql || t("schema.noGeneratedSql")} />
    </section>
  );
}

function buildEntries(diffs: SchemaDiff[]): DiffEntry[] {
  return diffs.map((diff, index) => ({
    ...diff,
    key: `diff:${index}:${diff.objectType}:${diff.tableName}:${diff.columnName || ""}:${diff.diffType}`,
  }));
}

function sqlWithTableComments(entries: DiffEntry[]) {
  const tableNames = Array.from(new Set(entries.map((entry) => entry.tableName))).sort();
  return tableNames
    .map((tableName) => {
      const sections = ([
        ["added", "Added"],
        ["modified", "Modified"],
        ["removed", "Removed"],
      ] as const)
        .map(([diffType, label]) => {
          const statements = entries
            .filter((entry) => entry.tableName === tableName && entry.diffType === diffType)
            .map((entry) => entry.syncSql)
            .filter(Boolean);
          return statements.length > 0 ? `-- ${label}: ${statements.length}\n${statements.join("\n")}` : "";
        })
        .filter(Boolean);
      return sections.length > 0 ? `-- Table: ${tableName}\n${sections.join("\n")}` : "";
    })
    .filter(Boolean)
    .join("\n\n");
}

function buildTree(diffs: DiffEntry[], side: "source" | "target"): TreeDataNode[] {
  const tables = new Map<string, TreeDataNode[]>();

  diffs.forEach((diff) => {
    const nodes = tables.get(diff.tableName) || [];
    nodes.push({
      key: diff.key,
      title: <DiffNodeTitle diff={diff} side={side} />,
    });
    tables.set(diff.tableName, nodes);
  });

  return Array.from(tables.entries()).map(([tableName, children]) => ({
    key: `table:${side}:${tableName}`,
    title: <Typography.Text strong>{tableName}</Typography.Text>,
    children,
  }));
}

function DiffNodeTitle({ diff, side }: { diff: SchemaDiff; side: "source" | "target" }) {
  const { t } = useI18n();
  const label = diff.columnName || "(table)";
  const value = side === "source" ? diff.sourceValue : diff.targetValue;

  return (
    <Space size={6} wrap>
      <Typography.Text>{label}</Typography.Text>
      <Tag color={diffColor(diff.diffType)}>{diffTypeLabel(diff.diffType, t)}</Tag>
      {value && (
        <Typography.Text type="secondary" className="diff-node-value">
          {value}
        </Typography.Text>
      )}
    </Space>
  );
}

function diffTypeLabel(diffType: SchemaDiff["diffType"], t: ReturnType<typeof useI18n>["t"]) {
  if (diffType === "added") return t("stats.added");
  if (diffType === "modified") return t("stats.modified");
  if (diffType === "removed") return t("stats.removed");
  return diffType;
}
