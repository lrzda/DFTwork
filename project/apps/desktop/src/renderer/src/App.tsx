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
  Tabs,
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
  ForcesResponse,
  HermesCapability,
  MagnetizationReport,
  ScanReport,
  ScfSeries,
  StructureScene,
  TaskStatusCode,
  TaskSummary,
  VersionInfo,
  VibrationReport,
} from "@dftwork/shared";
import { api, describeError } from "./api";
import {
  ForcePanel,
  MagnetizationPanel,
  ScfPanel,
  VibrationPanel,
} from "./PropertyPanels";
import { StatsPanel } from "./StatsPanel";
import { StructureViewport } from "./viewer/StructureViewport";

const { Title, Text, Paragraph } = Typography;

/** 右侧标签页。 */
type PanelKey =
  | "structure"
  | "convergence"
  | "scf"
  | "vibration"
  | "force"
  | "magnetization";

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
  const [scene, setScene] = useState<StructureScene | null>(null);
  const [sceneLoading, setSceneLoading] = useState(false);
  const [sceneError, setSceneError] = useState<string | null>(null);
  /**
   * PBC 模式。同时决定两件事：原子是否折回原胞、成键是否按最小镜像算。
   * 切换它必须重新向内核要一次场景——成键是内核算的，前端不重复实现判据。
   */
  const [pbc, setPbc] = useState(
    // 默认开启；`?wrap=0` 可指定关闭（截图对比与分享链接用）
    () => new URLSearchParams(window.location.search).get("wrap") !== "0",
  );
  /** 左侧标签页：任务表 / 统计。可由 `?tab=stats` 指定初始值（截图与分享用）。 */
  const [tab, setTab] = useState(
    () => new URLSearchParams(window.location.search).get("tab") ?? "tasks",
  );
  /** 右侧当前标签页。可由 `?panel=vibration` 指定初始值（截图与分享用）。 */
  const [rightTab, setRightTab] = useState<PanelKey>(
    () => (new URLSearchParams(window.location.search).get("panel") as PanelKey) || "structure",
  );
  // 属性面板数据：为空表示「还没请求过」，切到对应标签页时才去要
  const [scfData, setScfData] = useState<ScfSeries | null>(null);
  const [forceData, setForceData] = useState<ForcesResponse | null>(null);
  const [vibData, setVibData] = useState<VibrationReport | null>(null);
  const [magData, setMagData] = useState<MagnetizationReport | null>(null);
  const [panelLoading, setPanelLoading] = useState<string | null>(null);
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

  const absPathOf = useCallback(
    (task: TaskSummary): string => {
      if (!report) return "";
      const sep = report.root.includes("\\") ? "\\" : "/";
      return `${report.root.replace(/[\\/]+$/, "")}${sep}${task.rel_path.replace(/\//g, sep)}`;
    },
    [report],
  );

  /** 只取结构。PBC 切换时会单独再调一次，不会连带重取收敛曲线。 */
  const fetchScene = useCallback(
    async (task: TaskSummary, mode: boolean) => {
      setSceneLoading(true);
      setSceneError(null);
      try {
        setScene(await api.scene(absPathOf(task), mode));
      } catch (e) {
        setSceneError(describeError(e));
      } finally {
        setSceneLoading(false);
      }
    },
    [absPathOf],
  );

  const pick = useCallback(
    async (task: TaskSummary) => {
      setSelected(task);
      setSeries(null);
      setScene(null);
      // 换任务要把属性面板的数据清掉，否则会显示上一个任务的结果
      setScfData(null);
      setForceData(null);
      setVibData(null);
      setMagData(null);
      // 结构与收敛互不阻塞：一边失败不该让另一边也空着
      void fetchScene(task, pbc);
      try {
        setSeries(await api.convergence(absPathOf(task)));
      } catch (e) {
        setError(describeError(e));
      }
    },
    [absPathOf, fetchScene, pbc],
  );

  /** 切换 PBC：成键判据在核心里，所以必须重新要一次场景。 */
  const handlePbcChange = useCallback(
    (v: boolean) => {
      setPbc(v);
      if (!selected) return;
      setScene(null);
      void fetchScene(selected, v);
    },
    [selected, fetchScene],
  );

  // 属性面板**按需加载**：切到哪个标签页才请求哪个。
  // 一选中任务就把 OUTCAR 扫三遍（力、振动、磁矩各一遍）是纯浪费，
  // 几百 MB 的文件每遍要 10 秒。
  useEffect(() => {
    if (!selected) return;
    const abs = absPathOf(selected);
    if (!abs) return;

    const need =
      (rightTab === "scf" && !scfData) ||
      (rightTab === "force" && !forceData) ||
      (rightTab === "vibration" && !vibData) ||
      (rightTab === "magnetization" && !magData);
    if (!need) return;

    let cancelled = false;
    const load = async (): Promise<void> => {
      setPanelLoading(rightTab);
      try {
        if (rightTab === "scf") {
          const d = await api.scf(abs);
          if (!cancelled) setScfData(d);
        } else if (rightTab === "force") {
          const d = await api.forces(abs);
          if (!cancelled) setForceData(d);
        } else if (rightTab === "vibration") {
          const d = await api.vibrations(abs);
          if (!cancelled) setVibData(d);
        } else if (rightTab === "magnetization") {
          const d = await api.magnetization(abs);
          if (!cancelled) setMagData(d);
        }
      } catch (e) {
        if (!cancelled) setError(describeError(e));
      } finally {
        if (!cancelled) setPanelLoading(null);
      }
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [rightTab, selected, absPathOf, scfData, forceData, vibData, magData]);

  // 冒烟模式：扫完自动选中一个任务，让面板有内容可截。
  // `?task=<rel_path>` 可指定具体任务；否则选第一个已完成的。
  useEffect(() => {
    if (!bootProject || !report || selected) return;
    const wanted = new URLSearchParams(window.location.search).get("task");
    const target = wanted
      ? report.tasks.find((t) => t.rel_path === wanted)
      : (report.tasks.find((t) => t.status === "TASK_COMPLETED") ?? report.tasks[0]);
    if (target) void pick(target);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [report, bootProject, selected]);

  const columns: ColumnsType<TaskSummary> = [
    {
      title: "任务",
      dataIndex: "rel_path",
      render: (v: string) => <Text code>{v}</Text>,
    },
    {
      title: "类型",
      dataIndex: "kind",
      width: 96,
      render: (_: unknown, r) => <Tag>{r.kind_label}</Tag>,
    },
    {
      title: "状态",
      dataIndex: "status",
      width: 150,
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
            <Tabs
              size="small"
              activeKey={tab}
              onChange={setTab}
              items={[
                {
                  key: "tasks",
                  label: `任务 ${report?.tasks.length ?? 0}`,
                  children:
                    report && report.tasks.length > 0 ? (
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
                            background:
                              selected?.rel_path === r.rel_path
                                ? "rgba(22,119,255,0.12)"
                                : undefined,
                          },
                        })}
                      />
                    ) : (
                      <Empty
                        description={
                          report
                            ? "没有找到含 OUTCAR / vasprun.xml 的目录"
                            : "先填路径再扫描"
                        }
                      />
                    ),
                },
                {
                  key: "stats",
                  label: "统计",
                  children: <StatsPanel report={report} />,
                },
              ]}
            />
          </Spin>
        </Card>

        <Card
          size="small"
          className="right-pane"
          title={selected ? selected.rel_path : "任务详情"}
          styles={{ body: { padding: "0 10px 8px" } }}
        >
          <Tabs
            size="small"
            activeKey={rightTab}
            onChange={(k) => setRightTab(k as PanelKey)}
            items={[
              {
                key: "structure",
                label: "结构",
                children: (
                  <StructureViewport
                    scene={scene}
                    loading={sceneLoading}
                    sourceName={scene?.source_file ?? null}
                    error={sceneError}
                    pbc={pbc}
                    onPbcChange={handlePbcChange}
                  />
                ),
              },
              {
                key: "convergence",
                label: "收敛",
                children:
                  series && series.steps.length > 0 ? (
                    <div style={{ height: "100%", minHeight: 220 }}>
                      <ResponsiveContainer width="100%" height="100%">
                        <LineChart data={chartData} margin={{ top: 8, right: 16, bottom: 8, left: 8 }}>
                          <CartesianGrid strokeDasharray="3 3" opacity={0.3} />
                          <XAxis
                            dataKey="step"
                            tick={{ fontSize: 12 }}
                            label={{ value: "离子步", position: "insideBottom", offset: -4 }}
                          />
                          <YAxis yAxisId="e" tick={{ fontSize: 12 }} width={80} />
                          <YAxis yAxisId="f" orientation="right" tick={{ fontSize: 12 }} width={60} />
                          <Tooltip />
                          <Line yAxisId="e" type="monotone" dataKey="energy" name="F (eV)" dot={false} strokeWidth={2} />
                          <Line yAxisId="f" type="monotone" dataKey="force" name="最大力 (eV/Å)" dot={false} strokeWidth={2} />
                        </LineChart>
                      </ResponsiveContainer>
                      <Paragraph type="secondary" style={{ marginTop: 4, marginBottom: 0, fontSize: 12 }}>
                        来源 {series.source} · {series.steps.length} 步
                        {series.truncated ? " · 已截断" : ""}
                      </Paragraph>
                    </div>
                  ) : (
                    <Empty description={selected ? "没有读到离子步" : "选一个任务"} />
                  ),
              },
              {
                key: "scf",
                label: "SCF",
                children: <ScfPanel data={scfData} />,
              },
              {
                key: "vibration",
                label: "振动",
                children: <VibrationPanel data={vibData} />,
              },
              {
                key: "force",
                label: "力 / 应力",
                children: <ForcePanel data={forceData} />,
              },
              {
                key: "magnetization",
                label: "磁矩",
                children: <MagnetizationPanel data={magData} />,
              },
            ]}
          />
          {panelLoading && (
            <Text type="secondary" style={{ fontSize: 12 }}>
              正在读取 {panelLoading} …
            </Text>
          )}
        </Card>
      </div>
    </div>
  );
}
