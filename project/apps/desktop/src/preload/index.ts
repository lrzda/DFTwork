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
  scf: (req) => ipcRenderer.invoke(IPC.scf, req),
  forces: (req) => ipcRenderer.invoke(IPC.forces, req),
  vibrations: (req) => ipcRenderer.invoke(IPC.vibrations, req),
  magnetization: (req) => ipcRenderer.invoke(IPC.magnetization, req),
  compare: (req) => ipcRenderer.invoke(IPC.compare, req),
  build: (req) => ipcRenderer.invoke(IPC.build, req),
  apply: (req) => ipcRenderer.invoke(IPC.apply, req),
  rollback: (req) => ipcRenderer.invoke(IPC.rollback, req),
  analyze: (req) => ipcRenderer.invoke(IPC.analyze, req),
  trajectory: (req) => ipcRenderer.invoke(IPC.trajectory, req),
};

contextBridge.exposeInMainWorld("dftwork", api);
