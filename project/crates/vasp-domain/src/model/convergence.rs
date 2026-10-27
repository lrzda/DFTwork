use serde::{Deserialize, Serialize};

/// 一个离子步。字段全部可缺省——不同 VASP 版本、不同参数下能拿到的量不一样，
/// 拿不到就留 `None`，**不要填 0 或猜测值**。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IonicStep {
    /// 离子步序号，从 1 开始（与 OSZICAR 的 `1 F=` 一致）。
    pub index: u32,
    /// 该步的 F 值（eV）。自由能还是总能，取决于 ISIF——文档里写清，代码不替它决定。
    pub energy_ev: Option<f64>,
    /// 该步的最大力（eV/Å）。来自 OUTCAR 的 `POSITION  TOTAL-FORCE` 段。
    pub max_force_ev_per_a: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvergenceSeries {
    pub steps: Vec<IonicStep>,
    /// 是否达到 EDIFF 判据。`None` 表示文件里没有可判定的信息。
    pub reached_required_accuracy: Option<bool>,
    /// 数据来源：`OSZICAR` / `OUTCAR` / `vasprun.xml`。
    pub source: String,
    /// 若只读到部分步（例如只解析了尾部），标记是否被截断。
    pub truncated: bool,
}

impl ConvergenceSeries {
    pub fn empty(source: impl Into<String>) -> Self {
        Self {
            steps: Vec::new(),
            reached_required_accuracy: None,
            source: source.into(),
            truncated: false,
        }
    }
}
