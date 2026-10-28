use serde::Serialize;

/// 一个 SCF（电子步）迭代。
///
/// 来源是 OSZICAR 里以 `DAV:` / `RMM:` 开头的行：
/// ```text
/// DAV:   1     -0.123456789012E+03    0.47236E+04   -0.19626E+05  1328   0.126E+03
/// DAV: 499    -0.442682027688E+03    0.41684E-04   -0.45625E-05  1544   0.110E-02    0.620E-02
/// ```
/// 最后那个字段（`rms(c)`）只在部分行出现，所以是 `Option`。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScfStep {
    /// 电子步序号，从 1 开始（每个离子步内部重新计数）。
    pub index: u32,
    /// 迭代算法：`DAV`（分块 Davidson）或 `RMM`（残差最小化）。
    pub algorithm: String,
    /// 当前总能量（eV）。
    pub energy_ev: f64,
    /// 与上一步的能量差（eV）。
    pub delta_e_ev: Option<f64>,
    /// 本征值变化量（eV）。
    pub delta_eps_ev: Option<f64>,
    /// 调用 Davidson/RMM 的子空间矩阵对角化次数。
    pub ncg: Option<u32>,
    /// 电荷密度残差（RMS）。
    pub rms: Option<f64>,
    /// 带预条件的残差。只有部分行有。
    pub rms_c: Option<f64>,
}

/// 一个离子步内部的完整 SCF 序列。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScfBlock {
    /// 属于第几个离子步（与 OSZICAR 的 `F=` 行编号一致）。
    pub ionic_index: u32,
    pub steps: Vec<ScfStep>,
}

/// 一份 OSZICAR 里的全部 SCF 序列。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScfSeries {
    pub blocks: Vec<ScfBlock>,
    /// 是否因为数据过多只保留了尾部。为 `true` 时界面要提示「只显示最近 N 步」。
    pub truncated: bool,
    /// 总共解析到多少个离子步（含被丢弃的）。
    pub total_blocks: usize,
}

impl ScfSeries {
    /// 最后一个离子步的 SCF 序列——「这次计算收敛得好不好」问的就是它。
    pub fn last_block(&self) -> Option<&ScfBlock> {
        self.blocks.last()
    }
}
