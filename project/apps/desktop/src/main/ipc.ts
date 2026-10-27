import { ipcMain } from "electron";
import {
  DftworkError,
  IPC,
  type ConvergenceRequest,
  type ErrorPayload,
  type SceneRequest,
  type ScanRequest,
} from "@dftwork/shared";
import { callCore } from "./core";

type Envelope<T> =
  | { ok: true; data: T }
  | { ok: false; error: ErrorPayload };

/** 把异常收敛成可序列化的信封，避免 Electron 把错误变成 "Error invoking remote method"。 */
async function wrap<T>(fn: () => Promise<T>): Promise<Envelope<T>> {
  try {
    return { ok: true, data: await fn() };
  } catch (e) {
    if (e instanceof DftworkError) {
      return { ok: false, error: { code: e.code, message: e.message } };
    }
    return { ok: false, error: { code: "FAILED", message: String(e) } };
  }
}

export function registerIpc(): void {
  ipcMain.handle(IPC.coreVersion, () => wrap(() => callCore(["version"], 15_000)));

  ipcMain.handle(IPC.hermesProbe, () => wrap(() => callCore(["hermes-probe"], 20_000)));

  ipcMain.handle(IPC.scan, (_e, req: ScanRequest) =>
    wrap(() =>
      callCore([
        "scan",
        req.root,
        "--max-depth",
        String(req.maxDepth ?? 8),
      ]),
    ),
  );

  ipcMain.handle(IPC.convergence, (_e, req: ConvergenceRequest) =>
    wrap(() => callCore(["convergence", req.taskAbsPath])),
  );

  ipcMain.handle(IPC.scene, (_e, req: SceneRequest) => {
    // 默认按周期边界算成键；显式传 false 才用真实坐标
    const args = ["scene", req.fileAbsPath];
    if (req.pbc === false) args.push("--no-pbc");
    return wrap(() => callCore(args));
  });
}
