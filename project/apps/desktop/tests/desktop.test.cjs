const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const electron = require("electron");

const cases = [
  ["write recovery and rollback through the production UI", "write-recovery.e2e.cjs", 90_000],
  ["second instance exits and restores the first window", "single-instance.e2e.cjs", 60_000],
  ["stale IPC responses cannot replace newer UI state", "stale-responses.e2e.cjs", 90_000],
  ["Hermes chat resumes and follows the scanned project", "hermes-chat.e2e.cjs", 90_000],
];

function stopProcessTree(child) {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  if (process.platform !== "win32") {
    child.kill("SIGTERM");
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    const killer = spawn("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
      stdio: "ignore",
    });
    killer.once("error", () => {
      child.kill();
      resolve();
    });
    killer.once("close", resolve);
  });
}

async function runCase(file, timeout) {
  const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "dftwork-desktop-e2e-"));
  try {
    await new Promise((resolve, reject) => {
      const env = { ...process.env, DFTWORK_CORE_PROFILE: "debug", DFTWORK_E2E_SANDBOX: sandbox };
      delete env.ELECTRON_RUN_AS_NODE;
      delete env.DFTWORK_TEST_FAILPOINT;
      delete env.DFTWORK_SMOKE;
      delete env.DFTWORK_SMOKE_ROOT;
      const child = spawn(electron, [path.join(__dirname, file)], {
        cwd: __dirname,
        env,
        stdio: ["ignore", "pipe", "pipe"],
      });
      let output = "";
      let settled = false;
      const timer = setTimeout(() => {
        void stopProcessTree(child).finally(() => {
          finish(new Error(`${file} exceeded ${timeout} ms\n${output.slice(-4000)}`));
        });
      }, timeout);
      function finish(error) {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        if (output.trim()) console.log(output.trimEnd());
        if (error) reject(error);
        else resolve();
      }
      child.stdout.on("data", (chunk) => { output += chunk.toString(); });
      child.stderr.on("data", (chunk) => { output += chunk.toString(); });
      child.once("error", finish);
      child.once("close", (code, signal) => {
        finish(code === 0 ? undefined : new Error(`${file} exited with ${code ?? signal}\n${output.slice(-4000)}`));
      });
    });
  } finally {
    fs.rmSync(sandbox, { recursive: true, force: true });
  }
}

for (const [name, file, timeout] of cases) {
  test(name, { timeout: timeout + 10_000, concurrency: false }, async () => {
    assert.equal(fs.existsSync(path.join(__dirname, "../out/main/index.js")), true, "build Electron before running E2E tests");
    await runCase(file, timeout);
  });
}
