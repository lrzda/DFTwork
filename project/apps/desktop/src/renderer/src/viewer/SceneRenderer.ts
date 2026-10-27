import type { StructureScene } from "@dftwork/shared";

/**
 * 结构渲染器接口（ADR-011）。
 *
 * 界面只认这个接口，不直接 import three.js——理由有三个：
 * 1. 将来换渲染后端（WebGPU / 服务端渲染 / 静态导出图）时，组件代码一行不用改；
 * 2. 单测可以在 Node 里塞一个假渲染器，不需要 WebGL；
 * 3. 「自定义渲染效果」有明确的挂载点，而不是散落在组件里的 if 分支。
 */
export interface RenderOptions {
  /** 原子球半径倍率。1 = 共价半径原尺寸；0.3 左右最好看。 */
  atomScale: number;
  /** 是否画晶胞棱。 */
  showCell: boolean;
  /** 是否画化学键（`scene.bonds` 为空时无效）。 */
  showBonds: boolean;
  /**
   * 是否把原子折回原胞（PBC wrap）。
   *
   * POSCAR / CONTCAR / MD 轨迹里的坐标可以落在盒子外（分数坐标 <0 或 >1），
   * 打开后所有原子被平移回 `[0,1)` 的分数区间。
   *
   * ⚠️ 代价：跨越边界的分子会被劈成两半，成键线要横穿整个盒子。
   * 所以它默认**关闭**——保持数据原样最不容易误导。
   */
  wrapPbc: boolean;
  /** 背景色，`transparent` 表示透明。 */
  background: string;
  /** 晶胞棱颜色。 */
  cellColor: string;
}

export const DEFAULT_OPTIONS: RenderOptions = {
  atomScale: 0.32,
  showCell: true,
  showBonds: true,
  wrapPbc: false,
  background: "transparent",
  cellColor: "#5b7089",
};

/**
 * 自定义渲染效果的扩展点。
 *
 * 首版不实现任何效果，但接口先留着：将来「按原子属性着色（电荷/磁矩）」、
 * 「高亮某个原子」「画力矢量箭头」这类需求都从这里进，不用改结构。
 */
export interface RenderEffect {
  readonly id: string;
  /** 场景重建后调用，可往 root 组里挂自己的对象。 */
  apply(ctx: EffectContext): void;
  /** 每次渲染前调用，用于动画。 */
  update?(deltaSeconds: number): void;
  dispose(): void;
}

export interface EffectContext {
  /** 场景根节点（已经居中对齐到原点）。 */
  root: unknown;
  scene: StructureScene;
  options: RenderOptions;
}

export interface SceneRenderer {
  /** 实现名，用于日志与界面上显示。 */
  readonly name: string;

  /** 挂到容器上。容器尺寸变化时必须调用 `resize()`。 */
  mount(container: HTMLElement): void;

  /** 换一份结构数据；会清掉上一份的所有对象。 */
  setScene(scene: StructureScene): void;

  /** 改渲染参数（会原地更新，不重建场景数据）。 */
  setOptions(options: Partial<RenderOptions>): void;

  /** 容器尺寸变了。 */
  resize(): void;

  /** 回到初始机位。 */
  resetView(): void;

  /** 注册自定义效果（首版留空实现，见 RenderEffect）。 */
  addEffect(effect: RenderEffect): void;

  /** 释放 GPU 资源。组件卸载时必须调用。 */
  dispose(): void;
}
