/**
 * 轨迹播放器：把 XDATCAR 的每一帧连续播出来，看结构怎么变。
 *
 * 优化任务 = 看弛豫过程；MD = 看整个盒子在动。
 *
 * 两条性能约束决定了实现方式：
 * 1. **播放时只改原子位置，不重建场景**。每帧重建 90 个球 + 材质表 +
 *    包围盒，20 fps 下会直接卡死。首帧 `setScene` 一次，之后走
 *    `updatePositions`（只改 mesh.position）。
 * 2. **播放时不算键**。成键要按最小镜像逐对判定，73 原子约 2600 对，
 *    每帧算一遍不可能跟上 20 fps；而且 MD 里成键本来就该从一个
 *    轨迹均值去看，不是逐帧快照。键留给静态视图。
 */
import { useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Button,
  Empty,
  InputNumber,
  Slider,
  Space,
  Spin,
  Switch,
  Tag,
  Tooltip,
  Typography,
} from "antd";
import type { StructureScene, TrajectoryReport, Vec3 } from "@dftwork/shared";
import { api, describeError } from "./api";
import { ThreeRenderer } from "./viewer/ThreeRenderer";
import { DEFAULT_OPTIONS, type RenderOptions } from "./viewer/SceneRenderer";

const { Text } = Typography;

/** 把 `elements` + `counts` 展开成逐原子元素名。 */
function expandElements(elements: string[], counts: number[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < elements.length; i++) {
    for (let k = 0; k < (counts[i] ?? 0); k++) out.push(elements[i]);
  }
  return out;
}

/** 取第 i 帧，包成渲染器认识的 StructureScene。 */
function sceneAt(r: TrajectoryReport, i: number, symbols: string[]): StructureScene {
  const f = r.frames[Math.min(i, r.frames.length - 1)];
  return {
    cell: r.cell as [Vec3, Vec3, Vec3],
    atoms: f.positions.map((p, k) => ({
      element: symbols[k] ?? "X",
      position: p,
      species_index: k,
    })),
    bonds: [],
    comment: r.comment,
    selective_dynamics: false,
    scale: 1,
    source_file: "XDATCAR",
  };
}

export function TrajectoryPlayer({
  taskAbsPath,
  taskName,
  pbc,
  onPbcChange,
}: {
  taskAbsPath: string | null;
  taskName: string | null;
  pbc: boolean;
  onPbcChange: (v: boolean) => void;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<ThreeRenderer | null>(null);
  /** 需要重建场景（切换任务、改渲染参数）而不是只挪原子。 */
  const rebuildRef = useRef(true);
  const [ready, setReady] = useState(false);
  const [version, setVersion] = useState(0);

  const [report, setReport] = useState<TrajectoryReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 冒烟 / 深链用：?frame=120 直接定位到某一帧
  const [frame, setFrame] = useState(() => {
    const v = Number(new URLSearchParams(window.location.search).get("frame"));
    return Number.isFinite(v) && v > 0 ? Math.floor(v) - 1 : 0;
  });
  const [playing, setPlaying] = useState(false);
  const [fps, setFps] = useState(10);
  const [options, setOptions] = useState<RenderOptions>({
    ...DEFAULT_OPTIONS,
    showBonds: false, // 播放时不算键，见文件头
  });

  // URL 里的初始帧（?frame=N）只该在数据到位后生效一次；
  // 加载过程中如果直接 setFrame(0) 会把它冲掉。
  const initialFrameRef = useRef(frame);

  const symbols = useMemo(
    () => (report ? expandElements(report.elements, report.counts) : []),
    [report],
  );

  // ── 渲染器：容器出现之后再挂载。
  //
  // ⚠️ 依赖必须是 `taskAbsPath` 而不是 `[]`。没有选中任务时组件返回 `Empty`，
  // 容器根本不在 DOM 里，此时 `hostRef.current` 是 null。如果依赖写 `[]`，
  // 这个 effect 只跑一次、拿到 null 就退出了，之后即使容器出现也不会再来挂载——
  // 症状就是：数据正常、控件正常、中间一片黑，且渲染器实例始终是 null。
  // 换成任务时销毁重建也顺带处理了 WebGL 上下文泄漏。
  useEffect(() => {
    if (!taskAbsPath) return undefined;
    const host = hostRef.current;
    if (!host) return undefined;
    if (rendererRef.current) return undefined;

    const renderer = new ThreeRenderer();
    renderer.mount(host);
    rendererRef.current = renderer;
    setReady(true);
    rebuildRef.current = true;
    setVersion((v) => v + 1);

    return () => {
      renderer.dispose();
      rendererRef.current = null;
      setReady(false);
    };
  }, [taskAbsPath]);

  // ── PBC 跟着父组件
  useEffect(() => {
    setOptions((o) => (o.wrapPbc === pbc ? o : { ...o, wrapPbc: pbc }));
  }, [pbc]);

  // ── 拉数据
  useEffect(() => {
    if (!taskAbsPath) {
      setReport(null);
      setError(null);
      return undefined;
    }
    let alive = true;
    setLoading(true);
    setError(null);
    setReport(null);
    setPlaying(false);
    api
      .trajectory(taskAbsPath)
      .then((r) => {
        if (!alive) return;
        setReport(r);
        const init = Math.min(initialFrameRef.current, Math.max(r.frames.length - 1, 0));
        setFrame(init);
        rebuildRef.current = true;
        setVersion((v) => v + 1);
      })
      .catch((e) => {
        if (alive) setError(describeError(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [taskAbsPath]);

  // ── 渲染参数变化：先让渲染器更新，再标记重建
  useEffect(() => {
    const r = rendererRef.current;
    if (!r || !ready) return;
    r.setOptions(options);
    rebuildRef.current = true;
    setVersion((v) => v + 1);
  }, [options, ready]);

  // ── 帧推进：需要重建才 setScene，否则只挪原子
  useEffect(() => {
    const r = rendererRef.current;
    if (!r || !report || !ready) return;
    if (rebuildRef.current) {
      r.setScene(sceneAt(report, frame, symbols));
      rebuildRef.current = false;
    } else {
      r.updatePositions(report.frames[Math.min(frame, report.frames.length - 1)].positions);
    }
  }, [frame, report, ready, version, symbols]);

  // ── 播放：按 FPS 定时推进，末帧回到首帧循环
  useEffect(() => {
    if (!playing || !report || report.frames.length < 2) return undefined;
    const interval = Math.max(1000 / Math.max(fps, 1), 16);
    const id = window.setInterval(() => {
      setFrame((f) => (f + 1) % report.frames.length);
    }, interval);
    return () => window.clearInterval(id);
  }, [playing, fps, report]);

  const total = report?.frames.length ?? 0;
  const current = report?.frames[Math.min(frame, Math.max(total - 1, 0))];

  if (!taskAbsPath) return <Empty description="在左侧选一个任务" />;

  // ⚠️ 这里**不能**按 loading / error 提前 return。
  // 3D 容器必须从第一次渲染就存在，否则挂载 effect
  // （`useEffect(..., [])` → `renderer.mount(hostRef.current)`）拿到的是 null，
  // 渲染器根本不会建。症状就是：数据、控件都正常，中间一片黑。
  // 所以状态一律用覆盖层表达，容器始终在场。
  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}>
      {/* ── 顶栏 */}
      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginBottom: 6 }}>
        <Text strong style={{ fontSize: 13 }}>
          {taskName ?? "轨迹"}
        </Text>
        {report && (
          <>
            <Tag color="blue">{report.total_frames} 帧</Tag>
            <Tag>{symbols.length} 个原子</Tag>
            {report.sampled && (
              <Tooltip title="帧数超过 600，已均匀抽样；首尾两帧一定保留">
                <Tag color="orange">已抽样</Tag>
              </Tooltip>
            )}
            {report.comment && (
              <Text type="secondary" style={{ fontSize: 11 }}>
                {report.comment}
              </Text>
            )}
          </>
        )}
        <div style={{ flex: 1 }} />
        <Tooltip title={pbc ? "PBC 开启：原子折回原胞" : "PBC 关闭：保持真实坐标"}>
          <Space size={4}>
            <Text style={{ fontSize: 12, color: pbc ? "#69b1ff" : "#8b9bb4" }}>PBC</Text>
            <Switch size="small" checked={pbc} onChange={onPbcChange} />
          </Space>
        </Tooltip>
      </div>

      {/* ── 3D 画布：始终挂载 */}
      <div
        ref={hostRef}
        style={{
          flex: 1,
          minHeight: 240,
          position: "relative",
          borderRadius: 6,
          overflow: "hidden",
        }}
      >
        {loading && (
          <div style={overlayStyle}>
            <Space>
              <Spin size="small" />
              <Text type="secondary">正在读 XDATCAR…</Text>
            </Space>
          </div>
        )}

        {!loading && error && (
          <div style={overlayStyle}>
            <div style={{ maxWidth: 420, padding: 16 }}>
              <Alert
                type="info"
                showIcon
                message="没有可播放的轨迹"
                description={
                  <>
                    <div>{error}</div>
                    <div style={{ fontSize: 12, marginTop: 6, color: "#8b9bb4" }}>
                      轨迹来自任务目录下的 <Text code>XDATCAR</Text>。只有真正跑过的任务才有它；
                      单点计算（<Text code>NSW=0</Text>）通常只有一帧。
                    </div>
                  </>
                }
              />
            </div>
          </div>
        )}
      </div>

      {/* ── 播放控制（有数据才出现） */}
      {report && total > 0 && (
        <div style={{ marginTop: 6 }}>
          <Space size={8} wrap style={{ marginBottom: 4 }}>
            <Button
              size="small"
              onClick={() => {
                setPlaying(false);
                setFrame(0);
              }}
              disabled={total < 2}
            >
              ⏮
            </Button>
            <Button
              size="small"
              type={playing ? "default" : "primary"}
              onClick={() => {
                if (!playing && frame >= total - 1) setFrame(0);
                setPlaying((p) => !p);
              }}
              disabled={total < 2}
            >
              {playing ? "⏸ 暂停" : "▶ 播放"}
            </Button>
            <Button
              size="small"
              onClick={() => {
                setPlaying(false);
                setFrame(Math.max(total - 1, 0));
              }}
              disabled={total < 2}
            >
              ⏭
            </Button>

            <Tooltip title="输入帧号直接跳过去">
              <Space size={4}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  第
                </Text>
                <InputNumber
                  size="small"
                  min={1}
                  max={Math.max(total, 1)}
                  value={frame + 1}
                  onChange={(v) => {
                    if (typeof v === "number") {
                      setPlaying(false);
                      setFrame(Math.min(Math.max(v - 1, 0), Math.max(total - 1, 0)));
                    }
                  }}
                  style={{ width: 72 }}
                  controls={false}
                />
                <Text type="secondary" style={{ fontSize: 12 }}>
                  / {total} 帧
                </Text>
              </Space>
            </Tooltip>

            <Tooltip title="每秒播放多少帧，范围 1–60">
              <Space size={4}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  FPS
                </Text>
                <InputNumber
                  size="small"
                  min={1}
                  max={60}
                  value={fps}
                  onChange={(v) => {
                    if (typeof v === "number") setFps(Math.min(Math.max(v, 1), 60));
                  }}
                  style={{ width: 66 }}
                  controls={false}
                />
              </Space>
            </Tooltip>

            {current && (
              <Text type="secondary" style={{ fontSize: 11 }}>
                文件帧号 {current.index}
              </Text>
            )}
          </Space>

          {/* ── 时间轴：整个 step 一条横轴 */}
          <Slider
            min={1}
            max={Math.max(total, 1)}
            value={frame + 1}
            onChange={(v) => {
              setPlaying(false);
              setFrame(Math.min(Math.max(v - 1, 0), Math.max(total - 1, 0)));
            }}
            tooltip={{ formatter: (v) => `第 ${v} 帧` }}
            disabled={total < 2}
          />
        </div>
      )}
    </div>
  );
}

/** 覆盖在 3D 容器上的提示层。 */
const overlayStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "rgba(10, 14, 22, 0.72)",
  zIndex: 2,
};
