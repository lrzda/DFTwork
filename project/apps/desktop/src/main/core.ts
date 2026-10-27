import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { app } from "electron";
import { DftworkError, type ErrorPayload } from "@dftwork/shared";

/**
 * 向上找到仓库根（含 Cargo.toml 的那一层）。
 *
 * 不能简单 `resolve(start, '..', '..')`——`app.getAppPath()` 的返回值取决于
 * 怎么启动：打包后是 `resources/app`，`electron .` 是 apps/desktop，
 * 而 `electron out/main/index.js` 是 `apps/desktop/out/main`。
 * 按目录结构猜层级迟早出错，找标志文件才稳。
 */
function findRepoRoot(start: string): string {
  let dir = start;
  for (let i = 0; i < 8; i += 1) {
    if (existsSync(path.join(dir, "Cargo.toml"))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return path.resolve(start, "..", "..");
}

/**
 * 内核可执行文件的位置。
 *
 * 开发态：`<repo>/target/{debug,release}/vaspcore.exe`
 * 打包态：`<resources>/bin/vaspcore.exe`（由 electron-builder 的 extraResources 放进去）
 */
export function coreBinary(): string {
  const exe = process.platform === "win32" ? "vaspcore.exe" : "vaspcore";
  if (app.isPackaged) {
    return path.join(process.resourcesPath, "bin", exe);
  }
  const repoRoot = findRepoRoot(app.getAppPath());
  const profile = process.env.DFTWORK_CORE_PROFILE ?? "debug";
  return path.join(repoRoot, "target", profile, exe);
}

interface CoreEnvelope<T> {
  ok: boolean;
  data?: T;
  error?: ErrorPayload;
}

/**
 * 调一次内核。
 *
 * 约定：内核 stdout 恰好一行 JSON；日志在 stderr。
 * 因此这里只取 stdout **最后一行** 解析——容忍将来可能出现的多行进度。
 */
export function callCore<T>(args: string[], timeoutMs = 120_000): Promise<T> {
  const bin = coreBinary();

  return new Promise<T>((resolve, reject) => {
    let child;
    try {
      child = spawn(bin, args, { windowsHide: true });
    } catch (e) {
      reject(
        new DftworkError({
          code: "EXTERNAL",
          message: `无法启动内核 ${bin}：${String(e)}`,
        }),
      );
      return;
    }

    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => {
      child.kill();
      reject(
        new DftworkError({
          code: "TIMEOUT",
          message: `内核 ${args[0]} 超过 ${timeoutMs} ms 未返回`,
        }),
      );
    }, timeoutMs);

    child.stdout?.on("data", (d: Buffer) => {
      stdout += d.toString("utf8");
    });
    child.stderr?.on("data", (d: Buffer) => {
      stderr += d.toString("utf8");
    });

    child.on("error", (e) => {
      clearTimeout(timer);
      reject(
        new DftworkError({
          code: "EXTERNAL",
          message: `内核不可用（${bin}）：${e.message}`,
        }),
      );
    });

    child.on("close", (code) => {
      clearTimeout(timer);
      const line = stdout
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean)
        .pop();

      if (!line) {
        reject(
          new DftworkError({
            code: "EXTERNAL",
            message: `内核没有输出（退出码 ${code}）${stderr ? "：" + stderr.slice(0, 400) : ""}`,
          }),
        );
        return;
      }

      let env: CoreEnvelope<T>;
      try {
        env = JSON.parse(line) as CoreEnvelope<T>;
      } catch {
        reject(
          new DftworkError({
            code: "EXTERNAL",
            message: `内核输出不是 JSON：${line.slice(0, 200)}`,
          }),
        );
        return;
      }

      if (!env.ok) {
        reject(new DftworkError(env.error ?? { code: "FAILED", message: "未知错误" }));
        return;
      }
      resolve(env.data as T);
    });
  });
}
