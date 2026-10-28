use serde::Serialize;

use crate::ConvergenceSeries;

/// 一个 INCAR 参数在各任务里的取值。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputDiff {
    pub key: String,
    /// 与 [`CompareReport::tasks`] 一一对应。`None` 表示这个任务没写这项。
    pub values: Vec<Option<String>>,
}

/// 多任务对比结果。
///
/// 只列**不完全相同**的参数：几十项全列出来，真正要看的那两三行会被淹掉。
/// 相同项的数量单独给出，让用户知道「一共有多少项是一致的」。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompareReport {
    /// 参与对比的任务（相对项目根的路径，顺序与各字段的数组一致）。
    pub tasks: Vec<String>,
    /// 取值不完全相同的 INCAR 参数，按字母序。
    pub input_diff: Vec<InputDiff>,
    /// 完全一致的参数个数。
    pub same_count: usize,
    /// 每个任务的离子步序列，顺序与 `tasks` 一致。
    pub series: Vec<ConvergenceSeries>,
}

impl CompareReport {
    /// 差异项数为 0 时说明输入完全相同——这本身是个有用的结论。
    pub fn identical_inputs(&self) -> bool {
        self.input_diff.is_empty()
    }
}
