//! 从回执回滚一次写入。
//!
//! 回执里记着每个文件「写之前是什么样」：覆盖项指向备份 blob，新建项标 `None`。
//! 所以回滚不依赖任何全局状态——拿到回执就能还原，哪怕软件重装过。

use crate::ports::{Fs, MAX_FULL_READ};
use crate::AppResult;
use serde::Serialize;
use std::path::Path;
use vasp_domain::{blob_rel_path, BuildReceipt, Clock};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RollbackReport {
    /// 从备份还原的文件。
    pub restored: Vec<String>,
    /// 删除的文件（本次计划新建的）。
    pub removed: Vec<String>,
    pub finished_at: String,
}

pub struct Rollback<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

impl<'a> Rollback<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, receipt: &BuildReceipt, root: &Path) -> AppResult<RollbackReport> {
        let mut restored = Vec::new();
        let mut removed = Vec::new();

        for entry in &receipt.restores {
            let target = root.join(&entry.rel_path);
            match &entry.blob {
                Some(blob) => {
                    let dir = receipt.backup_dir.as_deref().ok_or_else(|| {
                        crate::AppError::Validation(
                            "回执说备份了文件，却没记录备份目录".to_string(),
                        )
                    })?;
                    let content = self
                        .fs
                        .read_text(&root.join(blob_rel_path(dir, blob)), MAX_FULL_READ)?;
                    self.fs.write_text(&target, &content)?;
                    restored.push(entry.rel_path.clone());
                }
                None => {
                    self.fs.remove_file(&target)?;
                    removed.push(entry.rel_path.clone());
                }
            }
        }

        Ok(RollbackReport {
            restored,
            removed,
            finished_at: self.clock.now_iso(),
        })
    }
}
