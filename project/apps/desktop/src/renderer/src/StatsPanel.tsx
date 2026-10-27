import { useMemo } from "react";
import { Card, Empty, Space, Tag, Tree, Typography } from "antd";
import type { DataNode } from "antd/es/tree";
import { Cell, Legend, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";
import type { ScanReport, TaskSummary } from "@dftwork/shared";

const { Text } = Typography;

/** 状态码 → 颜色。用码分组而不是中文标签：文案以后改了配色也不会错位。 */
const STATUS_COLORS: Record<string, string> = {
  TASK_COMPLETED: "#52c41a",
  RUNNING: "#1677ff",
  ERROR_DETECTED: "#ff4d4f",
  NO_OUTPUT_EVIDENCE: "#faad14",
  UNKNOWN: "#8c8c8c",
};

const KIND_COLORS: Record<string, string> = {
  RELAXATION: "#1677ff",
  STATIC_SCF: "#52c41a",
  NEB: "#722ed1",
  FREQUENCY: "#faad14",
  AIMD: "#13c2c2",
  UNKNOWN: "#8c8c8c",
};

interface Slice {
  key: string;
  label: string;
  value: number;
  percent: number;
}

/** 按某个码分组并算百分比，按数量从多到少排序。 */
function groupBy(
  tasks: TaskSummary[],
  codeOf: (t: TaskSummary) => string,
  labelOf: (t: TaskSummary) => string,
): Slice[] {
  const acc = new Map<string, { label: string; value: number }>();
  for (const t of tasks) {
    const code = codeOf(t);
    const cur = acc.get(code);
    if (cur) cur.value += 1;
    else acc.set(code, { label: labelOf(t), value: 1 });
  }
  const total = tasks.length || 1;
  return [...acc.entries()]
    .map(([key, v]) => ({
      key,
      label: v.label,
      value: v.value,
      percent: (v.value / total) * 100,
    }))
    .sort((a, b) => b.value - a.value);
}

interface DirNode {
  key: string;
  name: string;
  /** 这个目录（含所有子目录）里的任务数。 */
  count: number;
  children: Map<string, DirNode>;
}

/** 把 `a/b/c` 这样的相对路径还原成目录树，并逐层累计任务数。 */
function buildTree(tasks: TaskSummary[]): DirNode {
  const root: DirNode = { key: "", name: "根目录", count: 0, children: new Map() };
  for (const task of tasks) {
    const parts = task.rel_path.split("/").filter(Boolean);
    let cur = root;
    root.count += 1;
    let path = "";
    for (const part of parts) {
      path = path ? `${path}/${part}` : part;
      let next = cur.children.get(part);
      if (!next) {
        next = { key: path, name: part, count: 0, children: new Map() };
        cur.children.set(part, next);
      }
      next.count += 1;
      cur = next;
    }
  }
  return root;
}

function toAntdNodes(node: DirNode): DataNode {
  const children = [...node.children.values()]
    .sort((a, b) => b.count - a.count)
    .map(toAntdNodes);
  return {
    key: node.key || "__root__",
    title: (
      <span>
        {node.name} <Text type="secondary">（{node.count}）</Text>
      </span>
    ),
    children: children.length > 0 ? children : undefined,
  };
}

/** 一个饼图。数据只有一项时 recharts 不画扇区，所以单值时退化成一条提示。 */
function PiePanel({
  title,
  slices,
  colors,
}: {
  title: string;
  slices: Slice[];
  colors: Record<string, string>;
}) {
  return (
    <Card size="small" title={title} styles={{ body: { padding: 8 } }}>
      {slices.length === 0 ? (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description="没有任务" />
      ) : (
        <ResponsiveContainer width="100%" height={180}>
          <PieChart>
            <Pie
              data={slices}
              dataKey="value"
              nameKey="label"
              innerRadius={38}
              outerRadius={68}
              paddingAngle={1}
              stroke="none"
            >
              {slices.map((s) => (
                <Cell key={s.key} fill={colors[s.key] ?? "#8c8c8c"} />
              ))}
            </Pie>
            <Tooltip
              formatter={(v: number, _n, item) => [
                `${v} 个（${(item?.payload as Slice | undefined)?.percent.toFixed(1) ?? "0"}%）`,
                (item?.payload as Slice | undefined)?.label ?? "",
              ]}
            />
            <Legend
              verticalAlign="bottom"
              height={44}
              iconSize={8}
              formatter={(_v, entry) => {
                const p = entry?.payload as Slice | undefined;
                if (!p) return "";
                return (
                  <span style={{ fontSize: 12 }}>
                    {p.label} {p.value}（{p.percent.toFixed(1)}%）
                  </span>
                );
              }}
            />
          </PieChart>
        </ResponsiveContainer>
      )}
    </Card>
  );
}

/**
 * 扫描统计：状态分布、类型分布、任务目录树。
 *
 * 全部在前端由 `report.tasks` 现算——这些是纯聚合，不必让内核多跑一趟，
 * 也让「改个分组方式」不需要重新扫描。
 */
export function StatsPanel({ report }: { report: ScanReport | null }) {
  const status = useMemo(
    () => (report ? groupBy(report.tasks, (t) => t.status, (t) => t.status_label) : []),
    [report],
  );
  const kinds = useMemo(
    () => (report ? groupBy(report.tasks, (t) => t.kind, (t) => t.kind_label) : []),
    [report],
  );
  const tree = useMemo(() => (report ? toAntdNodes(buildTree(report.tasks)) : null), [report]);

  if (!report) {
    return <Empty description="先扫描一个项目目录" />;
  }
  if (report.tasks.length === 0) {
    return (
      <Empty
        description={`扫了 ${report.dirs_scanned} 个目录，没找到 VASP 任务（需要 OUTCAR 或 vasprun.xml）`}
      />
    );
  }

  return (
    <Space direction="vertical" size={10} style={{ width: "100%" }}>
      <Space size={6} wrap>
        <Text type="secondary" style={{ fontSize: 12 }}>
          扫描 {report.dirs_scanned} 个目录 · 找到 {report.tasks.length} 个任务 · 用时{" "}
          {report.elapsed_ms} ms
        </Text>
      </Space>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
        <PiePanel title="计算状态" slices={status} colors={STATUS_COLORS} />
        <PiePanel title="任务类型" slices={kinds} colors={KIND_COLORS} />
      </div>

      <Card
        size="small"
        title="任务目录"
        styles={{ body: { padding: 8, maxHeight: 260, overflow: "auto" } }}
        extra={
          <Text type="secondary" style={{ fontSize: 12 }}>
            {report.tasks.length} 个任务
          </Text>
        }
      >
        <Tree
          treeData={tree ? [tree] : []}
          defaultExpandAll={report.tasks.length < 60}
          showLine
          selectable={false}
          titleRender={(node) => node.title as React.ReactNode}
        />
      </Card>

      {report.pending_dirs.length > 0 && (
        <Card size="small" title={`只有输入、还没输出（${report.pending_dirs.length}）`}>
          <Space size={4} wrap>
            {report.pending_dirs.slice(0, 40).map((d) => (
              <Tag key={d}>{d}</Tag>
            ))}
            {report.pending_dirs.length > 40 && <Tag>…还有 {report.pending_dirs.length - 40} 个</Tag>}
          </Space>
        </Card>
      )}
    </Space>
  );
}
