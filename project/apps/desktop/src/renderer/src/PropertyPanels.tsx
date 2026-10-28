/**
 * 属性面板：SCF 电子步、振动频谱、力与应力、磁矩。
 *
 * 每个面板都是纯展示组件——数据由 `App` 按需加载后传进来，
 * 面板自己不发起 IPC。这样切标签页时才请求，不会一选中任务就
 * 把几百 MB 的 OUTCAR 扫三遍。
 */
import { useMemo, useState } from "react";
import { Alert, Card, Empty, Segmented, Space, Statistic, Table, Tag, Typography } from "antd";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type {
  CompareReport,
  ForcesResponse,
  InputDiff,
  MagnetizationReport,
  ScfSeries,
  VibrationReport,
} from "@dftwork/shared";

const { Text, Paragraph } = Typography;

// ───────────────────────────── SCF 电子步 ─────────────────────────────

type ScfMetric = "energy" | "rms" | "dE";

/**
 * SCF 迭代曲线。
 *
 * 默认看 `rms`（电荷密度残差）——这是判断「这一步 SCF 收敛得好不好」的直接依据；
 * 能量在最后几步的变化已经小于显示精度，看不出东西。
 */
export function ScfPanel({ data }: { data: ScfSeries | null }) {
  const [metric, setMetric] = useState<ScfMetric>("rms");
  const [blockIdx, setBlockIdx] = useState<number>(-1);

  const blocks = data?.blocks ?? [];
  const current = blocks.length === 0 ? null : blocks[blockIdx < 0 ? blocks.length - 1 : blockIdx];

  const chart = useMemo(() => {
    if (!current) return [];
    return current.steps
      .map((s) => ({
        step: s.index,
        energy: s.energy_ev,
        rms: s.rms,
        dE: s.delta_e_ev === null ? null : Math.abs(s.delta_e_ev),
      }))
      .filter((d) => (metric === "energy" ? Number.isFinite(d.energy) : true));
  }, [current, metric]);

  if (!data || blocks.length === 0) {
    return <Empty description="这份 OSZICAR 里没有 SCF 电子步（可能只记录了离子步）" />;
  }

  const last = current!.steps[current!.steps.length - 1];

  return (
    <Space direction="vertical" size={8} style={{ width: "100%" }}>
      <Space size={8} wrap>
        <Segmented
          size="small"
          value={metric}
          onChange={(v) => setMetric(v as ScfMetric)}
          options={[
            { label: "残差 rms", value: "rms" },
            { label: "能量", value: "energy" },
            { label: "|dE|", value: "dE" },
          ]}
        />
        {blocks.length > 1 && (
          <Segmented
            size="small"
            value={blockIdx < 0 ? blocks.length - 1 : blockIdx}
            onChange={(v) => setBlockIdx(Number(v))}
            options={blocks.map((b, i) => ({
              label: `第 ${b.ionic_index} 步`,
              value: i,
            }))}
          />
        )}
        <Text type="secondary" style={{ fontSize: 12 }}>
          {current!.steps.length} 个电子步
          {data.truncated ? "（只保留最近若干步）" : ""}
        </Text>
      </Space>

      <div style={{ height: 240 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={chart} margin={{ top: 6, right: 12, bottom: 4, left: 6 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.28} />
            <XAxis dataKey="step" tick={{ fontSize: 11 }} />
            <YAxis
              tick={{ fontSize: 11 }}
              width={68}
              scale={metric === "energy" ? "auto" : "log"}
              domain={metric === "energy" ? ["auto", "auto"] : ["auto", "auto"]}
            />
            <Tooltip formatter={(v: number) => (Number.isFinite(v) ? v.toExponential(4) : "—")} />
            <Line
              type="monotone"
              dataKey={metric}
              name={metric === "energy" ? "能量 (eV)" : metric === "rms" ? "rms" : "|dE| (eV)"}
              dot={false}
              strokeWidth={2}
              stroke={metric === "energy" ? "#69b1ff" : "#95de64"}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <Space size={16} wrap>
        <Text style={{ fontSize: 12 }}>
          末步能量 <Text code>{Number.isFinite(last.energy_ev) ? last.energy_ev.toFixed(8) : "—"}</Text> eV
        </Text>
        <Text style={{ fontSize: 12 }}>
          rms <Text code>{last.rms ?? "—"}</Text>
        </Text>
        <Text style={{ fontSize: 12 }}>
          |dE| <Text code>{last.delta_e_ev === null ? "—" : Math.abs(last.delta_e_ev).toExponential(3)}</Text>
        </Text>
        <Text style={{ fontSize: 12 }}>
          算法 <Tag>{last.algorithm}</Tag>
        </Text>
      </Space>
    </Space>
  );
}

// ───────────────────────────── 振动频谱 ─────────────────────────────

/**
 * 振动模式柱状图。
 *
 * 虚频用红色单独标出——它不是「小频率」，而是结构在该方向上不稳定，
 * 混在实频里当普通柱子看会严重误导。
 */
export function VibrationPanel({ data }: { data: VibrationReport | null }) {
  if (!data) return <Empty description="加载中…" />;
  if (data.modes.length === 0) {
    return <Empty description={data.note || "这个任务没有振动数据"} />;
  }

  const chart = data.modes.map((m) => ({
    index: m.index,
    freq: m.frequency_thz,
    imaginary: m.is_imaginary,
  }));

  const realMax = Math.max(...data.modes.map((m) => m.frequency_thz));

  return (
    <Space direction="vertical" size={8} style={{ width: "100%" }}>
      {data.imaginary_count > 0 && (
        <Alert
          type="warning"
          showIcon
          message={`检测到 ${data.imaginary_count} 个虚频`}
          description="虚频说明结构在简谐近似下不稳定：可能没优化到位、或有原子跑偏。频率最低的那个通常对应最可能发生畸变的方向。"
        />
      )}

      <div style={{ height: 230 }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={chart} margin={{ top: 6, right: 12, bottom: 4, left: 6 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.28} />
            <XAxis dataKey="index" tick={{ fontSize: 10 }} interval={Math.ceil(chart.length / 20)} />
            <YAxis tick={{ fontSize: 11 }} width={56} unit=" THz" />
            <Tooltip
              formatter={(v: number, _n, item) => [
                `${v.toFixed(4)} THz${(item?.payload as { imaginary?: boolean })?.imaginary ? "（虚频）" : ""}`,
                `模式 ${(item?.payload as { index?: number })?.index}`,
              ]}
            />
            <ReferenceLine y={0} stroke="#8b9bb4" />
            <Bar dataKey="freq" name="频率">
              {chart.map((d) => (
                <Cell key={d.index} fill={d.imaginary ? "#ff4d4f" : "#69b1ff"} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>

      <Space size={16} wrap>
        <Text style={{ fontSize: 12 }}>
          模式总数 <Text code>{data.modes.length}</Text>
        </Text>
        <Text style={{ fontSize: 12, color: data.imaginary_count > 0 ? "#ff7875" : undefined }}>
          虚频 <Text code>{data.imaginary_count}</Text>
        </Text>
        <Text style={{ fontSize: 12 }}>
          最低 <Text code>{Math.min(...data.modes.map((m) => m.frequency_thz)).toFixed(3)}</Text> THz
        </Text>
        <Text style={{ fontSize: 12 }}>
          最高 <Text code>{realMax.toFixed(3)}</Text> THz
        </Text>
        {data.hessian && (
          <Text type="secondary" style={{ fontSize: 12 }}>
            力常数矩阵 {data.hessian.n_dof}×{data.hessian.n_dof}
          </Text>
        )}
      </Space>

      {data.hessian && <HessianHeatmap hessian={data.hessian} />}

      <Paragraph type="secondary" style={{ fontSize: 12, marginBottom: 0 }}>
        {data.note}
      </Paragraph>
    </Space>
  );
}

/**
 * 力常数矩阵热图。
 *
 * 用 canvas 手绘而不是给每个格子放一个 DOM 元素：
 * 72×72 = 5184 个格子，用 div 会直接卡死渲染。
 */
function HessianHeatmap({ hessian }: { hessian: { n_dof: number; data: number[] } }) {
  const [scale, setScale] = useState<"log" | "linear">("log");
  const size = 220;

  const canvasRef = (canvas: HTMLCanvasElement | null) => {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const n = hessian.n_dof;
    const px = size / n;

    // 先找极值：Hessian 的对角元远大于非对角元，直接线性映射会只剩对角线可见
    let maxAbs = 0;
    for (const v of hessian.data) maxAbs = Math.max(maxAbs, Math.abs(v));
    if (maxAbs === 0) maxAbs = 1;

    ctx.clearRect(0, 0, size, size);
    for (let i = 0; i < n; i += 1) {
      for (let j = 0; j < n; j += 1) {
        const v = hessian.data[i * n + j] ?? 0;
        let t: number;
        if (scale === "log") {
          t = Math.log10(1 + (Math.abs(v) / maxAbs) * 9) / 1; // 0..1
        } else {
          t = Math.abs(v) / maxAbs;
        }
        t = Math.min(1, Math.max(0, t));
        // 负值偏蓝、正值偏红
        const color =
          v >= 0
            ? `rgba(${Math.round(220 * t + 40)}, ${Math.round(80 * (1 - t) + 40)}, ${Math.round(80 * (1 - t) + 40)}, 0.95)`
            : `rgba(${Math.round(60 * (1 - t) + 30)}, ${Math.round(120 * (1 - t) + 60)}, ${Math.round(220 * t + 40)}, 0.95)`;
        ctx.fillStyle = color;
        ctx.fillRect(j * px, i * px, px + 0.5, px + 0.5);
      }
    }
  };

  return (
    <Space size={10} align="start">
      <canvas ref={canvasRef} width={220} height={220} style={{ borderRadius: 4 }} />
      <Space direction="vertical" size={2}>
        <Text style={{ fontSize: 12 }}>力常数矩阵</Text>
        <Text type="secondary" style={{ fontSize: 11 }}>
          行/列 = 3N 个自由度
        </Text>
        <Text type="secondary" style={{ fontSize: 11 }}>
          红 = 正，蓝 = 负
        </Text>
        <Text type="secondary" style={{ fontSize: 11 }}>
          对角元远大于非对角元
        </Text>
        <Segmented
          size="small"
          value={scale}
          onChange={(v) => setScale(v as "log" | "linear")}
          options={[
            { label: "log", value: "log" },
            { label: "线性", value: "linear" },
          ]}
        />
      </Space>
    </Space>
  );
}

// ───────────────────────────── 力与应力 ─────────────────────────────

/** 受力排行 + 应力分量。 */
export function ForcePanel({ data }: { data: ForcesResponse | null }) {
  if (!data) return <Empty description="加载中…" />;
  const set = data.forces.sets[0];
  if (!set) return <Empty description="这份 OUTCAR 里没有力的数据段" />;

  const top = [...set.atoms]
    .sort((a, b) => b.magnitude_ev_per_a - a.magnitude_ev_per_a)
    .slice(0, 15)
    .map((a) => ({
      atom: `#${a.index}`,
      magnitude: Number(a.magnitude_ev_per_a.toFixed(4)),
    }));

  const stress = data.stress.sets[0];

  return (
    <Space direction="vertical" size={10} style={{ width: "100%" }}>
      <Space size={20} wrap>
        <Statistic
          title="最大力"
          value={set.max_force_ev_per_a}
          precision={4}
          suffix="eV/Å"
          valueStyle={{ fontSize: 18 }}
        />
        <Statistic title="原子数" value={set.atoms.length} valueStyle={{ fontSize: 18 }} />
        {stress && (
          <Statistic
            title="压强"
            value={stress.pressure_kb}
            precision={3}
            suffix="kB"
            valueStyle={{ fontSize: 18 }}
          />
        )}
        <Statistic
          title="力数据步数"
          value={data.forces.total_sets}
          valueStyle={{ fontSize: 18 }}
        />
      </Space>

      <div style={{ height: 260 }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={top} layout="vertical" margin={{ top: 4, right: 24, bottom: 4, left: 8 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.25} />
            <XAxis type="number" tick={{ fontSize: 11 }} unit=" eV/Å" />
            <YAxis type="category" dataKey="atom" tick={{ fontSize: 11 }} width={44} />
            <Tooltip formatter={(v: number) => [`${v} eV/Å`, "|F|"]} />
            <Bar dataKey="magnitude" fill="#faad14" />
          </BarChart>
        </ResponsiveContainer>
      </div>
      <Text type="secondary" style={{ fontSize: 12 }}>
        显示受力最大的 15 个原子（按 |F| 排序）
      </Text>

      {stress && (
        <Card size="small" title="应力张量（kB，1 kB = 0.1 GPa）" styles={{ body: { padding: 10 } }}>
          <Table
            size="small"
            pagination={false}
            rowKey="k"
            dataSource={[
              { k: "XX", v: stress.components_kb[0] },
              { k: "YY", v: stress.components_kb[1] },
              { k: "ZZ", v: stress.components_kb[2] },
              { k: "XY", v: stress.components_kb[3] },
              { k: "YZ", v: stress.components_kb[4] },
              { k: "ZX", v: stress.components_kb[5] },
            ].map((r) => ({
              ...r,
              // 剪应力理论上对称，正负只反映方向
              display: r.v.toFixed(4),
            }))}
            columns={[
              { title: "分量", dataIndex: "k", width: 70 },
              { title: "数值 (kB)", dataIndex: "display" },
            ]}
          />
        </Card>
      )}
    </Space>
  );
}

// ───────────────────────────── 磁矩 ─────────────────────────────

/** 总磁矩随离子步的演化。 */
export function MagnetizationPanel({ data }: { data: MagnetizationReport | null }) {
  if (!data) return <Empty description="加载中…" />;
  if (data.steps.length === 0) {
    return <Empty description={data.note || "这份 OUTCAR 没有磁矩数据"} />;
  }

  const chart = data.steps.map((s) => ({
    step: s.index,
    total: s.total_mu_b,
    interstitial: s.interstitial_mu_b,
  }));

  const first = data.steps[0].total_mu_b;
  const last = data.steps[data.steps.length - 1].total_mu_b;

  return (
    <Space direction="vertical" size={8} style={{ width: "100%" }}>
      {!data.per_atom_available && (
        <Alert
          type="info"
          showIcon
          message="只有总磁矩，没有逐原子磁矩"
          description={data.note}
        />
      )}

      <Space size={20} wrap>
        <Statistic title="首步" value={first} precision={2} suffix="μB" valueStyle={{ fontSize: 18 }} />
        <Statistic title="末步" value={last} precision={2} suffix="μB" valueStyle={{ fontSize: 18 }} />
        <Statistic
          title="末步球间磁矩"
          value={data.steps[data.steps.length - 1].interstitial_mu_b ?? 0}
          precision={2}
          suffix="μB"
          valueStyle={{ fontSize: 18 }}
        />
      </Space>

      <div style={{ height: 240 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={chart} margin={{ top: 6, right: 12, bottom: 4, left: 6 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.28} />
            <XAxis dataKey="step" tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} width={56} unit=" μB" />
            <Tooltip formatter={(v: number) => (v === null ? "—" : v.toFixed(4))} />
            <ReferenceLine y={0} stroke="#8b9bb4" />
            <Line type="monotone" dataKey="total" name="总磁矩" dot={false} strokeWidth={2} stroke="#b37feb" />
            <Line
              type="monotone"
              dataKey="interstitial"
              name="球间磁矩"
              dot={false}
              strokeWidth={1.5}
              stroke="#5cdbd3"
              strokeDasharray="4 3"
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
      <Text type="secondary" style={{ fontSize: 12 }}>
        共 {data.steps.length} 个离子步。球间磁矩是原子球之间区域的贡献，
        它与总磁矩之差近似为「球内」磁矩总和。
      </Text>
    </Space>
  );
}

// ───────────────────────────── 多任务对比 ─────────────────────────────

const COMPARE_COLORS = ["#69b1ff", "#95de64", "#ffc53d", "#ff7875", "#b37feb"];

/**
 * 多任务对比：INCAR 差异表 + 收敛曲线叠加。
 *
 * 差异表只列**不完全相同**的参数——几十项全列出来，真正要看的那两三行会被淹掉。
 * 注意「值不同」不等于「物理上不同」：`1.0E-6` 和 `0.1E-05` 是同一个数，
 * 这里如实显示原文差异，判断留给用户。
 */
export function ComparePanel({ data }: { data: CompareReport | null }) {
  // 把各任务的离子步按 step 合并成一张表，供多线叠加
  const merged = useMemo(() => {
    if (!data) return [];
    const byStep = new Map<number, Record<string, number | null>>();
    data.series.forEach((s, i) => {
      for (const step of s.steps) {
        const row = byStep.get(step.index) ?? { step: step.index };
        row[`e${i}`] = step.energy_ev ?? null;
        row[`f${i}`] = step.max_force_ev_per_a ?? null;
        byStep.set(step.index, row);
      }
    });
    return [...byStep.values()].sort((a, b) => (a.step as number) - (b.step as number));
  }, [data]);

  if (!data) {
    return <Empty description="在左侧任务表勾选两个以上任务，然后点「对比」" />;
  }

  return (
    <Space direction="vertical" size={10} style={{ width: "100%" }}>
      <Space size={8} wrap>
        {data.tasks.map((name, i) => (
          <Tag key={name} color={COMPARE_COLORS[i % COMPARE_COLORS.length]}>
            {name}
          </Tag>
        ))}
        <Text type="secondary" style={{ fontSize: 12 }}>
          相同参数 {data.same_count} 项 · 差异 {data.input_diff.length} 项
        </Text>
      </Space>

      {data.input_diff.length === 0 ? (
        <Alert type="success" showIcon message="这些任务的 INCAR 完全相同" />
      ) : (
        <Card size="small" title="INCAR 差异" styles={{ body: { padding: 0, maxHeight: 170, overflow: "auto" } }}>
          <Table
            size="small"
            pagination={false}
            rowKey="key"
            dataSource={data.input_diff}
            columns={[
              {
                title: "参数",
                dataIndex: "key",
                width: 130,
                fixed: "left",
                render: (v: string) => <Text code>{v}</Text>,
              },
              ...data.tasks.map((name, i) => ({
                title: name,
                key: `t${i}`,
                render: (_: unknown, r: InputDiff) => {
                  const v = r.values[i];
                  return v === null ? (
                    <Text type="secondary">未设置</Text>
                  ) : (
                    <span style={{ color: COMPARE_COLORS[i % COMPARE_COLORS.length] }}>{v}</span>
                  );
                },
              })),
            ]}
          />
        </Card>
      )}

      <div style={{ height: 150 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={merged} margin={{ top: 6, right: 12, bottom: 4, left: 6 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.28} />
            <XAxis dataKey="step" tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} width={76} />
            <Tooltip />
            <Legend iconSize={8} wrapperStyle={{ fontSize: 11 }} />
            {data.series.map((s, i) => (
              <Line
                key={`e${i}`}
                type="monotone"
                dataKey={`e${i}`}
                name={`${data.tasks[i]} F`}
                dot={false}
                strokeWidth={2}
                stroke={COMPARE_COLORS[i % COMPARE_COLORS.length]}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div style={{ height: 140 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={merged} margin={{ top: 6, right: 12, bottom: 4, left: 6 }}>
            <CartesianGrid strokeDasharray="3 3" opacity={0.28} />
            <XAxis dataKey="step" tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} width={76} unit=" eV/Å" />
            <Tooltip />
            <Legend iconSize={8} wrapperStyle={{ fontSize: 11 }} />
            {data.series.map((s, i) => (
              <Line
                key={`f${i}`}
                type="monotone"
                dataKey={`f${i}`}
                name={`${data.tasks[i]} maxF`}
                dot={false}
                strokeWidth={2}
                strokeDasharray="4 3"
                stroke={COMPARE_COLORS[i % COMPARE_COLORS.length]}
              />
            ))}
          </LineChart>
        </ResponsiveContainer>
      </div>
    </Space>
  );
}
