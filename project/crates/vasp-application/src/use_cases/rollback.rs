use super::build_store::BuildStore;
use super::path_safety::{business_target_path, canonical_root, reject_duplicates, target_path};
use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use vasp_domain::{blob_name, blob_rel_path, content_hash, BuildReceipt, Clock, RestoreEntry};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RollbackReport {
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub finished_at: String,
}

pub struct Rollback<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

struct Prepared<'a> {
    entry: &'a RestoreEntry,
    target: PathBuf,
    current: String,
    backup: Option<String>,
}

impl<'a> Rollback<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, receipt: &BuildReceipt, root: &Path) -> AppResult<RollbackReport> {
        reject_duplicates(receipt.restores.iter().map(|entry| entry.rel_path.as_str()))?;
        let root = canonical_root(self.fs, root)?;
        let store = BuildStore::new(self.fs, &root)?;
        store.ensure_no_recovery()?;
        if let Some(record) = store.receipt_record(receipt)? {
            if record.rolled_back {
                return Err(AppError::Conflict("这次写入已经回滚，不能重复执行".into()));
            }
            if let Some(reason) = record.rollback_error {
                return Err(AppError::Conflict(reason));
            }
        }
        let mut prepared = Vec::with_capacity(receipt.restores.len());
        for entry in &receipt.restores {
            let target = business_target_path(self.fs, &root, &entry.rel_path)?;
            let applied_hash = entry.applied_hash.as_deref().ok_or_else(|| {
                AppError::Validation(format!(
                    "{} 缺少 apply 后内容哈希；旧回执不能安全回滚",
                    entry.rel_path
                ))
            })?;
            if !self.fs.exists(&target) {
                if entry.blob.is_none() {
                    continue;
                }
                return Err(AppError::Conflict(format!(
                    "{} 在回滚前已被删除",
                    entry.rel_path
                )));
            }
            let current = self.fs.read_text(&target, MAX_FULL_READ)?;
            if content_hash(&current) != applied_hash {
                return Err(AppError::Conflict(format!(
                    "{} 在 apply 后又被修改；为保护后续工作，已拒绝回滚",
                    entry.rel_path
                )));
            }
            let backup = match &entry.blob {
                Some(blob) => {
                    let dir = receipt.backup_dir.as_deref().ok_or_else(|| {
                        AppError::Validation("回执包含备份项，却没有备份目录".into())
                    })?;
                    let backup_path = target_path(self.fs, &root, &blob_rel_path(dir, blob))?;
                    let content = self.fs.read_text(&backup_path, MAX_FULL_READ)?;
                    if blob_name(&content) != *blob {
                        return Err(AppError::Validation(format!(
                            "{} 的备份内容校验失败",
                            entry.rel_path
                        )));
                    }
                    Some(content)
                }
                None => None,
            };
            prepared.push(Prepared {
                entry,
                target,
                current,
                backup,
            });
        }

        let mut restored = Vec::new();
        let mut removed = Vec::new();
        let mut completed = Vec::new();
        for (index, entry) in prepared.iter().enumerate() {
            if let Err(error) = self.rollback_entry(&root, entry) {
                let compensation_errors = self.compensate(&prepared, &completed);
                if compensation_errors.is_empty() {
                    return Err(error);
                }
                return Err(AppError::External(format!(
                    "{}；恢复回滚前状态时仍失败：{}",
                    error.message(),
                    compensation_errors.join("；")
                )));
            }
            completed.push(index);
            if entry.entry.blob.is_some() {
                restored.push(entry.entry.rel_path.clone());
            } else {
                removed.push(entry.entry.rel_path.clone());
            }
        }

        let report = RollbackReport {
            restored,
            removed,
            finished_at: self.clock.now_iso(),
        };
        store.mark_rolled_back(receipt).map_err(|error| {
            AppError::External(format!(
                "文件已回滚，但回执状态保存失败：{}",
                error.message()
            ))
        })?;
        store.enforce_retention(&self.clock.now_iso())?;
        Ok(report)
    }

    fn rollback_entry(&self, root: &Path, entry: &Prepared<'_>) -> AppResult<()> {
        business_target_path(self.fs, root, &entry.entry.rel_path)?;
        if !self.fs.exists(&entry.target) && entry.entry.blob.is_none() {
            return Ok(());
        }
        let current = self.fs.read_text(&entry.target, MAX_FULL_READ)?;
        if content_hash(&current)
            != entry
                .entry
                .applied_hash
                .as_deref()
                .expect("预检已要求 apply 后内容哈希")
        {
            return Err(AppError::Conflict(format!(
                "{} 在回滚校验后又被修改",
                entry.entry.rel_path
            )));
        }
        match &entry.backup {
            Some(content) => self.fs.write_text(&entry.target, content),
            None => self.fs.remove_file(&entry.target),
        }
    }

    fn compensate(&self, prepared: &[Prepared<'_>], completed: &[usize]) -> Vec<String> {
        let mut errors = Vec::new();
        for &index in completed.iter().rev() {
            let entry = &prepared[index];
            if let Err(error) = self.fs.write_text(&entry.target, &entry.current) {
                errors.push(format!(
                    "{}: {}",
                    entry.entry.rel_path,
                    error.message()
                ));
            }
        }
        errors
    }
}
