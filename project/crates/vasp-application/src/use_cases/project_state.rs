use super::build_store::{BuildState, BuildStore, RetentionReport};
use super::path_safety::{business_target_path, target_path};
use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use vasp_domain::{blob_name, blob_rel_path, content_hash, Clock};

pub struct GetProjectState<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetProjectState<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, root: &Path) -> AppResult<BuildState> {
        BuildStore::new(self.fs, root)?.load()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOutcome {
    Committed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoveryReport {
    pub plan_hash: String,
    pub outcome: RecoveryOutcome,
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub finished_at: String,
}

struct RecoveryEntry {
    rel_path: String,
    target: PathBuf,
    current_hash: Option<String>,
    previous: Option<String>,
    applied_hash: String,
}

pub struct RecoverInterrupted<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

impl<'a> RecoverInterrupted<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, root: &Path) -> AppResult<RecoveryReport> {
        let store = BuildStore::new(self.fs, root)?;
        let (journal, committed) = store.recovery()?.ok_or_else(|| {
            AppError::NotFound("当前项目没有需要恢复的未完成写入".into())
        })?;
        if committed {
            store.remove_journal(&journal.batch_dir)?;
            store.clear_pending(&journal.plan.plan_hash)?;
            store.enforce_retention(&self.clock.now_iso())?;
            return Ok(RecoveryReport {
                plan_hash: journal.plan.plan_hash,
                outcome: RecoveryOutcome::Committed,
                restored: Vec::new(),
                removed: Vec::new(),
                finished_at: self.clock.now_iso(),
            });
        }

        let mut prepared = Vec::new();
        for entry in &journal.receipt.restores {
            let target = business_target_path(self.fs, store.root(), &entry.rel_path)?;
            let applied_hash = entry.applied_hash.clone().ok_or_else(|| {
                AppError::Validation(format!("{} 的恢复日志缺少后像哈希", entry.rel_path))
            })?;
            let previous = match &entry.blob {
                Some(blob) => {
                    let backup = target_path(
                        self.fs,
                        store.root(),
                        &blob_rel_path(&journal.batch_dir, blob),
                    )?;
                    let content = self.fs.read_text(&backup, MAX_FULL_READ)?;
                    if blob_name(&content) != *blob {
                        return Err(AppError::Validation(format!(
                            "{} 的备份内容校验失败，未修改任何文件",
                            entry.rel_path
                        )));
                    }
                    Some(content)
                }
                None => None,
            };
            let current_hash = if self.fs.exists(&target) {
                Some(content_hash(&self.fs.read_text(&target, MAX_FULL_READ)?))
            } else {
                None
            };
            let matches_original = previous
                .as_deref()
                .is_some_and(|content| current_hash.as_deref() == Some(content_hash(content).as_str()));
            if current_hash.as_deref() != Some(applied_hash.as_str())
                && !matches_original
                && !(previous.is_none() && current_hash.is_none())
            {
                return Err(AppError::Conflict(format!(
                    "{} 在中断后已被修改或删除；为保护后续工作，未执行自动恢复",
                    entry.rel_path
                )));
            }
            prepared.push(RecoveryEntry {
                rel_path: entry.rel_path.clone(),
                target,
                current_hash,
                previous,
                applied_hash,
            });
        }

        let mut restored = Vec::new();
        let mut removed = Vec::new();
        for entry in &prepared {
            business_target_path(self.fs, store.root(), &entry.rel_path)?;
            let actual_hash = if self.fs.exists(&entry.target) {
                Some(content_hash(&self.fs.read_text(&entry.target, MAX_FULL_READ)?))
            } else {
                None
            };
            if actual_hash != entry.current_hash {
                return Err(AppError::Conflict(format!(
                    "{} 在恢复预检后发生变化，已停止",
                    entry.rel_path
                )));
            }
            if actual_hash.as_deref() != Some(entry.applied_hash.as_str()) {
                continue;
            }
            match &entry.previous {
                Some(content) => {
                    self.fs.write_text(&entry.target, content)?;
                    restored.push(entry.rel_path.clone());
                }
                None => {
                    self.fs.remove_file(&entry.target)?;
                    removed.push(entry.rel_path.clone());
                }
            }
        }

        store.record_recovery(&journal.receipt)?;
        store.remove_journal(&journal.batch_dir)?;
        store.clear_pending(&journal.plan.plan_hash)?;
        store.enforce_retention(&self.clock.now_iso())?;
        Ok(RecoveryReport {
            plan_hash: journal.plan.plan_hash,
            outcome: RecoveryOutcome::RolledBack,
            restored,
            removed,
            finished_at: self.clock.now_iso(),
        })
    }
}

pub struct PruneBackups<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

impl<'a> PruneBackups<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, root: &Path) -> AppResult<RetentionReport> {
        BuildStore::new(self.fs, root)?.enforce_retention(&self.clock.now_iso())
    }
}
