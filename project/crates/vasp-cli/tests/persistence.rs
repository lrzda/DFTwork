use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use vasp_domain::{plan_hash, BuildItem, BuildPlan, BuildReceipt, WriteAction};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "dftwork-persistence-{}-{name}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .args(args)
        .output()
        .unwrap()
}

fn details(output: &Output) -> String {
    format!(
        "status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn data(output: &Output) -> Value {
    assert!(output.status.success(), "{}", details(output));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], true);
    envelope["data"].clone()
}

fn state(root: &Path) -> Value {
    data(&run(&["project-state", &root.to_string_lossy()]))
}

fn apply(root: &Path, plan: &BuildPlan, failpoint: Option<&str>) -> Output {
    let path = root.join("approved-plan.json");
    write(&path, &serde_json::to_string(plan).unwrap());
    let mut command = Command::new(env!("CARGO_BIN_EXE_vaspcore"));
    command.arg("apply").arg(path);
    if let Some(failpoint) = failpoint {
        command.env("DFTWORK_TEST_FAILPOINT", failpoint);
    }
    command.output().unwrap()
}

fn plan(root: &Path, items: Vec<BuildItem>) -> BuildPlan {
    BuildPlan {
        root: root.to_string_lossy().into_owned(),
        plan_hash: plan_hash::compute(&root.to_string_lossy(), &items),
        items,
        created_at: "2026-11-03T00:00:00Z".into(),
    }
}

fn item(path: &str, content: &str, previous: Option<&str>) -> BuildItem {
    BuildItem {
        rel_path: path.into(),
        content: content.into(),
        action: if previous.is_some() {
            WriteAction::Overwrite
        } else {
            WriteAction::Create
        },
        reason: "persistence regression".into(),
        previous_content: previous.map(str::to_owned),
    }
}

#[test]
fn pending_plan_and_receipt_survive_process_restarts() {
    let temp = TestDir::new("roundtrip");
    let root = temp.path().join("root");
    write(&root.join("source/INCAR"), "ENCUT = 400\n");
    write(&root.join("target/INCAR"), "ENCUT = 300\n");

    let generated = run(&[
        "build",
        "source",
        "target",
        "--root",
        &root.to_string_lossy(),
        "--file",
        "INCAR",
        "--set",
        "ENCUT=520",
    ]);
    let plan: BuildPlan = serde_json::from_value(data(&generated)).unwrap();
    let restored: BuildPlan =
        serde_json::from_value(state(&root)["pending"].clone()).expect("重载后找回 pending plan");
    assert_eq!(restored, plan);
    assert_eq!(
        root.join(".dftwork/plans/pending.json").exists(),
        true,
        "pending plan 必须原子持久化在项目内"
    );

    let applied = data(&apply(&root, &plan, None));
    let receipt: BuildReceipt = serde_json::from_value(applied).unwrap();
    let backup_dir = receipt.backup_dir.as_deref().unwrap();
    assert!(backup_dir.starts_with(".dftwork/backup/"));
    assert!(root.join(backup_dir).join("manifest.json").is_file());

    let loaded = state(&root);
    assert_eq!(loaded["pending"], Value::Null);
    assert_eq!(loaded["receipts"].as_array().unwrap().len(), 1);
    assert_eq!(loaded["receipts"][0]["receipt"], serde_json::to_value(&receipt).unwrap());
    assert_eq!(loaded["receipts"][0]["rolled_back"], false);
    assert!(loaded["backup_bytes"].as_u64().unwrap() > 0);

    let receipt_path = temp.path().join("receipt.json");
    write(&receipt_path, &serde_json::to_string(&receipt).unwrap());
    let rolled_back = run(&["rollback", &receipt_path.to_string_lossy()]);
    assert!(rolled_back.status.success(), "{}", details(&rolled_back));
    assert_eq!(
        std::fs::read_to_string(root.join("target/INCAR")).unwrap(),
        "ENCUT = 300\n"
    );
    assert_eq!(state(&root)["receipts"][0]["rolled_back"], true);
}

#[test]
fn crash_during_writes_can_restore_the_whole_batch() {
    let temp = TestDir::new("mid-write-crash");
    let root = temp.path().join("root");
    write(&root.join("task/INCAR"), "ORIGINAL\n");
    let plan = plan(
        &root,
        vec![
            item("task/INCAR", "APPLIED\n", Some("ORIGINAL\n")),
            item("task/KPOINTS", "CREATED\n", None),
        ],
    );

    let crashed = apply(&root, &plan, Some("after-first-write"));
    assert!(!crashed.status.success(), "故障注入没有中止进程");
    assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "APPLIED\n");

    let interrupted = state(&root);
    assert_eq!(interrupted["recovery"]["committed"], false);
    assert_eq!(interrupted["recovery"]["plan_hash"], plan.plan_hash);
    let recovered = data(&run(&["recover", &root.to_string_lossy()]));
    assert_eq!(recovered["outcome"], "rolled_back");
    assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "ORIGINAL\n");
    assert!(!root.join("task/KPOINTS").exists());
    let restored = state(&root);
    assert_eq!(restored["recovery"], Value::Null);
    assert_eq!(restored["pending"], Value::Null);
    assert_eq!(restored["receipts"][0]["rolled_back"], true);
}

#[test]
fn crash_after_receipt_commit_preserves_applied_work_and_receipt() {
    let temp = TestDir::new("committed-crash");
    let root = temp.path().join("root");
    write(&root.join("task/INCAR"), "ORIGINAL\n");
    let plan = plan(
        &root,
        vec![item("task/INCAR", "APPLIED\n", Some("ORIGINAL\n"))],
    );

    let crashed = apply(&root, &plan, Some("after-receipt"));
    assert!(!crashed.status.success(), "故障注入没有中止进程");
    let interrupted = state(&root);
    assert_eq!(interrupted["recovery"]["committed"], true);
    assert_eq!(interrupted["receipts"].as_array().unwrap().len(), 1);

    let recovered = data(&run(&["recover", &root.to_string_lossy()]));
    assert_eq!(recovered["outcome"], "committed");
    assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "APPLIED\n");
    let committed = state(&root);
    assert_eq!(committed["recovery"], Value::Null);
    assert_eq!(committed["pending"], Value::Null);
    assert_eq!(committed["receipts"][0]["rolled_back"], false);
}

#[test]
fn backup_and_receipt_failures_do_not_lose_recovery_information() {
    for failpoint in ["after-backup", "before-receipt"] {
        let temp = TestDir::new(failpoint);
        let root = temp.path().join("root");
        write(&root.join("task/INCAR"), "ORIGINAL\n");
        let plan = plan(
            &root,
            vec![
                item("task/INCAR", "APPLIED\n", Some("ORIGINAL\n")),
                item("task/KPOINTS", "CREATED\n", None),
            ],
        );

        let crashed = apply(&root, &plan, Some(failpoint));
        assert!(!crashed.status.success(), "{}", details(&crashed));
        assert_eq!(state(&root)["recovery"]["committed"], false);
        assert_eq!(root.join("task/KPOINTS").exists(), failpoint == "before-receipt");
        data(&run(&["recover", &root.to_string_lossy()]));
        assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "ORIGINAL\n");
        assert!(!root.join("task/KPOINTS").exists());
    }
}

#[test]
fn backup_io_failure_happens_before_any_business_write() {
    let temp = TestDir::new("backup-io");
    let root = temp.path().join("root");
    write(&root.join("task/INCAR"), "ORIGINAL\n");
    write(&root.join(".dftwork/backup"), "not a directory");
    let plan = plan(
        &root,
        vec![item("task/INCAR", "APPLIED\n", Some("ORIGINAL\n"))],
    );

    let failed = apply(&root, &plan, None);
    assert!(!failed.status.success(), "{}", details(&failed));
    assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "ORIGINAL\n");
    let persisted = state(&root);
    assert_eq!(persisted["pending"]["plan_hash"], plan.plan_hash);
    assert_eq!(persisted["recovery"], Value::Null);
}

#[test]
fn receipt_history_and_backup_versions_are_bounded() {
    let temp = TestDir::new("retention");
    let root = temp.path().join("root");
    write(&root.join("task/INCAR"), "VERSION 0\n");

    for version in 1..=101 {
        let plan = plan(
            &root,
            vec![item(
                "task/INCAR",
                &format!("VERSION {version}\n"),
                Some(&format!("VERSION {}\n", version - 1)),
            )],
        );
        let output = apply(&root, &plan, None);
        assert!(output.status.success(), "version={version} {}", details(&output));
    }

    let persisted = state(&root);
    assert_eq!(persisted["receipts"].as_array().unwrap().len(), 100);
    let backup_count = std::fs::read_dir(root.join(".dftwork/backup"))
        .unwrap()
        .count();
    assert_eq!(backup_count, 5, "同一文件最多保留 5 个可回滚版本");
    let active = persisted["receipts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["rollback_error"].is_null())
        .count();
    assert_eq!(active, 5, "被清理的回执必须明确标为不可回滚");
}

#[test]
fn expired_backup_marks_receipt_unavailable_and_preserves_current_file() {
    let temp = TestDir::new("expired");
    let root = temp.path().join("root");
    write(&root.join("task/INCAR"), "ORIGINAL\n");
    let plan = plan(
        &root,
        vec![item("task/INCAR", "APPLIED\n", Some("ORIGINAL\n"))],
    );
    let receipt: BuildReceipt = serde_json::from_value(data(&apply(&root, &plan, None))).unwrap();
    let manifest_path = root.join(receipt.backup_dir.as_deref().unwrap()).join("manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["created_at"] = Value::String("2026-09-01T00:00:00Z".into());
    write(&manifest_path, &serde_json::to_string(&manifest).unwrap());

    let pruned = data(&run(&["prune-backups", &root.to_string_lossy()]));
    assert_eq!(pruned["removed_batches"], 1);
    let record = state(&root)["receipts"][0].clone();
    assert!(record["rollback_error"].as_str().unwrap().contains("30 天"));

    let receipt_path = temp.path().join("expired-receipt.json");
    write(&receipt_path, &serde_json::to_string(&receipt).unwrap());
    let rolled_back = run(&["rollback", &receipt_path.to_string_lossy()]);
    assert!(!rolled_back.status.success(), "{}", details(&rolled_back));
    assert_eq!(std::fs::read_to_string(root.join("task/INCAR")).unwrap(), "APPLIED\n");
}

#[test]
fn apply_rejects_writes_to_dftwork_metadata() {
    let temp = TestDir::new("metadata-target");
    let root = temp.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let plan = plan(
        &root,
        vec![item(".dftwork/receipts/index.json", "NOT JSON\n", None)],
    );

    let failed = apply(&root, &plan, None);
    assert!(!failed.status.success(), "{}", details(&failed));
    assert!(!root.join(".dftwork/receipts/index.json").exists());
    assert_eq!(state(&root)["recovery"], Value::Null);
}
