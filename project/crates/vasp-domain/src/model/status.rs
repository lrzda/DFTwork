use serde::{Deserialize, Serialize};

/// 任务状态。判定依据永远是文件内容，不是文件是否存在。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    /// 只有输入、没有任何输出证据。
    NoOutputEvidence,
    /// OUTCAR / vasprun.xml 里有正常结束标记。
    Completed,
    /// 输出里有明确报错。
    ErrorDetected,
    /// 有输出但没有结束标记，且有正在推进的迹象。
    Running,
    /// 有输出，但既不完整也不报错——无法判断。
    Unknown,
}

impl TaskStatus {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoOutputEvidence => "NO_OUTPUT_EVIDENCE",
            Self::Completed => "TASK_COMPLETED",
            Self::ErrorDetected => "ERROR_DETECTED",
            Self::Running => "RUNNING",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// 界面上直接用的中文文案。状态语义只在领域层定义一次，
    /// 前端不写 if-else 判断状态。
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::NoOutputEvidence => "未开始 / 无输出",
            Self::Completed => "已完成",
            Self::ErrorDetected => "有报错",
            Self::Running => "可能仍在运行",
            Self::Unknown => "无法判断",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::ErrorDetected)
    }
}

/// 状态必须带证据。只给结论不给依据的判定在评审时无法复核，
/// 因此这个结构不可为 `None`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEvidence {
    /// 证据来自哪个文件（相对任务目录的文件名）。
    pub file: String,
    /// 命中的行号（1 起，取不到时为 `None`）。
    pub line: Option<usize>,
    /// 命中的原文片段，便于核对。
    pub excerpt: String,
}

impl StatusEvidence {
    pub fn new(file: impl Into<String>, line: Option<usize>, excerpt: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            line,
            excerpt: excerpt.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatusRecord {
    pub status: TaskStatus,
    pub reason: String,
    pub evidence: Vec<StatusEvidence>,
    /// ISO 8601，由调用方注入（领域层不读时钟）。
    pub observed_at: Option<String>,
}

impl TaskStatusRecord {
    pub fn new(status: TaskStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
            evidence: Vec::new(),
            observed_at: None,
        }
    }

    pub fn with_evidence(mut self, evidence: Vec<StatusEvidence>) -> Self {
        self.evidence = evidence;
        self
    }

    pub fn at(mut self, observed_at: impl Into<String>) -> Self {
        self.observed_at = Some(observed_at.into());
        self
    }
}
