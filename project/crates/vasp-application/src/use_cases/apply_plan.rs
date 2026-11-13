use super::build_store::{BackupManifest, BuildStore, ManifestEntry, WriteJournal};
use super::path_safety::{business_target_path, reject_duplicates, target_path};
use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};
use vasp_domain::{
    backup_dir_name, blob_name, blob_rel_path, content_hash, plan_hash, BuildItem, BuildPlan,
    BuildReceipt, Clock, RestoreEntry, WriteAction,
};

pub struct ApplyPlan<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

struct Prepared<'a> {
    item: &'a BuildItem,
    target: PathBuf,
    action: WriteAction,
    previous: Option<String>,
}

impl<'a> ApplyPlan<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, plan: &BuildPlan) -> AppResult<BuildReceipt> {
        plan_hash::assert_match(plan).map_err(|_| {
            AppError::Conflict("计划在放行后被改动过（指纹不匹配），已拒绝写入".to_string())
        })?;
        reject_duplicates(plan.items.iter().map(|item| item.rel_path.as_str()))?;
        let store = BuildStore::new(self.fs, Path::new(&plan.root))?;
        store.ensure_no_recovery()?;
        let root = store.root().to_path_buf();

        let mut prepared = Vec::with_capacity(plan.items.len());
        for item in &plan.items {
            let target = business_target_path(self.fs, &root, &item.rel_path)?;
            let (action, previous) = self.preflight(item, &target)?;
            prepared.push(Prepared {
                item,
                target,
                action,
                previous,
            });
        }

        let finished_at = self.clock.now_iso();
        let batch_dir = format!(
            ".dftwork/backup/{}",
            backup_dir_name(&finished_at, &plan.plan_hash)
        );
        let has_overwrites = prepared
            .iter()
            .any(|entry| entry.action == WriteAction::Overwrite);
        let mut written = Vec::new();
        let mut skipped = Vec::new();
        let mut restores = Vec::new();
        let mut manifest_entries = Vec::new();
        for entry in &prepared {
            if entry.action == WriteAction::Skip {
                skipped.push(entry.item.rel_path.clone());
                continue;
            }
            written.push(entry.item.rel_path.clone());
            let blob = (entry.action == WriteAction::Overwrite).then(|| {
                blob_name(
                    entry
                        .previous
                        .as_deref()
                        .expect("覆盖项预检时已保存原内容"),
                )
            });
            let applied_hash = content_hash(&entry.item.content);
            manifest_entries.push(ManifestEntry {
                rel_path: entry.item.rel_path.clone(),
                action: entry.action,
                blob: blob.clone(),
                applied_hash: applied_hash.clone(),
            });
            restores.push(RestoreEntry {
                rel_path: entry.item.rel_path.clone(),
                blob,
                applied_hash: Some(applied_hash),
            });
        }
        let receipt = BuildReceipt {
            root: plan.root.clone(),
            plan_hash: plan.plan_hash.clone(),
            written,
            skipped,
            backup_dir: has_overwrites.then_some(batch_dir.clone()),
            restores,
            finished_at: finished_at.clone(),
        };
        let manifest = BackupManifest {
            schema_version: 1,
            root: plan.root.clone(),
            plan_hash: plan.plan_hash.clone(),
            created_at: finished_at,
            entries: manifest_entries,
        };
        let journal = WriteJournal {
            schema_version: 1,
            batch_dir: batch_dir.clone(),
            plan: plan.clone(),
            receipt: receipt.clone(),
        };

        store.save_pending(plan)?;
        for entry in prepared
            .iter()
            .filter(|entry| entry.action == WriteAction::Overwrite)
        {
            let old = entry.previous.as_deref().ok_or_else(|| {
                AppError::Validation(format!("{} 缺少写入前像", entry.item.rel_path))
            })?;
            let blob = blob_name(old);
            let backup = target_path(self.fs, &root, &blob_rel_path(&batch_dir, &blob))?;
            self.fs.write_text(&backup, old)?;
        }
        store.write_manifest(&batch_dir, &manifest)?;
        store.write_journal(&journal)?;
        failpoint("after-backup");

        let mut completed = Vec::new();
        for (index, entry) in prepared.iter().enumerate() {
            if entry.action == WriteAction::Skip {
                continue;
            }
            if let Err(error) = self.write_entry(&root, entry) {
                let compensation_errors = self.compensate(&prepared, &completed);
                if compensation_errors.is_empty() {
                    store.remove_journal(&batch_dir)?;
                    return Err(error);
                }
                return Err(AppError::External(format!(
                    "{}；补偿已写入项时仍失败：{}。恢复日志已保留，请执行恢复",
                    error.message(),
                    compensation_errors.join("；")
                )));
            }
            completed.push(index);
            if completed.len() == 1 {
                failpoint("after-first-write");
            }
        }

        failpoint("before-receipt");
        store.commit_receipt(&receipt).map_err(|error| {
            AppError::External(format!(
                "业务文件已写入，但回执提交失败：{}。恢复日志已保留，请执行恢复",
                error.message()
            ))
        })?;
        failpoint("after-receipt");
        store.remove_journal(&batch_dir)?;
        store.clear_pending(&plan.plan_hash)?;
        store.enforce_retention(&self.clock.now_iso())?;
        Ok(receipt)
    }

    fn preflight(&self, item: &BuildItem, target: &Path) -> AppResult<(WriteAction, Option<String>)> {
        let current = if self.fs.exists(target) {
            Some(self.fs.read_text(target, MAX_FULL_READ)?)
        } else {
            None
        };

        if current.as_deref() == Some(item.content.as_str()) {
            return Ok((WriteAction::Skip, current));
        }

        match (item.action, current) {
            (WriteAction::Create, None) => {
                if item.previous_content.is_some() {
                    return Err(AppError::Validation(format!(
                        "{} 是新建项，却带有写入前像",
                        item.rel_path
                    )));
                }
                Ok((WriteAction::Create, None))
            }
            (WriteAction::Create, Some(_)) => Err(AppError::Conflict(format!(
                "{} 在放行后被创建或改动，原计划是新建，已拒绝覆盖",
                item.rel_path
            ))),
            (WriteAction::Overwrite, Some(old)) => {
                if item.previous_content.as_deref() != Some(old.as_str()) {
                    return Err(AppError::Conflict(format!(
                        "{} 在放行后被创建或改动，原计划是覆盖，请重新生成计划",
                        item.rel_path
                    )));
                }
                Ok((WriteAction::Overwrite, Some(old)))
            }
            (WriteAction::Overwrite, None) => Err(AppError::Conflict(format!(
                "{} 在放行后被删除，原计划是覆盖，请重新生成计划",
                item.rel_path
            ))),
            (WriteAction::Skip, Some(old)) => {
                if item.previous_content.as_deref() != Some(old.as_str()) {
                    return Err(AppError::Conflict(format!(
                        "{} 在放行后被改动，原计划是不动，已拒绝继续",
                        item.rel_path
                    )));
                }
                Ok((WriteAction::Skip, Some(old)))
            }
            (WriteAction::Skip, None) => Err(AppError::Conflict(format!(
                "{} 在放行后被删除，原计划是不动，已拒绝继续",
                item.rel_path
            ))),
        }
    }

    fn write_entry(&self, root: &Path, entry: &Prepared<'_>) -> AppResult<()> {
        business_target_path(self.fs, root, &entry.item.rel_path)?;
        match entry.action {
            WriteAction::Create => {
                if self.fs.exists(&entry.target) {
                    return Err(AppError::Conflict(format!(
                        "{} 在写入前被创建，已停止本批次",
                        entry.item.rel_path
                    )));
                }
            }
            WriteAction::Overwrite => {
                let current = self.fs.read_text(&entry.target, MAX_FULL_READ)?;
                if Some(current.as_str()) != entry.previous.as_deref() {
                    return Err(AppError::Conflict(format!(
                        "{} 在写入前发生变化，已停止本批次",
                        entry.item.rel_path
                    )));
                }
            }
            WriteAction::Skip => return Ok(()),
        }
        self.fs.write_text(&entry.target, &entry.item.content)
    }

    fn compensate(&self, prepared: &[Prepared<'_>], completed: &[usize]) -> Vec<String> {
        let mut errors = Vec::new();
        for &index in completed.iter().rev() {
            let entry = &prepared[index];
            let result = match entry.action {
                WriteAction::Create => self.fs.remove_file(&entry.target),
                WriteAction::Overwrite => self.fs.write_text(
                    &entry.target,
                    entry
                        .previous
                        .as_deref()
                        .expect("覆盖项补偿需要原内容"),
                ),
                WriteAction::Skip => continue,
            };
            if let Err(error) = result {
                errors.push(format!("{}: {}", entry.item.rel_path, error.message()));
            }
        }
        errors
    }
}

fn failpoint(stage: &str) {
    if cfg!(debug_assertions)
        && std::env::var("DFTWORK_TEST_FAILPOINT").ok().as_deref() == Some(stage)
    {
        std::process::exit(97);
    }
}
