/**
 * IPC 通道名与签名。
 *
 * 只在这里定义一次：主进程注册、preload 暴露、渲染进程调用都引用它，
 * 写错通道名会直接编译不过。
 */

export const IPC = {
  coreVersion: "core:version",
  hermesProbe: "core:hermes-probe",
  scan: "core:scan",
  convergence: "core:convergence",
  scene: "core:scene",
  scf: "core:scf",
  forces: "core:forces",
  vibrations: "core:vibrations",
  magnetization: "core:magnetization",
  compare: "core:compare",
  build: "core:build",
  apply: "core:apply",
  rollback: "core:rollback",
} as const;

export type IpcChannel = (typeof IPC)[keyof typeof IPC];

export interface ScanRequest {
  root: string;
  maxDepth?: number;
}

export interface ConvergenceRequest {
  taskAbsPath: string;
}

export interface SceneRequest {
  fileAbsPath: string;
  /**
   * 成键是否按周期边界计算，默认 `true`。
   *
   * - `true`：最小镜像距离，晶体/周期体系用
   * - `false`：只按真实坐标距离，孤立分子、团簇或想看「盒子里实际摆成什么样」时用
   */
  pbc?: boolean;
}

/** 以模板任务生成输入计划。 */
export interface BuildRequest {
  root: string;
  /** 源任务，相对 root。 */
  sourceRel: string;
  /** 目标任务目录，相对 root。 */
  targetRel: string;
  /** INCAR 参数覆盖。 */
  overrides: [string, string][];
}

/** 放行一份计划。整个计划随请求传输，内核写临时文件后再执行。 */
export interface ApplyRequest {
  plan: import("./dto").BuildPlan;
}

export interface RollbackRequest {
  receipt: import("./dto").BuildReceipt;
}

/** 多任务对比。至少两个路径。 */
export interface CompareRequest {
  taskAbsPaths: string[];
}

/** 四项属性查询共用同一个入参形状：给任务目录或文件都行。 */
export interface TaskRequest {
  taskAbsPath: string;
}

/** `forces` 一次返回两样东西：每原子力和晶胞应力。 */
export interface ForcesResponse {
  forces: import("./dto").ForceReport;
  stress: import("./dto").StressReport;
}

/** 跨进程边界不抛异常，只回信封——Electron 会把异常变成没有 code 的字符串。 */
export type Envelope<T> =
  | { ok: true; data: T }
  | { ok: false; error: import("./errors").ErrorPayload };

/** preload 暴露到 `window.dftwork` 的完整形状（返回信封，由渲染进程解包）。 */
export interface DftworkApi {
  version(): Promise<Envelope<import("./dto").VersionInfo>>;
  hermesProbe(): Promise<Envelope<import("./dto").HermesCapability>>;
  scan(req: ScanRequest): Promise<Envelope<import("./dto").ScanReport>>;
  convergence(req: ConvergenceRequest): Promise<Envelope<import("./dto").ConvergenceSeries>>;
  scene(req: SceneRequest): Promise<Envelope<import("./dto").StructureScene>>;
  scf(req: TaskRequest): Promise<Envelope<import("./dto").ScfSeries>>;
  forces(req: TaskRequest): Promise<Envelope<ForcesResponse>>;
  vibrations(req: TaskRequest): Promise<Envelope<import("./dto").VibrationReport>>;
  magnetization(req: TaskRequest): Promise<Envelope<import("./dto").MagnetizationReport>>;
  compare(req: CompareRequest): Promise<Envelope<import("./dto").CompareReport>>;
  build(req: BuildRequest): Promise<Envelope<import("./dto").BuildPlan>>;
  apply(req: ApplyRequest): Promise<Envelope<import("./dto").BuildReceipt>>;
  rollback(req: RollbackRequest): Promise<Envelope<import("./dto").RollbackReport>>;
}
