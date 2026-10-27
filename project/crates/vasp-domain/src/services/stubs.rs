//! 尚未实现的领域能力。**返回显式错误，不返回空结果**——
//! 空结果会让上层误以为「查了，什么都没有」，而错误能一眼看出「还没做」。
//!
//! 每一项都对应 docs/rust-rewrite.md §11 里的一条判据，实现前先核对原实现。

use crate::{BuildPlan, DomainError, DomainResult};

/// 对应 `reference/VASPFlow/input-checker.js`（365 行）。
pub fn validate_inputs(_profile: &str, _files: &[(&str, String)]) -> DomainResult<()> {
    Err(DomainError::NotImplemented("validate_inputs"))
}

/// 对应 `reference/VASPFlow/input-builder.js` 的 `buildOneTask`（195 行 / 15 次 IO）。
/// 实现时必须拆成「纯计划生成」与「写盘」两段，本函数只产计划。
pub fn plan_build(_sources: &[String], _options: &str) -> DomainResult<BuildPlan> {
    Err(DomainError::NotImplemented("plan_build"))
}

/// 对应 `lib/index.js` 里 `buildAnalysisBundle` 的那段（≤12 KB 结构化事实）。
pub fn build_analysis_bundle(_task: &crate::Task) -> DomainResult<String> {
    Err(DomainError::NotImplemented("build_analysis_bundle"))
}

/// SCF 电子步序列。**首版留空实现**（ADR-006），接口先占位。
pub fn scf_series(_task_dir: &str) -> DomainResult<Vec<f64>> {
    Err(DomainError::NotImplemented("scf_series"))
}

/// XDATCAR 轨迹帧索引。**首版留空实现**（ADR-006）。
pub fn index_trajectory(_path: &str) -> DomainResult<Vec<u64>> {
    Err(DomainError::NotImplemented("index_trajectory"))
}
