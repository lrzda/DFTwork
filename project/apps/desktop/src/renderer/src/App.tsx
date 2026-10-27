import { useCallback, useEffect, useState } from "react";
import {
  Alert,
  Button,
  Card,
  Empty,
  Input,
  Space,
  Spin,
  Table,
  Tag,
  Typography,
} from "antd";
import type { ColumnsType } from "antd/es/table";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type {
  ConvergenceSeries,
  HermesCapability,
  ScanReport,
  TaskStatusCode,
  TaskSummary,
  VersionInfo,
} from "@dftwork/shared";
import { api, describeError } from "./api";

const { Title, Text, Paragraph } = Typography;

const STATUS_COLOR: Record<TaskStatusCode, string> = {
  TASK_COMPLETED: "green",
  ERROR_DETECTED: "red",
  RUNNING: "blue",
  NO_OUTPUT_EVIDENCE: "default",
  UNKNOWN: "orange",
};

export default function App(): JSX.Element {
  // 冒烟/开发时可以通过 `?project=<路径>` 直接带初始目录（见 main/index.ts 的 DFTWORK_SMOKE_ROOT）
  const bootProject =
    new URLSearchParams(window.location.search).get("project") ?? "";
  const [root, setRoot] = useState(bootProject);
  const [scanning, setScanning] = useState(false);
  const [report, setReport] = useState<ScanReport | null>(null);
  const [selected, setSelected] = useState<TaskSummary | null>(null);
  const [series, setSeries] = useState<ConvergenceSeries | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ver, setVer] = useState<VersionInfo | null>(null);
  const [hermes, setHermes] = useState<HermesCapability | null>(null);

  useEffect(() => {
    api
      .version()
      .then(setVer)
      .catch((e) => setError(describeError(e)));
    api
      .hermesProbe()
      .then(setHermes)
      .catch(() => setHermes({ found: false, bin: null, version: null, supports_stream_json: false, reason: "探测失败" }));
  }, []);

  const doScan = useCallback(async (override?: string) => {
    const trimmed = (override ?? root).trim();
    if (!trimmed) {
      setError("先填项目根目录");
      return;
    }
    setScanning(true);
    setError(null);
    setSelected(null);
    setSeries(null);
    try {
      setReport(await api.scan(trimmed));
    } catch (e) {
      setError(describeError(e));
      setReport(null);
    } finally {
      setScanning(false);
    }
  }, [root]);

  // 带初始目录时（冒烟模式 / 从命令行传入）自动扫一次
  useEffect(() => {
    if (bootProject) void doScan(bootProject);
    // 只在首次挂载时跑一次
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pick = useCallback(
    async (task: TaskSummary) => {
      setSelected(task);
      setSeries(null);
      if (!report) return;
      const sep = report.root.includes("\\") ? "\\" : "/";
      const abs = `${report.root.replace(/[\\/]+$/, "")}${sep}${task.rel_path.replace(/\//g, sep)}`;
      try {
        setSeries(await api.convergence(abs));
      } catch (e) {
        setError(describeError(e));
      }
    },
    [report],
  );

  // 冒烟模式：扫完自动选中第一个已完成的任务，让曲线图有内容可截
  useEffect(() => {
    if (!bootProject || !report || selected) return;
    const first =
      report.tasks.find((t) => t.status === "TASK_COMPLETED") ?? report.tasks[0];
    if (first) void pick(first);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [report, bootProject, selected]);

  const columns: ColumnsType<TaskSummary> = [
    {
      title: "任务",
      dataIndex: "rel_path",
      render: (v: string) => <Text code>{v}</Text>,
    },
    {
      title: "状态",
      dataIndex: "status",
      width: 160,
      render: (_: unknown, r) => (
        <Tag color={STATUS_COLOR[r.status]}>{r.status_label}</Tag>
      ),
    },
    {
      title: "依据",
      dataIndex: "reason",
      ellipsis: true,
    },
    {
      title: "输入",
      width: 110,
      render: (_: unknown, r) =>
        r.inputs_complete ? (
          <Tag color="green">齐全</Tag>
        ) : (
          <Tag color="orange">缺 {r.missing_inputs.join("/")}</Tag>
        ),
    },
  ];

  const chartData =
    series?.steps.map((s) => ({
      step: s.index,
      energy: s.energy_ev,
      force: s.max_force_ev_per_a,
    })) ?? [];

  return (
    <div className="app">
      <div className="header">
        <Title level={4} style={{ margin: 0 }}>
          DFTwork
        </Title>
        <Text type="secondary">
          {ver ? `内核 ${ver.name} ${ver.version} · schema v${ver.schema_version}` : "内核未连接"}
        </Text>
        <div className="spacer" />
        <Tag color={hermes?.found ? "green" : "default"}>
          {hermes?.found ? `Hermes ${hermes.version ?? "已就绪"}` : "AI 未安装（可选）"}
        </Tag>
      </div>

      <Space.Compact style={{ width: "100%", marginBottom: 12 }}>
        <Input
          placeholder="项目根目录，例如 D:\projects\my-vasp-project"
          value={root}
          onChange={(e) => setRoot(e.target.value)}
          onPressEnter={() => void doScan()}
        />
        <Button type="primary" loading={scanning} onClick={() => void doScan()}>
          扫描
        </Button>
      </Space.Compact>

      {error && (
        <Alert
          type="error"
          showIcon
          message={error}
          closable
          onClose={() => setError(null)}
          style={{ marginBottom: 12 }}
        />
      )}

      <div className="body">
        <Card
          size="small"
          title={
            report
              ? `任务 ${report.tasks.length} 个 · 遍历 ${report.dirs_scanned} 个目录 · ${report.elapsed_ms} ms`
              : "扫描结果"
          }
          className="left"
        >
          <Spin spinning={scanning}>
            {report && report.tasks.length > 0 ? (
              <Table
                size="small"
                rowKey="rel_path"
                columns={columns}
                dataSource={report.tasks}
                pagination={{ pageSize: 50, size: "small" }}
                onRow={(r) => ({
                  onClick: () => void pick(r),
                  style: {
                    cursor: "pointer",
                    background: selected?.rel_path === r.rel_path ? "rgba(22,119,255,0.12)" : undefined,
                  },
                })}
              />
            ) : (
              <Empty description={report ? "没有找到含 OUTCAR / vasprun.xml 的目录" : "先填路径再扫描"} />
            )}
          </Spin>
        </Card>

        <Card size="small" title={selected ? `收敛 · ${selected.rel_path}` : "收敛曲线"} className="right">
          {series && series.steps.length > 0 ? (
            <ResponsiveContainer width="100%" height={320}>
              <LineChart data={chartData} margin={{ top: 8, right: 16, bottom: 8, left: 8 }}>
                <CartesianGrid strokeDasharray="3 3" opacity={0.3} />
                <XAxis dataKey="step" tick={{ fontSize: 12 }} label={{ value: "离子步", position: "insideBottom", offset: -4 }} />
                <YAxis yAxisId="e" tick={{ fontSize: 12 }} width={80} />
                <YAxis yAxisId="f" orientation="right" tick={{ fontSize: 12 }} width={60} />
                <Tooltip />
                <Line yAxisId="e" type="monotone" dataKey="energy" name="F (eV)" dot={false} strokeWidth={2} />
                <Line yAxisId="f" type="monotone" dataKey="force" name="最大力 (eV/Å)" dot={false} strokeWidth={2} />
              </LineChart>
            </ResponsiveContainer>
          ) : (
            <Empty description={selected ? "没有读到离子步（OSZICAR 缺失或格式待支持）" : "选一个任务"} />
          )}
          {series && (
            <Paragraph type="secondary" style={{ marginTop: 8, marginBottom: 0, fontSize: 12 }}>
              来源 {series.source} · {series.steps.length} 步
              {series.truncated ? " · 已截断" : ""}
            </Paragraph>
          )}
        </Card>
      </div>
    </div>
  );
}
