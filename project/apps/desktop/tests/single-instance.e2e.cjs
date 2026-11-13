const { app, BrowserWindow } = require("electron");
const { spawn } = require("node:child_process");
const path = require("node:path");

const sandbox = process.env.DFTWORK_E2E_SANDBOX;
if (!sandbox) throw new Error("DFTWORK_E2E_SANDBOX is required");
app.setPath("userData", path.join(sandbox, "user-data"));
app.disableHardwareAcceleration();
process.env.HERMES_BIN = path.join(sandbox, "hermes-not-installed");

const entry = path.join(__dirname, "../out/main/index.js");
let second;

async function stopSecond() {
  if (!second || second.exitCode !== null || second.signalCode !== null) return;
  if (process.platform !== "win32") {
    second.kill("SIGTERM");
    return;
  }
  await new Promise((resolve) => {
    const killer = spawn("taskkill", ["/PID", String(second.pid), "/T", "/F"], { stdio: "ignore" });
    killer.once("error", () => {
      second.kill();
      resolve();
    });
    killer.once("close", resolve);
  });
}

function withTimeout(promise, milliseconds, message) {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(message)), milliseconds);
    }),
  ]).finally(() => clearTimeout(timer));
}

require(entry);

if (process.env.DFTWORK_E2E_SECOND !== "1") {
  const watchdog = setTimeout(() => {
    console.error("FAIL single-instance-watchdog");
    void stopSecond().finally(() => app.exit(2));
  }, 45_000);

  app.whenReady().then(async () => {
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) throw new Error("The production main entry did not create a window");
    if (win.webContents.isLoading()) {
      await withTimeout(
        new Promise((resolve) => win.webContents.once("did-finish-load", resolve)),
        15_000,
        "First window load timed out",
      );
    }
    win.show();
    win.minimize();

    const raised = new Promise((resolve) => app.once("second-instance", () => resolve(true)));
    const env = { ...process.env, DFTWORK_E2E_SECOND: "1" };
    delete env.ELECTRON_RUN_AS_NODE;
    second = spawn(process.execPath, [__filename], { env, stdio: ["ignore", "pipe", "pipe"] });
    let secondOutput = "";
    second.stdout.on("data", (chunk) => { secondOutput += chunk.toString(); });
    second.stderr.on("data", (chunk) => { secondOutput += chunk.toString(); });
    const exited = new Promise((resolve, reject) => {
      second.once("error", reject);
      second.once("close", (code, signal) => resolve({ code, signal }));
    });

    let exit;
    try {
      exit = await withTimeout(exited, 12_000, "Second instance did not exit");
    } catch (error) {
      await stopSecond();
      throw new Error(`${error.message}; output: ${secondOutput.slice(-2000)}`);
    }
    await withTimeout(raised, 5_000, "First instance did not receive second-instance");
    const state = {
      exit,
      visible: win.isVisible(),
      restored: !win.isMinimized(),
      focused: win.isFocused(),
    };
    console.log(JSON.stringify(state));
    if (exit.code !== 0 || !state.visible || !state.restored || !state.focused) {
      throw new Error("Second instance exit or first window restore/focus was incorrect");
    }
    console.log("PASS second-instance-exits");
    console.log("PASS first-window-restored-focused");
  }).catch((error) => {
    console.error(error);
    process.exitCode = 1;
  }).finally(async () => {
    clearTimeout(watchdog);
    await stopSecond();
    app.exit(process.exitCode ?? 0);
  });
}
