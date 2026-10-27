import { resolve } from "node:path";
import { defineConfig, externalizeDepsPlugin } from "electron-vite";
import react from "@vitejs/plugin-react";

/**
 * `@dftwork/shared` 不参与外部化：它直接以 TypeScript 源码分发，
 * Node 在运行时无法 require `.ts`，所以必须由 vite 打进 bundle。
 */
const bundleShared = () =>
  externalizeDepsPlugin({ exclude: ["@dftwork/shared"] });

export default defineConfig({
  main: {
    plugins: [bundleShared()],
    build: {
      rollupOptions: { input: { index: resolve(__dirname, "src/main/index.ts") } },
    },
  },
  preload: {
    plugins: [bundleShared()],
    build: {
      rollupOptions: { input: { index: resolve(__dirname, "src/preload/index.ts") } },
    },
  },
  renderer: {
    root: resolve(__dirname, "src/renderer"),
    resolve: {
      alias: {
        "@renderer": resolve(__dirname, "src/renderer/src"),
        "@dftwork/shared": resolve(__dirname, "../../packages/shared/src"),
      },
    },
    plugins: [react()],
    build: {
      rollupOptions: { input: { index: resolve(__dirname, "src/renderer/index.html") } },
    },
  },
});
