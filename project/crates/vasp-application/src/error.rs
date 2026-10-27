use vasp_domain::DomainError;

/// 对外错误码。界面、CLI、AI 工具看到的是同一套。
///
/// 系统错误（`ENOENT`、`PermissionDenied`…）在这里统一翻译，
/// 不让它们泄漏到上层去各自处理。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("[VALIDATION] {0}")]
    Validation(String),

    #[error("[NOT_FOUND] {0}")]
    NotFound(String),

    #[error("[CONFLICT] {0}")]
    Conflict(String),

    #[error("[EXTERNAL] {0}")]
    External(String),

    #[error("[TIMEOUT] {0}")]
    Timeout(String),

    #[error("[IO] {path}: {detail}")]
    Io { path: String, detail: String },

    #[error("[NOT_IMPLEMENTED] {0}")]
    NotImplemented(String),
}

impl AppError {
    /// 稳定的机器可读码，前端据此决定提示方式。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "VALIDATION",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::External(_) => "EXTERNAL",
            Self::Timeout(_) => "TIMEOUT",
            Self::Io { .. } => "IO",
            Self::NotImplemented(_) => "NOT_IMPLEMENTED",
        }
    }

    pub fn io(path: impl Into<String>, detail: impl std::fmt::Display) -> Self {
        Self::Io {
            path: path.into(),
            detail: detail.to_string(),
        }
    }

    /// 不带 `[CODE]` 前缀的原始描述，用于已经单独给出 `code` 的场合（CLI/JSON）。
    pub fn message(&self) -> String {
        match self {
            Self::Validation(m)
            | Self::NotFound(m)
            | Self::Conflict(m)
            | Self::External(m)
            | Self::Timeout(m)
            | Self::NotImplemented(m) => m.clone(),
            Self::Io { path, detail } => format!("{path}: {detail}"),
        }
    }
}

/// 领域错误 → 应用错误。
///
/// 「尚未实现」单独映射，避免与「真的没有这个文件」混在一起。
impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::NotImplemented(what) => Self::NotImplemented(what.to_string()),
            DomainError::PathOutsideProject(p) | DomainError::InvalidPath(p) => {
                Self::Validation(format!("路径不合法：{p}"))
            }
            DomainError::PlanHashMismatch => {
                Self::Conflict("计划已被修改，请重新确认后再放行".into())
            }
            other => Self::Validation(other.to_string()),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
