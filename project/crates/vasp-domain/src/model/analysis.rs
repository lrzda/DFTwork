//! AI 分析的结果类型。
//!
//! 只描述「一次分析产出了什么」，不描述怎么调用 CLI——那是基础设施的事。

use serde::{Deserialize, Serialize};

/// token 用量。字段名与 Hermes 的 `result.tokens` 一致，便于直接对照。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub total: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

/// 一次工具调用（AI 若调了工具才有）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    /// 参数摘要，用于界面展示「它查了什么」。
    pub summary: String,
    pub is_error: bool,
}

/// 一次分析的结果。
///
/// `exit_code` 综合 Hermes 的 `result.exit_code` 与进程退出状态：模型失败可能仍
/// 输出完整 JSONL，进程失败也可能带有伪成功 result；二者都不能误判为成功。
/// 超时由独立的 `timed_out` 标记。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub session_id: String,
    /// Hermes 报告的结果码；进程失败或无 result 时为宿主记录的非零值。
    pub exit_code: i32,
    /// 模型的完整回答（已把 token 级片段拼接起来）。
    pub text: String,
    pub tokens: TokenUsage,
    pub duration_ms: u64,
    /// 收到的事件行数，用于诊断「到底读到了多少」。
    pub event_lines: usize,
    pub tool_calls: Vec<ToolCall>,
    /// 是否因为超时被我们 kill 掉的。
    pub timed_out: bool,
}

impl AnalysisReport {
    pub fn ok(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}
