use clap::{Parser, Subcommand};

/// DFTwork 内核。所有子命令都往 stdout 输出 JSON，日志一律走 stderr。
#[derive(Debug, Parser)]
#[command(name = "vaspcore", version, about = "DFTwork 内核：VASP 任务扫描、解析与构建")]
pub struct Cli {
    /// 打开调试日志（写 stderr）。
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 扫描项目根，输出所有 VASP 任务与状态。
    Scan {
        /// 项目根目录。
        root: String,
        /// 最大下探深度，默认 8。
        #[arg(long, default_value_t = 8)]
        max_depth: usize,
    },

    /// 读一个任务的收敛序列（离子步能量 + 最大力）。
    Convergence {
        /// 任务目录。
        task: String,
    },

    /// 把 POSCAR / CONTCAR 读成 3D 场景。
    Scene {
        /// 结构文件路径，或任务目录（自动挑 POSCAR / CONTCAR）。
        file: String,
        /// 成键不考虑周期边界：只按真实坐标判断，用于孤立分子 / 团簇。
        /// 不加这个参数时按周期边界（最小镜像）算——晶体场景的正确做法。
        #[arg(long)]
        no_pbc: bool,
    },

    /// 提取磁矩（每个离子步的总磁矩）。
    Magnetization {
        /// 任务目录，或直接给 OUTCAR 路径。
        task: String,
    },

    /// SCF 电子步收敛序列（每个离子步内部的迭代）。
    Scf {
        /// 任务目录，或直接给 OSZICAR 路径。
        task: String,
    },

    /// 每原子力矢量与晶胞应力。
    Forces {
        /// 任务目录，或直接给 OUTCAR 路径。
        task: String,
    },

    /// 振动频率与力常数矩阵。
    Vibrations {
        /// 任务目录，或直接给 OUTCAR 路径。
        task: String,
    },

    /// 对比多个任务：INCAR 差异 + 收敛曲线。
    Compare {
        /// 至少两个任务目录。
        #[arg(required = true, num_args = 2..)]
        tasks: Vec<String>,
    },

    /// 以某个任务为模板生成一份输入计划（只产出计划，不落盘）。
    Build {
        /// 源任务目录（绝对路径，或相对 --root）。
        source: String,
        /// 目标任务目录（相对 --root）。
        target: String,
        /// INCAR 参数覆盖，可重复：--set ENCUT=520 --set ISMEAR=0
        #[arg(long = "set", value_name = "KEY=VALUE")]
        set: Vec<String>,
        /// 项目根；缺省取源任务目录的父目录。
        #[arg(long)]
        root: Option<String>,
        /// 只复制这些文件（默认 INCAR/POSCAR/KPOINTS，不含 POTCAR）。
        #[arg(long = "file", value_name = "NAME")]
        file: Vec<String>,
    },

    /// 执行一份已放行的构建计划（写入前自动备份覆盖项）。
    Apply {
        /// 计划 JSON 文件路径。
        plan: String,
    },

    /// 从回执回滚一次写入。
    Rollback {
        /// 回执 JSON 文件路径。
        receipt: String,
    },

    /// 让 AI 分析一个任务（需要本机装有 Hermes；没有也能用其他功能）。
    Analyze {
        /// 任务目录。
        task: String,
        /// 模型；`--ignore-user-config` 下必须显式给。
        #[arg(long)]
        model: Option<String>,
        /// 提供方（deepseek / openrouter / anthropic …）。
        #[arg(long)]
        provider: Option<String>,
        /// hermes 可执行文件路径；缺省用 PATH 里的 hermes。
        #[arg(long)]
        hermes_bin: Option<String>,
        #[arg(long, default_value_t = 2)]
        max_turns: u32,
        /// 超时秒数（超时由我们 kill 子进程，CLI 的 --run-budget 不支持）。
        #[arg(long, default_value_t = 180)]
        timeout: u64,
    },

    /// 探测 Hermes 是否可用（缺席是正常状态）。
    HermesProbe,

    /// 打印版本与构建信息。
    Version,
}
