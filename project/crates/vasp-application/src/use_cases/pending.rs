//! 其余用例的骨架。
//!
//! 每个都返回 `NotImplemented` 而不是空结果——**空结果会让上层误以为
//! 「查了，什么都没有」**，而错误能一眼看出「还没做」。
//!
//! 实现顺序见 docs/roadmap.md 的 M1 / R3。

use crate::dto::{FileNode, TaskSummary, TextPreview};
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{BuildPlan, BuildReceipt, InputCheckReport};

fn todo<T>(what: &str) -> AppResult<T> {
    Err(AppError::NotImplemented(what.to_string()))
}

/// 打开项目：校验路径 + 记住最近项目。
pub struct OpenProject;
impl OpenProject {
    pub fn execute(_root: &Path) -> AppResult<()> {
        todo("OpenProject（需要 ProjectStore 端口）")
    }
}

/// 查看若干任务的详情（状态 + 证据 + 文件清单）。
pub struct InspectTasks;
impl InspectTasks {
    pub fn execute(_rels: &[String]) -> AppResult<Vec<TaskSummary>> {
        todo("InspectTasks")
    }
}

/// 列出一个任务目录的文件树。
pub struct ListTaskFiles;
impl ListTaskFiles {
    pub fn execute(_task_dir: &Path) -> AppResult<Vec<FileNode>> {
        todo("ListTaskFiles")
    }
}

/// 读文件预览（500 KB 截断）。
pub struct ReadTaskFile;
impl ReadTaskFile {
    pub fn execute(_file: &Path) -> AppResult<TextPreview> {
        todo("ReadTaskFile")
    }
}

/// 批量校验输入文件的一致性。
pub struct CheckInputs;
impl CheckInputs {
    pub fn execute(_rels: &[String], _profile: &str) -> AppResult<Vec<InputCheckReport>> {
        todo("CheckInputs（依赖 domain::validate_inputs）")
    }
}

/// 构建前发现可用的源结构与模板。
pub struct DiscoverInputs;
impl DiscoverInputs {
    pub fn execute(_root: &Path, _max_depth: usize) -> AppResult<Vec<String>> {
        todo("DiscoverInputs")
    }
}

/// 扫描候选模板。
pub struct ScanTemplates;
impl ScanTemplates {
    pub fn execute(_root: &Path, _max_depth: usize) -> AppResult<Vec<String>> {
        todo("ScanTemplates")
    }
}

/// 产出一份构建计划（**不落盘**）。
pub struct ProposeBuild;
impl ProposeBuild {
    pub fn execute(_targets: &[String]) -> AppResult<BuildPlan> {
        todo("ProposeBuild（依赖 domain::plan_build）")
    }
}

/// 放行一份构建计划（校验 planHash 后落盘）。
pub struct ApproveBuild;
impl ApproveBuild {
    pub fn execute(_plan_hash: &str) -> AppResult<BuildReceipt> {
        todo("ApproveBuild（需要 FileWriter 端口）")
    }
}

/// 续接上一次分析。
pub struct ContinueAnalysis;
impl ContinueAnalysis {
    pub fn execute(_session_id: &str, _message: &str) -> AppResult<()> {
        todo("ContinueAnalysis")
    }
}
