use serde::Serialize;

/// 一个振动模式。
///
/// 来源是 OUTCAR 的 `Eigenvectors and eigenvalues of the dynamical matrix` 段：
/// ```text
///    1 f  =   十几 THz    87.765876 2PiTHz  465.934755 cm-1    57.768568 meV
///   56 f/i=    0.044329 THz     0.278526 2PiTHz    1.478647 cm-1     0.183329 meV
/// ```
/// `f` 是实频，`f/i` 是**虚频**（结构在该模式上不稳定）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VibrationalMode {
    /// 模式序号，从 1 开始。总共应有 3N 个（N = 原子数）。
    pub index: u32,
    /// 频率（THz）。**虚频取负值**，这样排序与绘图天然把虚频放在 0 以下。
    pub frequency_thz: f64,
    /// 是否是虚频。
    pub is_imaginary: bool,
    /// 频率（cm⁻¹），虚频同样取负。
    pub frequency_cm1: f64,
    /// 能量（meV）。
    pub energy_mev: f64,
}

/// 力常数（Hessian）矩阵。
///
/// 来源是 OUTCAR 的 `SECOND DERIVATIVES (NOT SYMMETRIZED)` 段。
/// 维度是 `3N × 3N`（N = 原子数），例如 24 个原子 → 72×72。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HessianMatrix {
    /// 自由度数目（= 3 × 原子数）。
    pub n_dof: usize,
    /// 行主序的完整矩阵，长度 `n_dof × n_dof`，单位 eV/Å²。
    ///
    /// 72×72 是 5184 个数，序列化后约 60 KB——可以直接传给界面画热图。
    pub data: Vec<f64>,
}

/// 振动分析结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VibrationReport {
    /// 全部振动模式，**保持 OUTCAR 的原始顺序**。
    ///
    /// VASP 的排序不是单调的：实频从高到低（例如 13.97 → 0.587 THz），
    /// 虚频紧接着从低到高（0.044 → 十几 THz）。所以「第一个元素」既不是
    /// 最低频也不是最高频——要最低频请用 [`VibrationReport::lowest_thz`]。
    pub modes: Vec<VibrationalMode>,
    /// 虚频个数。非零说明结构在简谐近似下不稳定——界面必须显著标出。
    pub imaginary_count: usize,
    /// 力常数矩阵。没做有限差分（IBRION 不是 5/6）时为 `None`。
    pub hessian: Option<HessianMatrix>,
    /// 一句话说明数据来源或缺失原因。
    pub note: String,
}

impl VibrationReport {
    /// 最低频率（虚频取负值后，最负的那个）。
    ///
    /// 不能取 `modes.first()`：VASP 的输出顺序是「实频降序 + 虚频升序」，
    /// 首元素往往是最大的实频。
    pub fn lowest_thz(&self) -> Option<f64> {
        self.modes
            .iter()
            .map(|m| m.frequency_thz)
            .fold(None, |acc: Option<f64>, x| Some(acc.map_or(x, |a| a.min(x))))
    }
}
