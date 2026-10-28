use serde::Serialize;

/// 单个原子的受力。
///
/// 来源是 OUTCAR 的 `POSITION  TOTAL-FORCE (eV/Angst)` 数据段：
/// ```text
///      4.65438      6.00944      3.38929        -1.912784     -0.142551     -1.840043
/// ```
/// 前 3 列是坐标（Å），后 3 列是力（eV/Å）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AtomForce {
    /// 原子序号，从 0 开始（与 POSCAR 里的顺序一致）。
    pub index: usize,
    /// 笛卡尔坐标（Å）。
    pub position: [f64; 3],
    /// 力矢量（eV/Å）。
    pub force: [f64; 3],
    /// 力的大小（eV/Å）。VASP 的 `max atom` 就是这个值取最大。
    pub magnitude_ev_per_a: f64,
}

/// 某一个离子步结束时全部原子的受力。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForceSet {
    /// 属于第几个离子步（从 1 开始）。
    pub ionic_index: u32,
    pub atoms: Vec<AtomForce>,
    /// 该步的最大受力（eV/Å）。收敛判据 `EDIFFG` 比的就是它。
    pub max_force_ev_per_a: f64,
}

/// 受力解析结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForceReport {
    /// 按离子步倒序保留最近几组（最后一步在最前面，最常看）。
    pub sets: Vec<ForceSet>,
    /// 是否因为数据过多只保留了尾部。
    pub truncated: bool,
    /// OUTCAR 里一共出现过多少组力数据。
    pub total_sets: usize,
}

impl ForceReport {
    /// 最后一步的受力——「现在这个结构受力多大」问的就是它。
    pub fn latest(&self) -> Option<&ForceSet> {
        self.sets.first()
    }
}

/// 应力的 6 个独立分量。
///
/// 来源是 OUTCAR 的 `in kB` 行：
/// ```text
/// in kB      36.83850     9.46328     9.83154    -1.06092     2.16883    12.17631
/// ```
/// 顺序是 XX YY ZZ XY YZ ZX（VASP 固定顺序，注意不是 XY XZ YZ）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StressSet {
    pub ionic_index: u32,
    /// XX YY ZZ XY YZ ZX，单位 kB（1 kB = 0.1 GPa）。
    pub components_kb: [f64; 6],
    /// 各向同性分压 = (XX+YY+ZZ)/3，单位 kB。正值为压应力。
    pub pressure_kb: f64,
}

/// 应力解析结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StressReport {
    /// 最近几个离子步的应力，最后一步在最前面。
    pub sets: Vec<StressSet>,
    pub truncated: bool,
    pub total_sets: usize,
}
