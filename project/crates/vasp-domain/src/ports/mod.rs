//! 6 个业务端口。**只有需要替换或需要 mock 的才端口化。**
//!
//! 文件读取与目录遍历不在这里——它们由 application 层自己声明所需的
//! `Fs` 能力（见 `vasp_application::ports::Fs`），因为「读 VASP 目录里的文件」
//! 没有第二种实现，抽象成领域端口只会多一层包装。

use crate::{BuildPlan, BuildReceipt};
use serde::Serialize;

/// 时间。领域层不读时钟，需要时间戳时由调用方注入。
pub trait Clock: Send + Sync {
    /// ISO 8601，带本地时区偏移。
    fn now_iso(&self) -> String;
}

/// 日志。测试时可换成静音实现。
pub trait Logger: Send + Sync {
    fn debug(&self, msg: &str);
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
}

/// 进度事件回推（扫描边扫边报、分析流式输出）。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &ProgressEvent);
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressEvent {
    Discovered { rel_path: String, is_task: bool },
    Scanned { done: usize, total_hint: Option<usize> },
    AnalysisText { delta: String },
    AnalysisTool { name: String, summary: String },
    AnalysisDone { session_id: Option<String> },
}

/// 项目级状态（最近项目、视图状态、会话映射）。
pub trait ProjectStore: Send + Sync {
    fn load(&self) -> Result<String, String>;
    fn save(&self, json: &str) -> Result<(), String>;
}

/// 唯一落盘点。
///
/// 只有放行之后的计划能到达这里；实现负责「备份 → 写临时文件 → rename」。
pub trait FileWriter: Send + Sync {
    fn apply(&self, plan: &BuildPlan) -> Result<BuildReceipt, String>;
}

/// Hermes 能力探测结果。缺席是正常状态，不是错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HermesCapability {
    pub found: bool,
    pub bin: Option<String>,
    pub version: Option<String>,
    /// 是否支持 `--format stream-json`（一期唯一依赖的能力）。
    pub supports_stream_json: bool,
    /// 探测失败的原因，`found == true` 时通常为 `None`。
    pub reason: Option<String>,
}

impl HermesCapability {
    pub fn absent(reason: impl Into<String>) -> Self {
        Self {
            found: false,
            bin: None,
            version: None,
            supports_stream_json: false,
            reason: Some(reason.into()),
        }
    }
}

/// 可选 AI 通道。缺席时所有调用方都必须照常工作。
pub trait Hermes: Send + Sync {
    fn probe(&self) -> HermesCapability;
}
