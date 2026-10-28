import { app, ipcMain } from "electron";
import fs from "node:fs/promises";
import path from "node:path";
import {
  DftworkError,
  IPC,
  type ConvergenceRequest,
  type ErrorPayload,
  type SceneRequest,
  type ScanRequest,
  type AnalyzeRequest,
  type ApplyRequest,
  type BuildRequest,
  type CompareRequest,
  type RollbackRequest,
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
  ipcMain.handle(IPC.compare, (_e, req: CompareRequest) =>
    wrap(() => callCore(["compare", ...req.taskAbsPaths])),
  );

  ipcMain.handle(IPC.build, (_e, req: BuildRequest) => {
    const args = ["build", req.sourceRel, req.targetRel, "--root", req.root];
    for (const [k, v] of req.overrides) args.push("--set", `${k}=${v}`);
    return wrap(() => callCore(args));
  });

  // 内核的 apply/rollback 读文件，而计划体积可能超命令行长度上限，
  // 所以这里落一个临时 JSON 交给它，执行完就删。
  ipcMain.handle(IPC.apply, async (_e, req: ApplyRequest) => {
    const tmp = path.join(app.getPath("temp"), `dftwork-plan-${Date.now()}.json`);
    await fs.writeFile(tmp, JSON.stringify(req.plan), "utf8");
    try {
      return await wrap(() => callCore(["apply", tmp], 120_000));
    } finally {
      await fs.unlink(tmp).catch(() => undefined);
    }
  });

  // AI 分析可能要跑几十秒到几分钟：超时放宽到 5 分钟，
  // 内核自己还有 --timeout 兜底（默认 180 秒）
  ipcMain.handle(IPC.analyze, (_e, req: AnalyzeRequest) => {
    const args = ["analyze", req.taskAbsPath];
    if (req.model) args.push("--model", req.model);
    if (req.provider) args.push("--provider", req.provider);
    if (req.maxTurns) args.push("--max-turns", String(req.maxTurns));
    return wrap(() => callCore(args, 300_000));
  });

  ipcMain.handle(IPC.rollback, async (_e, req: RollbackRequest) => {
    const tmp = path.join(app.getPath("temp"), `dftwork-receipt-${Date.now()}.json`);
    await fs.writeFile(tmp, JSON.stringify(req.receipt), "utf8");
    try {
      return await wrap(() => callCore(["rollback", tmp], 120_000));
    } finally {
      await fs.unlink(tmp).catch(() => undefined);
    }
  });
}
