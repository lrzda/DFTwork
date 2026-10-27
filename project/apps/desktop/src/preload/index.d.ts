import type { DftworkApi } from "@dftwork/shared";

declare global {
  interface Window {
    dftwork: DftworkApi;
  }
}

export {};
