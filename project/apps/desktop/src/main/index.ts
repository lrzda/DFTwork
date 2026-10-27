import { BrowserWindow, app, shell } from "electron";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { registerIpc } from "./ipc";
import { coreBinary } from "./core";

function createWindow(): void {
  const win = new BrowserWindow({
    width: 1320,
    height: 860,
    minWidth: 980,
    minHeight: 640,
    show: false,
    autoHideMenuBar: true,
    backgroundColor: "#0b1220",
    title: "DFTwork",
    webPreferences: {
      preload: path.join(__dirname, "../preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  win.on("ready-to-show", () => win.show());

  // 渲染进程挂了要能看见原因，否则只有一句 "Renderer process crashed"
  win.webContents.on("render-process-gone", (_e, details) => {
    console.error(
      `[dftwork] renderer gone: reason=${details.reason} exitCode=${details.exitCode}`,
    );
  });
  win.webContents.on("did-fail-load", (_e, code, desc, url) => {
    console.error(`[dftwork] load failed: ${code} ${desc} ${url}`);
  });
  win.webContents.on("preload-error", (_e, preloadPath, error) => {
    console.error(`[dftwork] preload error: ${preloadPath} -> ${error.message}`);
  });
  win.webContents.on("console-message", (_e, level, message) => {
    if (level >= 2) console.error(`[renderer] ${message}`);
  });

  // 外链一律走系统浏览器，不在应用内开窗
  win.webContents.setWindowOpenHandler(({ url }) => {
    void shell.openExternal(url);
    return { action: "deny" };
  });

  const devUrl = process.env.ELECTRON_RENDERER_URL;
  if (devUrl) {
    void win.loadURL(devUrl);
  } else {
    const query = process.env.DFTWORK_SMOKE_ROOT
      ? { project: process.env.DFTWORK_SMOKE_ROOT }
      : undefined;
    void win.loadFile(path.join(__dirname, "../renderer/index.html"), query ? { query } : undefined);
  }

  // 冒烟模式：加载完成后截图并退出，用来在无人值守时证明「界面真的渲染出来了」。
  // 跑法：DFTWORK_SMOKE=1 electron out/main/index.js
  if (process.env.DFTWORK_SMOKE) {
    win.webContents.once("did-finish-load", () => {
      setTimeout(() => {
        void win.webContents.capturePage().then((img) => {
          const out = path.join(
            process.env.DFTWORK_SMOKE_DIR ?? app.getPath("temp"),
            "dftwork-smoke.png",
          );
          writeFileSync(out, img.toPNG());
          console.info(`[dftwork] smoke screenshot = ${out}`);
          app.quit();
        });
      }, 4000); // 留时间给首屏数据（scan + scene 两次 IPC）与 three.js 初始化
    });
  }
}

app.whenReady().then(() => {
  // 启动时打一行内核路径，排查「找不到 exe」时第一时间能看见
  console.info(`[dftwork] core = ${coreBinary()}`);
  registerIpc();
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
