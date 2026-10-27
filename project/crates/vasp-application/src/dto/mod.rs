use serde::Serialize;
use vasp_domain::{InputPresence, OutputPresence, StatusEvidence, TaskStatus};

/// 扫描出来的一个任务。**这是给界面看的形状**，不是领域实体。
///
/// 状态文案（`status_label`）由后端给，前端不写 if-else 判断状态。
#[derive(Debug, Clone, Serialize)]
pub struct TaskSummary {
    pub rel_path: String,
    /// 机器可读码，见 [`TaskStatus::code`]。
    pub status: &'static str,
    pub status_label: &'static str,
    pub reason: String,
    pub evidence: Vec<StatusEvidence>,
    pub inputs: InputPresence,
    pub inputs_complete: bool,
    pub missing_inputs: Vec<&'static str>,
    pub outputs: OutputPresence,
    pub observed_at: Option<String>,
}

impl TaskSummary {
    pub fn from_parts(
        rel_path: String,
        status: TaskStatus,
        reason: String,
        evidence: Vec<StatusEvidence>,
        inputs: InputPresence,
        outputs: OutputPresence,
        observed_at: Option<String>,
    ) -> Self {
        Self {
            rel_path,
            status: status.code(),
            status_label: status.label_zh(),
            reason,
            evidence,
            inputs,
            inputs_complete: inputs.complete(),
            missing_inputs: inputs.missing(),
            outputs,
            observed_at,
        }
    }
}

/// 一次扫描的完整结果。
#[derive(Debug, Clone, Serialize)]
pub struct ScanReport {
    pub root: String,
    /// 遍历过的目录数（含非任务目录），用于告诉用户「确实扫了这么多」。
    pub dirs_scanned: usize,
    pub tasks: Vec<TaskSummary>,
    /// 只有输入、没有输出的目录——首版单独列出，不混进 `tasks`。
    pub pending_dirs: Vec<String>,
    pub elapsed_ms: u128,
    pub schema_version: u32,
}

impl ScanReport {
    pub const SCHEMA_VERSION: u32 = 1;
}

/// 文件预览。
#[derive(Debug, Clone, Serialize)]
pub struct TextPreview {
    pub rel_path: String,
    pub content: String,
    pub truncated: bool,
    pub bytes_read: u64,
}

/// 目录树节点。
#[derive(Debug, Clone, Serialize)]
pub struct FileNode {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}
