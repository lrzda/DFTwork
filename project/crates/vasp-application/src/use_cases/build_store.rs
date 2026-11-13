use super::path_safety::{canonical_root, target_path};
use crate::ports::{DirEntryInfo, Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use vasp_domain::{plan_hash, BuildPlan, BuildReceipt, WriteAction};

const PENDING_PATH: &str = ".dftwork/plans/pending.json";
const RECEIPTS_PATH: &str = ".dftwork/receipts/index.json";
const BACKUP_ROOT: &str = ".dftwork/backup";
const MAX_RECEIPTS: usize = 100;
const MAX_VERSIONS_PER_FILE: usize = 5;
const MAX_BACKUP_BYTES: u64 = 500 * 1024 * 1024;
const MAX_BACKUP_AGE_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiptRecord {
    pub receipt: BuildReceipt,
    #[serde(default)]
    pub rolled_back: bool,
    #[serde(default)]
    pub rollback_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryNotice {
    pub plan_hash: String,
    pub backup_dir: String,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildState {
    pub pending: Option<BuildPlan>,
    pub receipts: Vec<ReceiptRecord>,
    pub recovery: Option<RecoveryNotice>,
    pub backup_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: u32,
    pub root: String,
    pub plan_hash: String,
    pub created_at: String,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub rel_path: String,
    pub action: WriteAction,
    pub blob: Option<String>,
    pub applied_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteJournal {
    pub schema_version: u32,
    pub batch_dir: String,
    pub plan: BuildPlan,
    pub receipt: BuildReceipt,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetentionReport {
    pub removed_batches: usize,
    pub freed_bytes: u64,
    pub backup_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ReceiptLog {
    #[serde(default)]
    receipts: Vec<ReceiptRecord>,
}

#[derive(Debug, Clone)]
struct BatchInfo {
    dir: String,
    bytes: u64,
    created_at: Option<String>,
    has_journal: bool,
}

#[derive(Debug, Clone, Copy)]
struct RetentionPolicy {
    max_versions: usize,
    max_bytes: u64,
    max_age_seconds: i64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_versions: MAX_VERSIONS_PER_FILE,
            max_bytes: MAX_BACKUP_BYTES,
            max_age_seconds: MAX_BACKUP_AGE_SECONDS,
        }
    }
}

pub struct BuildStore<'a> {
    fs: &'a dyn Fs,
    root: PathBuf,
}

impl<'a> BuildStore<'a> {
    pub fn new(fs: &'a dyn Fs, root: &Path) -> AppResult<Self> {
        Ok(Self {
            fs,
            root: canonical_root(fs, root)?,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, rel: &str) -> AppResult<PathBuf> {
        target_path(self.fs, &self.root, rel)
    }

    fn read_json<T: DeserializeOwned>(&self, rel: &str) -> AppResult<Option<T>> {
        let path = self.path(rel)?;
        if !self.fs.exists(&path) {
            return Ok(None);
        }
        let text = self.fs.read_text(&path, MAX_FULL_READ)?;
        serde_json::from_str(&text).map(Some).map_err(|error| {
            AppError::Validation(format!("持久化状态 {} 已损坏：{error}", path.display()))
        })
    }

    fn write_json<T: Serialize>(&self, rel: &str, value: &T) -> AppResult<()> {
        let path = self.path(rel)?;
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| AppError::External(format!("序列化项目状态失败：{error}")))?;
        self.fs.write_text(&path, &text)
    }

    fn assert_root(&self, stored_root: &str) -> AppResult<()> {
        let stored = canonical_root(self.fs, Path::new(stored_root))?;
        if stored != self.root {
            return Err(AppError::Conflict(format!(
                "状态属于项目 {}，不能用于 {}",
                stored.display(),
                self.root.display()
            )));
        }
        Ok(())
    }

    fn pending(&self) -> AppResult<Option<BuildPlan>> {
        let pending = self.pending_unchecked()?;
        if let Some(plan) = &pending {
            self.assert_root(&plan.root)?;
            plan_hash::assert_match(plan).map_err(|_| {
                AppError::Validation("pending plan 指纹不匹配，请丢弃后重新生成".into())
            })?;
        }
        Ok(pending)
    }

    fn pending_unchecked(&self) -> AppResult<Option<BuildPlan>> {
        self.read_json(PENDING_PATH)
    }

    fn receipt_log(&self) -> AppResult<ReceiptLog> {
        let log = self.receipt_log_unchecked()?;
        for record in &log.receipts {
            self.assert_root(&record.receipt.root)?;
        }
        Ok(log)
    }

    fn receipt_log_unchecked(&self) -> AppResult<ReceiptLog> {
        Ok(self.read_json(RECEIPTS_PATH)?.unwrap_or_default())
    }

    fn save_receipt_log(&self, log: &ReceiptLog) -> AppResult<()> {
        self.write_json(RECEIPTS_PATH, log)
    }

    pub fn save_pending(&self, plan: &BuildPlan) -> AppResult<()> {
        self.ensure_no_recovery()?;
        self.assert_root(&plan.root)?;
        plan_hash::assert_match(plan)
            .map_err(|_| AppError::Conflict("计划指纹不匹配，不能保存为 pending".into()))?;
        self.write_json(PENDING_PATH, plan)
    }

    pub fn discard_pending(&self) -> AppResult<()> {
        self.ensure_no_recovery()?;
        self.fs.remove_file(&self.path(PENDING_PATH)?)
    }

    pub fn clear_pending(&self, plan_hash: &str) -> AppResult<()> {
        if self
            .pending_unchecked()?
            .is_some_and(|plan| plan.plan_hash == plan_hash)
        {
            self.fs.remove_file(&self.path(PENDING_PATH)?)?;
        }
        Ok(())
    }

    pub(crate) fn write_manifest(
        &self,
        batch_dir: &str,
        manifest: &BackupManifest,
    ) -> AppResult<()> {
        self.write_json(&format!("{batch_dir}/manifest.json"), manifest)
    }

    pub(crate) fn write_journal(&self, journal: &WriteJournal) -> AppResult<()> {
        self.assert_root(&journal.plan.root)?;
        self.write_json(&format!("{}/journal.json", journal.batch_dir), journal)
    }

    pub(crate) fn remove_journal(&self, batch_dir: &str) -> AppResult<()> {
        self.fs
            .remove_file(&self.path(&format!("{batch_dir}/journal.json"))?)
    }

    fn read_journal(&self) -> AppResult<Option<WriteJournal>> {
        let backup_root = self.path(BACKUP_ROOT)?;
        if !self.fs.is_dir(&backup_root) {
            return Ok(None);
        }
        let mut journals = Vec::new();
        for entry in self.fs.list_dir(&backup_root)? {
            if !entry.is_dir {
                continue;
            }
            let batch_dir = format!("{BACKUP_ROOT}/{}", entry.name);
            let journal_path = format!("{batch_dir}/journal.json");
            if self.fs.exists(&self.path(&journal_path)?) {
                let journal: WriteJournal = self.read_json(&journal_path)?.ok_or_else(|| {
                    AppError::Validation("恢复日志读取失败".into())
                })?;
                if journal.schema_version != 1 {
                    return Err(AppError::Validation(format!(
                        "不支持的恢复日志版本：{}",
                        journal.schema_version
                    )));
                }
                self.assert_root(&journal.plan.root)?;
                plan_hash::assert_match(&journal.plan).map_err(|_| {
                    AppError::Validation("恢复日志中的计划指纹不匹配".into())
                })?;
                journals.push(journal);
            }
        }
        if journals.len() > 1 {
            return Err(AppError::Conflict(
                "存在多个未完成写入；为避免误恢复，请先检查 .dftwork/backup".into(),
            ));
        }
        Ok(journals.pop())
    }

    pub fn recovery(&self) -> AppResult<Option<(WriteJournal, bool)>> {
        let Some(journal) = self.read_journal()? else {
            return Ok(None);
        };
        let committed = self
            .receipt_log()?
            .receipts
            .iter()
            .any(|record| record.receipt == journal.receipt);
        Ok(Some((journal, committed)))
    }

    pub fn ensure_no_recovery(&self) -> AppResult<()> {
        if self.recovery()?.is_some() {
            return Err(AppError::Conflict(
                "检测到未完成的写入，请先执行恢复，再生成计划或写入".into(),
            ));
        }
        Ok(())
    }

    pub fn commit_receipt(&self, receipt: &BuildReceipt) -> AppResult<()> {
        self.assert_root(&receipt.root)?;
        let mut log = self.receipt_log()?;
        if log.receipts.iter().any(|record| {
            record.receipt.plan_hash == receipt.plan_hash
                && record.receipt.finished_at == receipt.finished_at
        }) {
            return Ok(());
        }
        log.receipts.insert(
            0,
            ReceiptRecord {
                receipt: receipt.clone(),
                rolled_back: false,
                rollback_error: None,
            },
        );
        log.receipts.truncate(MAX_RECEIPTS);
        self.save_receipt_log(&log)
    }

    pub fn record_recovery(&self, receipt: &BuildReceipt) -> AppResult<()> {
        let mut log = self.receipt_log()?;
        if let Some(record) = log
            .receipts
            .iter_mut()
            .find(|record| record.receipt == *receipt)
        {
            record.rolled_back = true;
        } else {
            log.receipts.insert(
                0,
                ReceiptRecord {
                    receipt: receipt.clone(),
                    rolled_back: true,
                    rollback_error: None,
                },
            );
            log.receipts.truncate(MAX_RECEIPTS);
        }
        self.save_receipt_log(&log)
    }

    pub fn mark_rolled_back(&self, receipt: &BuildReceipt) -> AppResult<()> {
        let mut log = self.receipt_log()?;
        if let Some(record) = log.receipts.iter_mut().find(|record| {
            record.receipt.plan_hash == receipt.plan_hash
                && record.receipt.finished_at == receipt.finished_at
        }) {
            record.rolled_back = true;
        } else {
            log.receipts.insert(
                0,
                ReceiptRecord {
                    receipt: receipt.clone(),
                    rolled_back: true,
                    rollback_error: None,
                },
            );
            log.receipts.truncate(MAX_RECEIPTS);
        }
        self.save_receipt_log(&log)
    }

    pub fn receipt_record(&self, receipt: &BuildReceipt) -> AppResult<Option<ReceiptRecord>> {
        Ok(self
            .load()?
            .receipts
            .into_iter()
            .find(|record| record.receipt == *receipt))
    }

    pub fn load(&self) -> AppResult<BuildState> {
        let mut records = self.receipt_log()?.receipts;
        for record in &mut records {
            if record.rollback_error.is_some() || record.receipt.backup_dir.is_none() {
                continue;
            }
            let dir = record.receipt.backup_dir.as_deref().unwrap();
            let missing = record.receipt.restores.iter().any(|entry| {
                entry.blob.as_ref().is_some_and(|blob| {
                    let path = format!("{dir}/blobs/{blob}");
                    self.path(&path)
                        .map(|path| !self.fs.exists(&path))
                        .unwrap_or(true)
                })
            });
            if missing {
                record.rollback_error = Some("备份不存在或已被清理，不能回滚".into());
            }
        }
        let recovery = self.recovery()?.map(|(journal, committed)| RecoveryNotice {
            plan_hash: journal.plan.plan_hash,
            backup_dir: journal.batch_dir,
            committed,
        });
        let backup_bytes = self.backup_bytes()?;
        Ok(BuildState {
            pending: self.pending()?,
            receipts: records,
            recovery,
            backup_bytes,
        })
    }

    fn batches(&self) -> AppResult<Vec<BatchInfo>> {
        let backup_root = self.path(BACKUP_ROOT)?;
        if !self.fs.is_dir(&backup_root) {
            return Ok(Vec::new());
        }
        let mut batches = Vec::new();
        for DirEntryInfo { name, is_dir, .. } in self.fs.list_dir(&backup_root)? {
            if !is_dir {
                continue;
            }
            let dir = format!("{BACKUP_ROOT}/{name}");
            let manifest: Option<BackupManifest> =
                self.read_json(&format!("{dir}/manifest.json"))?;
            batches.push(BatchInfo {
                has_journal: self.fs.exists(&self.path(&format!("{dir}/journal.json"))?),
                bytes: self.dir_bytes(&dir)?,
                created_at: manifest.map(|manifest| manifest.created_at),
                dir,
            });
        }
        Ok(batches)
    }

    fn dir_bytes(&self, rel: &str) -> AppResult<u64> {
        let path = self.path(rel)?;
        if !self.fs.is_dir(&path) || self.fs.is_symlink(&path) {
            return Ok(0);
        }
        let mut bytes = 0_u64;
        for entry in self.fs.list_dir(&path)? {
            let child = format!("{rel}/{}", entry.name);
            if entry.is_dir {
                bytes = bytes.saturating_add(self.dir_bytes(&child)?);
            } else {
                bytes = bytes.saturating_add(entry.size);
            }
        }
        Ok(bytes)
    }

    pub fn backup_bytes(&self) -> AppResult<u64> {
        Ok(self.batches()?.iter().map(|batch| batch.bytes).sum())
    }

    pub fn enforce_retention(&self, now_iso: &str) -> AppResult<RetentionReport> {
        self.enforce_retention_with(now_iso, RetentionPolicy::default())
    }

    fn enforce_retention_with(
        &self,
        now_iso: &str,
        policy: RetentionPolicy,
    ) -> AppResult<RetentionReport> {
        let now = OffsetDateTime::parse(now_iso, &Rfc3339)
            .map_err(|error| AppError::Validation(format!("保留策略时间无效：{error}")))?;
        let mut log = self.receipt_log()?;
        let batches = self.batches()?;
        let receipts_by_dir: HashMap<String, Vec<usize>> = log
            .receipts
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                record
                    .receipt
                    .backup_dir
                    .as_deref()
                    .map(|dir| (dir.to_string(), index))
            })
            .fold(HashMap::new(), |mut map, (dir, index)| {
                map.entry(dir).or_default().push(index);
                map
            });

        let remove = retention_candidates(&batches, &log.receipts, now, policy);
        let mut freed_bytes = 0_u64;
        for batch in batches.iter().filter(|batch| remove.contains(&batch.dir)) {
            self.fs.remove_dir_all(&self.path(&batch.dir)?)?;
            freed_bytes = freed_bytes.saturating_add(batch.bytes);
            if let Some(indices) = receipts_by_dir.get(batch.dir.as_str()) {
                for &index in indices {
                    log.receipts[index].rollback_error =
                        Some("备份已按 5 版 / 500 MB / 30 天保留策略清理".into());
                }
            }
        }
        self.save_receipt_log(&log)?;
        Ok(RetentionReport {
            removed_batches: remove.len(),
            freed_bytes,
            backup_bytes: self.backup_bytes()?,
        })
    }
}

fn retention_candidates(
    batches: &[BatchInfo],
    receipts: &[ReceiptRecord],
    now: OffsetDateTime,
    policy: RetentionPolicy,
) -> HashSet<String> {
    let mut retained = HashSet::new();
    let mut versions: HashMap<String, usize> = HashMap::new();
    for record in receipts {
        let Some(dir) = record.receipt.backup_dir.as_deref() else {
            continue;
        };
        for entry in &record.receipt.restores {
            if entry.blob.is_none() {
                continue;
            }
            let count = versions.entry(entry.rel_path.clone()).or_default();
            if *count < policy.max_versions {
                retained.insert(dir.to_string());
                *count += 1;
            }
        }
    }

    let referenced: HashSet<&str> = receipts
        .iter()
        .filter_map(|record| record.receipt.backup_dir.as_deref())
        .collect();
    let mut remove = HashSet::new();
    for batch in batches {
        if batch.has_journal {
            continue;
        }
        let expired = batch
            .created_at
            .as_deref()
            .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
            .is_some_and(|created| now - created > time::Duration::seconds(policy.max_age_seconds));
        if expired || !referenced.contains(batch.dir.as_str()) || !retained.contains(&batch.dir) {
            remove.insert(batch.dir.clone());
        }
    }

    let mut remaining: u64 = batches
        .iter()
        .filter(|batch| !remove.contains(&batch.dir))
        .map(|batch| batch.bytes)
        .sum();
    let mut by_age = batches
        .iter()
        .filter(|batch| !batch.has_journal && !remove.contains(&batch.dir))
        .collect::<Vec<_>>();
    by_age.sort_by_key(|batch| batch.created_at.clone());
    for batch in by_age {
        if remaining <= policy.max_bytes {
            break;
        }
        remaining = remaining.saturating_sub(batch.bytes);
        remove.insert(batch.dir.clone());
    }
    remove
}

#[cfg(test)]
mod tests {
    use super::*;
    use vasp_domain::RestoreEntry;

    fn batch(name: &str, bytes: u64, created_at: &str) -> BatchInfo {
        BatchInfo {
            dir: name.into(),
            bytes,
            created_at: Some(created_at.into()),
            has_journal: false,
        }
    }

    fn record(dir: &str, path: &str) -> ReceiptRecord {
        ReceiptRecord {
            receipt: BuildReceipt {
                root: "root".into(),
                plan_hash: dir.into(),
                written: vec![path.into()],
                skipped: vec![],
                backup_dir: Some(dir.into()),
                restores: vec![RestoreEntry {
                    rel_path: path.into(),
                    blob: Some("blob".into()),
                    applied_hash: Some("hash".into()),
                }],
                finished_at: "2026-11-03T00:00:00Z".into(),
            },
            rolled_back: false,
            rollback_error: None,
        }
    }

    #[test]
    fn retention_removes_batches_older_than_thirty_days() {
        let now = OffsetDateTime::parse("2026-11-03T00:00:00Z", &Rfc3339).unwrap();
        let remove = retention_candidates(
            &[
                batch("old", 10, "2026-10-01T00:00:00Z"),
                batch("new", 10, "2026-11-02T00:00:00Z"),
            ],
            &[record("old", "task/INCAR"), record("new", "task/INCAR")],
            now,
            RetentionPolicy::default(),
        );
        assert!(remove.contains("old"));
        assert!(!remove.contains("new"));
    }

    #[test]
    fn retention_enforces_capacity_and_protects_recovery_journal() {
        let now = OffsetDateTime::parse("2026-11-03T00:00:00Z", &Rfc3339).unwrap();
        let mut recovering = batch("recovering", 400, "2026-10-01T00:00:00Z");
        recovering.has_journal = true;
        let remove = retention_candidates(
            &[
                batch("oldest", 300, "2026-11-01T00:00:00Z"),
                batch("newest", 300, "2026-11-02T00:00:00Z"),
                recovering,
            ],
            &[
                record("oldest", "a/INCAR"),
                record("newest", "b/INCAR"),
                record("recovering", "c/INCAR"),
            ],
            now,
            RetentionPolicy {
                max_versions: 5,
                max_bytes: 700,
                max_age_seconds: 30 * 24 * 60 * 60,
            },
        );
        assert!(remove.contains("oldest"));
        assert!(!remove.contains("newest"));
        assert!(!remove.contains("recovering"));
    }
}
