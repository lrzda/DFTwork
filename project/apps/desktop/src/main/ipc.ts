import { app, ipcMain } from "electron";
import { randomUUID } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { DftworkError, IPC, type ErrorPayload } from "@dftwork/shared";
import { callCore } from "./core";
import { parseRequest, requests } from "./validation";

type Envelope<T> =
  | { ok: true; data: T }
  | { ok: false; error: ErrorPayload };

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

async function withTempInput<T>(
  kind: string,
  extension: string,
  content: string,
  fn: (file: string) => Promise<T>,
): Promise<T> {
  const tmp = path.join(app.getPath("temp"), `dftwork-${kind}-${randomUUID()}.${extension}`);
  await fs.writeFile(tmp, content, "utf8");
  try {
    return await fn(tmp);
  } finally {
    await fs.unlink(tmp).catch(() => undefined);
  }
}

function appendModelArgs(
  args: string[],
  req: { model?: string; provider?: string; maxTurns?: number },
): void {
  if (req.model) args.push("--model", req.model);
  if (req.provider) args.push("--provider", req.provider);
  if (req.maxTurns) args.push("--max-turns", String(req.maxTurns));
}

export function registerIpc(): void {
  ipcMain.handle(IPC.coreVersion, () => wrap(() => callCore(["version"], 15_000)));
  ipcMain.handle(IPC.hermesProbe, () => wrap(() => callCore(["hermes-probe"], 20_000)));

  ipcMain.handle(IPC.scan, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.scan, value);
      return callCore(["scan", req.root, "--max-depth", String(req.maxDepth ?? 8)]);
    }),
  );
  ipcMain.handle(IPC.convergence, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.convergence, value);
      return callCore(["convergence", req.taskAbsPath]);
    }),
  );
  ipcMain.handle(IPC.scene, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.scene, value);
      const args = ["scene", req.fileAbsPath];
      if (req.pbc === false) args.push("--no-pbc");
      return callCore(args);
    }),
  );
  ipcMain.handle(IPC.scf, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.task, value);
      return callCore(["scf", req.taskAbsPath]);
    }),
  );
  ipcMain.handle(IPC.forces, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.task, value);
      return callCore(["forces", req.taskAbsPath], 180_000);
    }),
  );
  ipcMain.handle(IPC.vibrations, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.task, value);
      return callCore(["vibrations", req.taskAbsPath], 180_000);
    }),
  );
  ipcMain.handle(IPC.magnetization, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.task, value);
      return callCore(["magnetization", req.taskAbsPath], 180_000);
    }),
  );
  ipcMain.handle(IPC.compare, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.compare, value);
      return callCore(["compare", ...req.taskAbsPaths]);
    }),
  );
  ipcMain.handle(IPC.build, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.build, value);
      const args = ["build", req.sourceRel, req.targetRel, "--root", req.root];
      for (const [k, v] of req.overrides) args.push("--set", `${k}=${v}`);
      return callCore(args);
    }),
  );
  ipcMain.handle(IPC.apply, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.apply, value);
      return withTempInput("plan", "json", JSON.stringify(req.plan), (tmp) =>
        callCore(["apply", tmp], 120_000),
      );
    }),
  );
  ipcMain.handle(IPC.analyze, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.analyze, value);
      const args = ["analyze", req.taskAbsPath];
      appendModelArgs(args, req);
      return callCore(args, 300_000);
    }),
  );
  ipcMain.handle(IPC.hermesChat, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.hermesChat, value);
      return withTempInput("hermes-chat", "txt", req.message, (tmp) => {
        const args = ["hermes-chat", req.root, tmp];
        appendModelArgs(args, req);
        if (req.sessionId) args.push("--resume", req.sessionId);
        return callCore(args, 300_000);
      });
    }),
  );
  ipcMain.handle(IPC.trajectory, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.trajectory, value);
      return callCore(["trajectory", req.taskAbsPath], 60_000);
    }),
  );
  ipcMain.handle(IPC.rollback, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.rollback, value);
      return withTempInput("receipt", "json", JSON.stringify(req.receipt), (tmp) =>
        callCore(["rollback", tmp], 120_000),
      );
    }),
  );
  ipcMain.handle(IPC.projectState, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.projectRoot, value);
      return callCore(["project-state", req.root]);
    }),
  );
  ipcMain.handle(IPC.discardPlan, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.projectRoot, value);
      return callCore(["discard-plan", req.root]);
    }),
  );
  ipcMain.handle(IPC.recover, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.projectRoot, value);
      return callCore(["recover", req.root], 120_000);
    }),
  );
  ipcMain.handle(IPC.pruneBackups, (_e, value: unknown) =>
    wrap(() => {
      const req = parseRequest(requests.projectRoot, value);
      return callCore(["prune-backups", req.root], 120_000);
    }),
  );
}
