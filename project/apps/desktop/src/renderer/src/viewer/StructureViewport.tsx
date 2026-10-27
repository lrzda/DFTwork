import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Empty, Slider, Space, Switch, Tag, Tooltip, Typography } from "antd";
import type { StructureScene } from "@dftwork/shared";
import { DEFAULT_OPTIONS, type RenderOptions } from "./SceneRenderer";
import { ThreeRenderer } from "./ThreeRenderer";
import { legendOf } from "./elements";
import { isInsideCell } from "./geometry";

const { Text } = Typography;

interface Props {
  scene: StructureScene | null;
  /** 正在从内核取数据。 */
  loading: boolean;
  /** 这份结构来自哪个文件（POSCAR / CONTCAR），界面要显示。 */
  sourceName: string | null;
  /** 出错信息（例如目录里没有结构文件）。 */
  error: string | null;
}

/**
 * 结构视图容器。
 *
 * 这个组件**不认识 three.js**：它只做三件事——把容器交出去、把数据递进去、
 * 卸载时让渲染器释放资源。所有绘制细节在 `ThreeRenderer` 里。
 */
export function StructureViewport({ scene, loading, sourceName, error }: Props): JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<ThreeRenderer | null>(null);
  const [ready, setReady] = useState(false);
  // 初始状态可由 `?wrap=1` 指定（冒烟截图 / 分享链接用），默认关闭
  const [options, setOptions] = useState<RenderOptions>(() => ({
    ...DEFAULT_OPTIONS,
    wrapPbc: new URLSearchParams(window.location.search).get("wrap") === "1",
  }));

  // 挂载一次：建渲染器，卸载时销毁（不销毁会泄漏 WebGL 上下文，切几次任务就白屏）
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return undefined;
    const renderer = new ThreeRenderer();
    renderer.mount(host);
    rendererRef.current = renderer;
    setReady(true);
    return () => {
      renderer.dispose();
      rendererRef.current = null;
      setReady(false);
    };
  }, []);

  // 结构数据变了就重画
  useEffect(() => {
    if (!ready) return;
    const renderer = rendererRef.current;
    if (!renderer || !scene) return;
    renderer.setScene(scene);
  }, [scene, ready]);

  // 渲染参数变了（会原地更新，必要时内部重建几何）
  useEffect(() => {
    if (!ready) return;
    rendererRef.current?.setOptions(options);
  }, [options, ready]);

  const symbols = useMemo(() => (scene ? scene.atoms.map((a) => a.element) : []), [scene]);
  const legend = useMemo(() => legendOf(symbols), [symbols]);

  /** 有多少原子落在原胞外——用户需要知道「图为什么看起来是散的」。 */
  const outsideCount = useMemo(() => {
    if (!scene) return 0;
    return scene.atoms.reduce(
      (n, a) => (isInsideCell(a.position, scene.cell) ? n : n + 1),
      0,
    );
  }, [scene]);
  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const s of symbols) m.set(s, (m.get(s) ?? 0) + 1);
    return m;
  }, [symbols]);

  const hasScene = Boolean(scene && scene.atoms.length > 0);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}>
      {/* ── 顶部信息条 */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "6px 4px 8px",
          flexWrap: "wrap",
        }}
      >
        <Text strong style={{ fontSize: 13 }}>
          结构
        </Text>
        {sourceName && <Tag color="geekblue">{sourceName}</Tag>}
        {scene && (
          <Text type="secondary" style={{ fontSize: 12 }}>
            {scene.atoms.length} 个原子
            {scene.bonds.length > 0 ? ` · ${scene.bonds.length} 根键` : " · 暂无成键信息"}
          </Text>
        )}
        {outsideCount > 0 && !options.wrapPbc && (
          <Tooltip title="这些原子的分数坐标落在 [0,1) 之外（MD 快照与优化后的结构很常见）。打开 PBC 开关可把它们折回原胞——代价是跨越边界的分子会被劈开">
            <Tag color="orange" style={{ cursor: "help", margin: 0 }}>
              {outsideCount} 个在盒外
            </Tag>
          </Tooltip>
        )}
        {loading && (
          <Text type="secondary" style={{ fontSize: 12 }}>
            读取中…
          </Text>
        )}
        <div style={{ flex: 1 }} />
        {hasScene && (
          <Space size={10}>
            <Tooltip
              title={
                options.wrapPbc
                  ? "已把所有原子折回 [0,1) 分数区间。跨界分子会被劈成两半"
                  : "把落在盒外的原子折回原胞（按 PBC 平移整数个晶格矢量）"
              }
            >
              <Space size={4} style={{ cursor: "help" }}>
                <Text
                  style={{
                    fontSize: 12,
                    color: options.wrapPbc ? "#69b1ff" : "#8b9bb4",
                    userSelect: "none",
                  }}
                >
                  PBC 折回
                </Text>
                <Switch
                  size="small"
                  checked={options.wrapPbc}
                  onChange={(v) => setOptions((o) => ({ ...o, wrapPbc: v }))}
                />
              </Space>
            </Tooltip>
            <Tooltip title="原子球大小">
              <Slider
                style={{ width: 96, margin: 0 }}
                min={0.12}
                max={0.75}
                step={0.01}
                value={options.atomScale}
                onChange={(v) => setOptions((o) => ({ ...o, atomScale: v }))}
              />
            </Tooltip>
            <Tooltip title="显示晶胞棱">
              <Switch
                size="small"
                checked={options.showCell}
                onChange={(v) => setOptions((o) => ({ ...o, showCell: v }))}
              />
            </Tooltip>
            <Button size="small" onClick={() => rendererRef.current?.resetView()}>
              复位
            </Button>
          </Space>
        )}
      </div>

      {/* ── 画布 */}
      <div
        ref={hostRef}
        style={{
          flex: 1,
          minHeight: 0,
          position: "relative",
          borderRadius: 8,
          overflow: "hidden",
          background:
            "radial-gradient(circle at 50% 45%, #16233a 0%, #0b1220 70%)",
          border: "1px solid rgba(91,112,137,0.35)",
        }}
      >
        {!hasScene && (
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                <span style={{ fontSize: 13 }}>
                  {error ?? (loading ? "读取结构…" : "选一个任务")}
                </span>
              }
            />
          </div>
        )}
      </div>

      {/* ── 底部图例 */}
      {legend.length > 0 && (
        <div style={{ display: "flex", gap: 14, flexWrap: "wrap", padding: "8px 4px 0" }}>
          {legend.map((item) => (
            <span key={item.symbol} style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <span
                style={{
                  width: 11,
                  height: 11,
                  borderRadius: "50%",
                  background: item.color,
                  boxShadow: "inset 0 -2px 3px rgba(0,0,0,0.35)",
                  border: "1px solid rgba(255,255,255,0.25)",
                }}
              />
              <Text style={{ fontSize: 12, color: "#c9d4e4" }}>
                {item.symbol}
                <Text type="secondary" style={{ fontSize: 11 }}>
                  {" "}
                  ×{counts.get(item.symbol) ?? 0}
                </Text>
              </Text>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
