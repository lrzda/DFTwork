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
  /**
   * PBC 模式：开启时原子折回原胞、成键按最小镜像距离；
   * 关闭时原子保持真实坐标、成键只看真实距离。
   *
   * 这个状态由父组件持有，因为切换它需要**重新向内核请求场景**
   * （成键判据也跟着变），不是纯渲染层能决定的事。
   */
  pbc: boolean;
  onPbcChange: (v: boolean) => void;
}

/**
 * 结构视图容器。
 *
 * 这个组件**不认识 three.js**：它只做三件事——把容器交出去、把数据递进去、
 * 卸载时让渲染器释放资源。所有绘制细节在 `ThreeRenderer` 里。
 */
export function StructureViewport({
  scene,
  loading,
  sourceName,
  error,
  pbc,
  onPbcChange,
}: Props): JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const rendererRef = useRef<ThreeRenderer | null>(null);
  const [ready, setReady] = useState(false);
  // 冒烟验证用：?bonds=0 关闭成键显示（默认开启）。
  // 和 ?wrap=1 一样，只是给无人值守截图用的入口，不影响正常使用。
  const [options, setOptions] = useState<RenderOptions>(() => ({
    ...DEFAULT_OPTIONS,
    showBonds: new URLSearchParams(window.location.search).get("bonds") !== "0",
  }));

  // 渲染选项里的 wrapPbc 跟随父组件的 PBC 状态：
  // 内核已经按同一模式算好成键，显示与判定必须一致，否则会出现
  // 「原子折回了、键却按真实坐标连」这种自相矛盾的画面。
  useEffect(() => {
    setOptions((o) => (o.wrapPbc === pbc ? o : { ...o, wrapPbc: pbc }));
  }, [pbc]);

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
        {outsideCount > 0 && !pbc && (
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
                pbc
                  ? "PBC 开启：原子折回原胞，成键按最小镜像距离（跨界相邻的原子算成键）"
                  : "PBC 关闭：原子保持真实坐标，成键只看真实距离（不跨周期）"
              }
            >
              <Space size={4} style={{ cursor: "help" }}>
                <Text
                  style={{
                    fontSize: 12,
                    color: pbc ? "#69b1ff" : "#8b9bb4",
                    userSelect: "none",
                  }}
                >
                  PBC
                </Text>
                <Switch
                  size="small"
                  checked={pbc}
                  onChange={onPbcChange}
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
            {/* 「键」与「晶胞」是纯显示开关，只影响渲染，不重新请求内核。
                成键的**判据**由 PBC 开关决定（内核算好递过来），这里只管画不画。 */}
            <Tooltip title="显示化学键（判据：共价半径和 ×1.2，跟随 PBC 开关）">
              <Space size={4} style={{ cursor: "help" }}>
                <Text
                  style={{
                    fontSize: 12,
                    color: options.showBonds ? "#69b1ff" : "#8b9bb4",
                    userSelect: "none",
                  }}
                >
                  键
                </Text>
                <Switch
                  size="small"
                  checked={options.showBonds}
                  onChange={(v) => setOptions((o) => ({ ...o, showBonds: v }))}
                />
              </Space>
            </Tooltip>
            <Tooltip title="显示晶胞棱">
              <Space size={4} style={{ cursor: "help" }}>
                <Text
                  style={{
                    fontSize: 12,
                    color: options.showCell ? "#69b1ff" : "#8b9bb4",
                    userSelect: "none",
                  }}
                >
                  晶胞
                </Text>
                <Switch
                  size="small"
                  checked={options.showCell}
                  onChange={(v) => setOptions((o) => ({ ...o, showCell: v }))}
                />
              </Space>
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
