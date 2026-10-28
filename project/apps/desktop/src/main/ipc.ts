import { ipcMain } from "electron";
import {
  DftworkError,
  IPC,
  type ConvergenceRequest,
  type ErrorPayload,
  type SceneRequest,
  type ScanRequest,
  type TaskRequest,
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

  // 四项属性查询。OUTCAR 大的时候（几百 MB）要等十几秒，所以放宽超时。
  ipcMain.handle(IPC.scf, (_e, req: TaskRequest) =>
    wrap(() => callCore(["scf", req.taskAbsPath])),
  );
  ipcMain.handle(IPC.forces, (_e, req: TaskRequest) =>
    wrap(() => callCore(["forces", req.taskAbsPath], 180_000)),
  );
  ipcMain.handle(IPC.vibrations, (_e, req: TaskRequest) =>
    wrap(() => callCore(["vibrations", req.taskAbsPath], 180_000)),
  );
  ipcMain.handle(IPC.magnetization, (_e, req: TaskRequest) =>
    wrap(() => callCore(["magnetization", req.taskAbsPath], 180_000)),
  );
}
