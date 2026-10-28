use serde::Serialize;

/// 一个离子步的磁矩。
///
/// 来源是 OUTCAR 里每个电子步都会刷新的两行：
/// ```text
///  number of electron     123.4567890 magnetization      12.3456789
///  augmentation part        9.4486309 magnetization      6.7890123
/// ```
/// 第一行是**整个原胞的总磁矩**（μB），第二行是**球间/增强区**的磁矩。
/// 两者之差近似为「原子球内」的磁矩总和——但没有逐原子分解时，
/// 这个差值只能作为一个整体参考，不能当作某个原子的磁矩。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MagneticStep {
    /// 离子步序号，从 1 开始（与 OSZICAR 的离子步编号一致）。
    pub index: u32,
    /// 总磁矩（μB）。
    pub total_mu_b: f64,
    /// 球间区磁矩（μB）。缺失时为 `None`。
    pub interstitial_mu_b: Option<f64>,
}

/// 磁矩解析结果。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MagnetizationReport {
    /// 逐离子步的总磁矩。空数组表示这份 OUTCAR 里没有磁矩输出
    /// （多半是 `ISPIN=1` 的非自旋极化计算）。
    pub steps: Vec<MagneticStep>,
    /// 是否找到了**逐原子**磁矩表（`magnetization (x)` 块）。
    ///
    /// 为 `false` 时界面必须明确告诉用户「只有总磁矩」，
    /// 不能让用户以为「所有原子的磁矩都是 0」。
    pub per_atom_available: bool,
    /// 一句话说明数据来源或缺失原因，直接给用户看。
    pub note: String,
}
