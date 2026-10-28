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
  /** 计算类型码，由 INCAR 的 IBRION/NSW/IMAGES 推断。 */
  kind: TaskKindCode;
  kind_label: string;
  reason: string;
  evidence: StatusEvidence[];
  inputs: InputPresence;
  inputs_complete: boolean;
  missing_inputs: string[];
  outputs: OutputPresence;
  observed_at: string | null;
}

export type TaskKindCode =
  | "RELAXATION"
  | "STATIC_SCF"
  | "NEB"
  | "FREQUENCY"
  | "AIMD"
  | "UNKNOWN";

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

// ───────────────────────── 属性查询：SCF / 力 / 应力 / 振动 / 磁矩 ─────────────────────────

/** 一个 SCF（电子步）迭代。来自 OSZICAR 的 `DAV:` / `RMM:` 行。 */
export interface ScfStep {
  index: number;
  /** `DAV`（分块 Davidson）或 `RMM`（残差最小化）。 */
  algorithm: string;
  energy_ev: number;
  delta_e_ev: number | null;
  delta_eps_ev: number | null;
  ncg: number | null;
  rms: number | null;
  /** 带预条件的残差，只有部分行有。 */
  rms_c: number | null;
}

/** 一个离子步内部的 SCF 序列。 */
export interface ScfBlock {
  ionic_index: number;
  steps: ScfStep[];
}

export interface ScfSeries {
  /** 最多保留最近 20 个离子步。 */
  blocks: ScfBlock[];
  /** 为 true 表示只保留了尾部。 */
  truncated: boolean;
  total_blocks: number;
}

/** 单个原子的受力。 */
export interface AtomForce {
  index: number;
  position: Vec3;
  /** 力矢量（eV/Å）。 */
  force: Vec3;
  magnitude_ev_per_a: number;
}

export interface ForceSet {
  ionic_index: number;
  atoms: AtomForce[];
  max_force_ev_per_a: number;
}

export interface ForceReport {
  /** 按离子步倒序，最新的一步在最前面。 */
  sets: ForceSet[];
  truncated: boolean;
  total_sets: number;
}

export interface StressSet {
  ionic_index: number;
  /** XX YY ZZ XY YZ ZX，单位 kB（1 kB = 0.1 GPa）。 */
  components_kb: number[];
  /** (XX+YY+ZZ)/3，正值为压应力。 */
  pressure_kb: number;
}

export interface StressReport {
  sets: StressSet[];
  truncated: boolean;
  total_sets: number;
}

/** 一个振动模式。虚频的 `frequency_thz` 与 `frequency_cm1` 都是负值。 */
export interface VibrationalMode {
  index: number;
  frequency_thz: number;
  is_imaginary: boolean;
  frequency_cm1: number;
  energy_mev: number;
}

export interface HessianMatrix {
  /** 自由度数目（= 3 × 原子数）。 */
  n_dof: number;
  /** 行主序，长度 n_dof × n_dof，单位 eV/Å²。 */
  data: number[];
}

export interface VibrationReport {
  /** 保持 OUTCAR 原始顺序：实频降序 + 虚频升序。 */
  modes: VibrationalMode[];
  imaginary_count: number;
  hessian: HessianMatrix | null;
  note: string;
}

/** 一个离子步的总磁矩。 */
export interface MagneticStep {
  index: number;
  total_mu_b: number;
  interstitial_mu_b: number | null;
}

export interface MagnetizationReport {
  steps: MagneticStep[];
  /** 是否有逐原子磁矩表。目前恒为 false（见 note 里的原因）。 */
  per_atom_available: boolean;
  note: string;
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
