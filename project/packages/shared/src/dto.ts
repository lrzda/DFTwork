/**
 * 与 Rust 端 DTO 一一对应的类型。
 *
 * 改这里必须同步改 `crates/vasp-application/src/dto/mod.rs`，
 * 否则 `pnpm typecheck` 过、运行时字段却是 undefined。
 * 字段只增不改；未知字段前端一律忽略（`schema_version` 守门）。
 */

export const SCHEMA_VERSION = 1 as const;

export type TaskStatusCode =
  | "NO_OUTPUT_EVIDENCE"
  | "TASK_COMPLETED"
  | "ERROR_DETECTED"
  | "RUNNING"
  | "UNKNOWN";

export interface StatusEvidence {
  file: string;
  line: number | null;
  excerpt: string;
}

export interface InputPresence {
  poscar: boolean;
  incar: boolean;
  kpoints: boolean;
  potcar: boolean;
}

export interface OutputPresence {
  outcar: boolean;
  oszicar: boolean;
  vasprun: boolean;
  contcar: boolean;
  xdatcar: boolean;
}

export interface TaskSummary {
  rel_path: string;
  status: TaskStatusCode;
  /** 中文状态文案。由后端给，前端不要自己判断状态拼文案。 */
  status_label: string;
  reason: string;
  evidence: StatusEvidence[];
  inputs: InputPresence;
  inputs_complete: boolean;
  missing_inputs: string[];
  outputs: OutputPresence;
  observed_at: string | null;
}

export interface ScanReport {
  root: string;
  dirs_scanned: number;
  tasks: TaskSummary[];
  /** 只有输入、没有输出的目录，首版单独列出。 */
  pending_dirs: string[];
  elapsed_ms: number;
  schema_version: number;
}

export interface IonicStep {
  index: number;
  energy_ev: number | null;
  max_force_ev_per_a: number | null;
}

export interface ConvergenceSeries {
  steps: IonicStep[];
  reached_required_accuracy: boolean | null;
  source: string;
  truncated: boolean;
}

export type Vec3 = [number, number, number];

export interface Atom {
  element: string;
  /** 笛卡尔坐标（Å）。 */
  position: Vec3;
  species_index: number;
}

export interface Bond {
  a: number;
  b: number;
}

export interface StructureScene {
  cell: [Vec3, Vec3, Vec3];
  atoms: Atom[];
  /**
   * 化学键。首版恒为空数组——成键算法待与 reference 对拍，
   * 渲染器必须能降级为「只画原子」。
   */
  bonds: Bond[];
  comment: string;
  selective_dynamics: boolean;
  scale: number;
  /** 数据来自哪个文件（POSCAR / CONTCAR），由内核填入。 */
  source_file: string | null;
}

export interface HermesCapability {
  found: boolean;
  bin: string | null;
  version: string | null;
  supports_stream_json: boolean;
  reason: string | null;
}

export interface BuildItem {
  rel_path: string;
  content: string;
  action: "create" | "overwrite" | "skip";
  reason: string;
}

export interface BuildPlan {
  root: string;
  items: BuildItem[];
  /** 放行凭据：内容一变就失效。 */
  plan_hash: string;
  created_at: string;
}

export interface BuildReceipt {
  plan_hash: string;
  written: string[];
  skipped: string[];
  backup_dir: string | null;
  finished_at: string;
}

export interface VersionInfo {
  name: string;
  version: string;
  schema_version: number;
}
