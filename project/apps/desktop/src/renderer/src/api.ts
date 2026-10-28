import type {
  AnalysisReport,
  BuildPlan,
  BuildReceipt,
  CompareReport,
  ConvergenceSeries,
  DftworkApi,
  Envelope,
  ErrorPayload,
  ForcesResponse,
  HermesCapability,
  MagnetizationReport,
  ScanReport,
  RollbackReport,
  ScfSeries,
  StructureScene,
  VersionInfo,
  VibrationReport,
} from "@dftwork/shared";

declare global {
  interface Window {
    dftwork: DftworkApi;
  }
}

/**
 * 渲染进程唯一的数据入口。
 *
 * 组件不许直接 `window.dftwork.*`——所有调用都从这里走，
 * 这样「信封解包 / 错误码保留 / 超时提示」只有一份实现。
 */

export class ApiError extends Error {
  readonly code: ErrorPayload["code"];
  constructor(payload: ErrorPayload) {
    super(payload.message);
    this.name = "ApiError";
    this.code = payload.code;
  }
}

async function unwrap<T>(p: Promise<Envelope<T>>): Promise<T> {
  const r = await p;
  if (!r.ok) throw new ApiError(r.error);
  return r.data;
}

function missingBridge(): never {
  throw new ApiError({
    code: "EXTERNAL",
    message: "preload 没有加载：window.dftwork 不存在（检查 contextIsolation / preload 路径）",
  });
}

const bridge = (): DftworkApi => {
  if (!window.dftwork) missingBridge();
  return window.dftwork;
};

export const api = {
  version: (): Promise<VersionInfo> => unwrap(bridge().version()),
  hermesProbe: (): Promise<HermesCapability> => unwrap(bridge().hermesProbe()),
  scan: (root: string, maxDepth = 8): Promise<ScanReport> =>
    unwrap(bridge().scan({ root, maxDepth })),
  convergence: (taskAbsPath: string): Promise<ConvergenceSeries> =>
    unwrap(bridge().convergence({ taskAbsPath })),
  scene: (fileAbsPath: string, pbc = true): Promise<StructureScene> =>
    unwrap(bridge().scene({ fileAbsPath, pbc })),
  scf: (taskAbsPath: string): Promise<ScfSeries> => unwrap(bridge().scf({ taskAbsPath })),
  forces: (taskAbsPath: string): Promise<ForcesResponse> =>
    unwrap(bridge().forces({ taskAbsPath })),
  vibrations: (taskAbsPath: string): Promise<VibrationReport> =>
    unwrap(bridge().vibrations({ taskAbsPath })),
  magnetization: (taskAbsPath: string): Promise<MagnetizationReport> =>
    unwrap(bridge().magnetization({ taskAbsPath })),
  compare: (taskAbsPaths: string[]): Promise<CompareReport> =>
    unwrap(bridge().compare({ taskAbsPaths })),
  build: (
    root: string,
    sourceRel: string,
    targetRel: string,
    overrides: [string, string][],
  ): Promise<BuildPlan> => unwrap(bridge().build({ root, sourceRel, targetRel, overrides })),
  apply: (plan: BuildPlan): Promise<BuildReceipt> => unwrap(bridge().apply({ plan })),
  rollback: (receipt: BuildReceipt): Promise<RollbackReport> =>
    unwrap(bridge().rollback({ receipt })),
  analyze: (
    taskAbsPath: string,
    model?: string,
    provider?: string,
    maxTurns?: number,
  ): Promise<AnalysisReport> =>
    unwrap(bridge().analyze({ taskAbsPath, model, provider, maxTurns })),
};

/** 把任意异常转成一句人话，用于界面提示。 */
export function describeError(e: unknown): string {
  if (e instanceof ApiError) return `[${e.code}] ${e.message}`;
  if (e instanceof Error) return e.message;
  return String(e);
}
