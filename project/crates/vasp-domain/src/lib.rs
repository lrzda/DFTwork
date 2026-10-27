//! VASP 领域层：解析、判定、规则、计划生成。
//!
//! 硬约束（由 Cargo.toml 保证，不靠自觉）：
//! - 不读文件、不 spawn 进程、不联网、不碰环境变量。
//! - 所有函数的入参是**文本或纯数据**；凡是要读盘的，一律留在 application。
//! - 不抛系统错误（`ENOENT` 之类），只抛 [`DomainError`]。

pub mod model;
pub mod policies;
pub mod ports;
pub mod services;

pub use model::*;
pub use policies::*;
pub use ports::*;
pub use services::*;

/// 领域错误。基础设施层负责把系统错误翻译成这些。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    #[error("目标路径不在项目根内：{0}")]
    PathOutsideProject(String),

    #[error("路径含非法片段：{0}")]
    InvalidPath(String),

    #[error("结构文件无法解析：{0}")]
    InvalidStructure(String),

    #[error("POTCAR 的元素顺序与 POSCAR 不一致")]
    PotcarOrderMismatch,

    #[error("INCAR 格式错误：{0}")]
    InvalidIncar(String),

    #[error("构建计划已被修改（planHash 不匹配），需要重新放行")]
    PlanHashMismatch,

    #[error("尚未实现：{0}")]
    NotImplemented(&'static str),
}

pub type DomainResult<T> = Result<T, DomainError>;
