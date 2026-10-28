//! 执行一份已放行的构建计划。
//!
//! 这是**全系统唯一的落盘点**。ADR-004 的两阶段：AI/向导产出计划 → 用户看到 diff
//! 并放行 → 这里才真正写盘。指纹校验保证「用户看到的」和「即将写的」是同一份。

use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::{Component, Path};
use vasp_domain::{
    backup_dir_name, blob_name, blob_rel_path, plan_hash, BuildPlan, BuildReceipt, Clock,
    RestoreEntry, WriteAction,
};

pub struct ApplyPlan<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

impl<'a> ApplyPlan<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, plan: &BuildPlan) -> AppResult<BuildReceipt> {
        // 先校验还是先写？必须先校验：一旦开始写，「计划被改过」这件事就无法撤销了
        plan_hash::assert_match(plan).map_err(|_| {
            AppError::Conflict("计划在放行后被改动过（指纹不匹配），已拒绝写入".to_string())
        })?;

        // 路径逃逸防护：`..` 或绝对路径能让计划写到项目根之外去
        for item in &plan.items {
            if !is_safe_rel_path(&item.rel_path) {
                return Err(AppError::Validation(format!(
                    "计划里的路径不安全（含 .. 或是绝对路径）：{}",
                    item.rel_path
                )));
            }
        }

        let root = Path::new(&plan.root);
        let dir = format!("backup/{}", backup_dir_name(&self.clock.now_iso(), &plan.plan_hash));

        let mut written = Vec::new();
        let mut skipped = Vec::new();
        let mut restores = Vec::new();
        let mut any_backup = false;

        for item in &plan.items {
            let target = root.join(&item.rel_path);

            match item.action {
                WriteAction::Skip => {
                    skipped.push(item.rel_path.clone());
                    continue;
                }
                WriteAction::Overwrite => {
                    // 覆盖前必须把原内容存下来，否则回滚无从谈起
                    let old = self.fs.read_text(&target, MAX_FULL_READ)?;
                    let blob = blob_name(&old);
                    self.fs
                        .write_text(&root.join(blob_rel_path(&dir, &blob)), &old)?;
                    restores.push(RestoreEntry {
                        rel_path: item.rel_path.clone(),
                        blob: Some(blob),
                    });
                    any_backup = true;
                }
                WriteAction::Create => {
                    // 原来没有这个文件，回滚时删掉即可
                    restores.push(RestoreEntry {
                        rel_path: item.rel_path.clone(),
                        blob: None,
                    });
                }
            }

            self.fs.write_text(&target, &item.content)?;
            written.push(item.rel_path.clone());
        }

        Ok(BuildReceipt {
            root: plan.root.clone(),
            plan_hash: plan.plan_hash.clone(),
            written,
            skipped,
            backup_dir: any_backup.then_some(dir),
            restores,
            finished_at: self.clock.now_iso(),
        })
    }
}

/// 相对路径必须是「纯普通段」：不能是绝对路径，也不能有 `..` / `.` / 盘符前缀。
///
/// 光看 `starts_with(root)` 不够——`root/../etc/passwd` 在拼接前就逃逸了，
/// 而先拼接再规范化又把「写到哪」变成了运行时才知道的事。
fn is_safe_rel_path(rel: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    let p = Path::new(rel);
    !p.is_absolute() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escape_paths() {
        assert!(is_safe_rel_path("task-1/INCAR"));
        assert!(is_safe_rel_path("INCAR"));
        assert!(!is_safe_rel_path("../outside/INCAR"));
        assert!(!is_safe_rel_path("a/../../b"));
        assert!(!is_safe_rel_path("C:/Windows/system32/x"));
        assert!(!is_safe_rel_path("/etc/passwd"));
        assert!(!is_safe_rel_path(""));
    }
}
