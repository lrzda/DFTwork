use serde::{Deserialize, Serialize};

/// 一条文件变更的处置方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WriteAction {
    /// 文件不存在，直接创建（不备份）。
    Create,
    /// 文件已存在，需要备份后覆盖（必须用户逐项确认）。
    Overwrite,
    /// 内容与现有文件完全一致，跳过。
    Skip,
}

/// 一条文件变更。「计划」就是这些条目的集合——它描述未来，不产生任何副作用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildItem {
    /// 相对项目根的目标路径。
    pub rel_path: String,
    /// 完整文件内容（不是 diff）。
    pub content: String,
    pub action: WriteAction,
    /// 为什么这样写（例如「由 task-001/CONTCAR 生成，ENCUT 取 400」）。
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildPlan {
    pub root: String,
    pub items: Vec<BuildItem>,
    /// 计划指纹。放行时校验它没变，计划内容一变即失效。
    pub plan_hash: String,
    pub created_at: String,
}

impl BuildPlan {
    /// 需要用户确认的条目（只有覆盖才需要）。
    pub fn overwrite_items(&self) -> impl Iterator<Item = &BuildItem> {
        self.items
            .iter()
            .filter(|i| i.action == WriteAction::Overwrite)
    }

    pub fn write_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.action != WriteAction::Skip)
            .count()
    }
}

/// 一次落盘的回执。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildReceipt {
    pub plan_hash: String,
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    /// 备份目录，相对项目根；没有覆盖项时为 `None`。
    pub backup_dir: Option<String>,
    pub finished_at: String,
}
