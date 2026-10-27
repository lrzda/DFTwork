use serde::{Deserialize, Serialize};

use crate::StatusEvidence;

/// 输入一致性检查的结论。
///
/// 它回答「输入文件之间是否自相一致」，**不回答「能不能直接提交」**——
/// 后者涉及集群队列、机时、收敛判据等软件不掌握的信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InputCheckCode {
    /// 这一项本次没有要求检查。
    NotRequested,
    /// 要求检查了，但文件缺失或读不到。
    NotChecked,
    Pass,
    Warn,
    Fail,
}

impl InputCheckCode {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotRequested => "NOT_REQUESTED",
            Self::NotChecked => "NOT_CHECKED",
            Self::Pass => "PASS",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        }
    }

    pub fn worst(self, other: Self) -> Self {
        let rank = |c: Self| match c {
            Self::NotRequested => 0,
            Self::Pass => 1,
            Self::NotChecked => 2,
            Self::Warn => 3,
            Self::Fail => 4,
        };
        if rank(other) > rank(self) {
            other
        } else {
            self
        }
    }
}

/// 一条检查项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCheck {
    /// 检查项的名字，例如 `POTCAR_ORDER`、`INCAR_ENCUT`。
    pub rule: String,
    pub code: InputCheckCode,
    /// 一句话结论，直接给用户看。
    pub message: String,
    pub evidence: Vec<StatusEvidence>,
}

impl InputCheck {
    pub fn new(rule: impl Into<String>, code: InputCheckCode, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            code,
            message: message.into(),
            evidence: Vec::new(),
        }
    }
}

/// 一个任务的检查汇总。逐项保留，不折叠成一个总分——
/// 「3 项通过 1 项警告」比「总体：警告」有用得多。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputCheckReport {
    pub rel_path: String,
    pub overall: InputCheckCode,
    pub items: Vec<InputCheck>,
}

impl InputCheckReport {
    pub fn from_items(rel_path: impl Into<String>, items: Vec<InputCheck>) -> Self {
        let overall = items
            .iter()
            .map(|i| i.code)
            .fold(InputCheckCode::NotRequested, |a, b| a.worst(b));
        Self {
            rel_path: rel_path.into(),
            overall,
            items,
        }
    }
}
