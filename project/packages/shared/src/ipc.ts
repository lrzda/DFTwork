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
}
