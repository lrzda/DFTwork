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
        /// 结构文件路径。
        file: String,
    },

    /// 探测 Hermes 是否可用（缺席是正常状态）。
    HermesProbe,

    /// 打印版本与构建信息。
    Version,
}
