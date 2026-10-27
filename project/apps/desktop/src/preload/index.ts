import { contextBridge, ipcRenderer } from "electron";
import { IPC, type DftworkApi } from "@dftwork/shared";

/**
 * 只做转发，不做业务判断。
 * 返回值是信封（`{ok,data|error}`），解包与抛错在渲染进程的 `api.ts` 里做——
 * 跨 contextBridge 抛出的 Error 会丢原型和自定义字段。
 */
const api: DftworkApi = {
  version: () => ipcRenderer.invoke(IPC.coreVersion),
  hermesProbe: () => ipcRenderer.invoke(IPC.hermesProbe),
  scan: (req) => ipcRenderer.invoke(IPC.scan, req),
  convergence: (req) => ipcRenderer.invoke(IPC.convergence, req),
  scene: (req) => ipcRenderer.invoke(IPC.scene, req),
};

contextBridge.exposeInMainWorld("dftwork", api);
