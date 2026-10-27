use serde::Serialize;
use std::path::PathBuf;

/// 一个 VASP 计算目录。
///
/// 判定标准（首版，须与 `reference/VASPFlow/scanner.js` 对拍确认）：
/// 目录里存在 `OUTCAR` 或 `vasprun.xml` 才算「任务」；只有输入文件的目录
/// 记为「未开始」，由上层决定是否展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Task {
    /// 相对项目根的路径，统一用 `/` 分隔，作为稳定标识。
    pub rel_path: String,
    /// 绝对路径。只在同一个进程内使用，不落盘、不进 DTO。
    #[serde(skip)]
    pub abs_path: PathBuf,
    /// 目录里出现过的关键文件名（原始大小写，便于排查）。
    pub files: Vec<String>,
    pub inputs: InputPresence,
    pub outputs: OutputPresence,
}

/// 输入文件的齐全情况。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct InputPresence {
    pub poscar: bool,
    pub incar: bool,
    pub kpoints: bool,
    pub potcar: bool,
}

impl InputPresence {
    /// 四个文件都在才算完整。缺哪个由调用方逐项展示，不做「部分完整」这种模糊判断。
    pub fn complete(&self) -> bool {
        self.poscar && self.incar && self.kpoints && self.potcar
    }

    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.poscar {
            out.push("POSCAR");
        }
        if !self.incar {
            out.push("INCAR");
        }
        if !self.kpoints {
            out.push("KPOINTS");
        }
        if !self.potcar {
            out.push("POTCAR");
        }
        out
    }
}

/// 输出文件的出现情况。它只说明「有没有」，不说明「算完没」——
/// 后者由 [`crate::services::classify`] 依据文件内容判断。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct OutputPresence {
    pub outcar: bool,
    pub oszicar: bool,
    pub vasprun: bool,
    pub contcar: bool,
    pub xdatcar: bool,
}

impl OutputPresence {
    /// 有任何一条输出证据，说明这个目录真的跑过。
    pub fn any(&self) -> bool {
        self.outcar || self.oszicar || self.vasprun
    }
}

/// 计算类型。由 INCAR 的 `IBRION` / `NSW` / `IMAGES` 推断，
/// **不读输出文件**——所以「算没算」和「算什么」是两件独立的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskKind {
    /// `IBRION=1/2/3` 且 `NSW>0`：结构优化
    Relaxation,
    /// `IBRION=5/6/7/8`：有限差分或 DFPT 频率
    Frequency,
    /// `IBRION=0` 且 `NSW>0`：从头算分子动力学
    Aimd,
    /// 其余（`IBRION=-1` 或 `NSW=0`）：静态单点 / SCF
    StaticScf,
    /// `IMAGES>0`：NEB / CI-NEB
    Neb,
    /// INCAR 缺失或读不出来
    Unknown,
}

impl TaskKind {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Relaxation => "RELAXATION",
            Self::Frequency => "FREQUENCY",
            Self::Aimd => "AIMD",
            Self::StaticScf => "STATIC_SCF",
            Self::Neb => "NEB",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// 界面上直接显示的中文名。与 `reference/VASPFlow` 的 label 保持一致，
    /// 免得同一批数据在两个工具里叫不同名字。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Relaxation => "结构优化",
            Self::Frequency => "频率 / ZPE",
            Self::Aimd => "AIMD",
            Self::StaticScf => "静态 / SCF",
            Self::Neb => "NEB",
            Self::Unknown => "未知",
        }
    }
}
