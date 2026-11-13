use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use vasp_domain::{
    blob_name, plan_hash, BuildItem, BuildPlan, BuildReceipt, RestoreEntry, WriteAction,
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dftwork-write-safety-{}-{name}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("创建测试目录");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("创建父目录");
    }
    std::fs::write(path, content).expect("写入测试文件");
}

fn item(
    rel_path: &str,
    content: &str,
    action: WriteAction,
    previous_content: Option<&str>,
) -> BuildItem {
    BuildItem {
        rel_path: rel_path.into(),
        content: content.into(),
        action,
        reason: "write safety regression".into(),
        previous_content: previous_content.map(str::to_owned),
    }
}

fn plan(root: &Path, items: Vec<BuildItem>) -> BuildPlan {
    let plan_hash = plan_hash::compute(&root.to_string_lossy(), &items);
    BuildPlan {
        root: root.to_string_lossy().into_owned(),
        items,
        plan_hash,
        created_at: "2026-11-02T00:00:00+00:00".into(),
    }
}

fn run_with_json(command: &str, path: &Path, value: &impl serde::Serialize) -> Output {
    write(
        path,
        &serde_json::to_string(value).expect("序列化命令输入"),
    );
    Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg(command)
        .arg(path)
        .output()
        .expect("运行 vaspcore")
}

fn failure_details(output: &Output) -> String {
    format!(
        "status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn apply_rejects_plan_with_tampered_root() {
    let temp = TestDir::new("tampered-root");
    let approved_root = temp.path().join("approved");
    let other_root = temp.path().join("other");
    std::fs::create_dir_all(&approved_root).unwrap();
    std::fs::create_dir_all(&other_root).unwrap();
    let mut plan = plan(
        &approved_root,
        vec![item("task/INCAR", "ENCUT = 520\n", WriteAction::Create, None)],
    );
    plan.root = other_root.to_string_lossy().into_owned();

    let output = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(!output.status.success(), "修改 root 后仍放行：{}", failure_details(&output));
    assert!(!other_root.join("task/INCAR").exists());
}

#[test]
fn apply_rejects_overwrite_when_previous_content_changed() {
    let temp = TestDir::new("stale-overwrite");
    let root = temp.path().join("root");
    let target = root.join("task/INCAR");
    write(&target, "ENCUT = 400\n");
    let plan = plan(
        &root,
        vec![item(
            "task/INCAR",
            "ENCUT = 520\n",
            WriteAction::Overwrite,
            Some("ENCUT = 400\n"),
        )],
    );
    write(&target, "USER EDIT AFTER APPROVAL\n");

    let output = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(!output.status.success(), "陈旧覆盖仍放行：{}", failure_details(&output));
    assert_eq!(std::fs::read_to_string(target).unwrap(), "USER EDIT AFTER APPROVAL\n");
}

#[test]
fn apply_conflict_does_not_partially_write_batch() {
    let temp = TestDir::new("conflict-partial-write");
    let root = temp.path().join("root");
    let first = root.join("task/INCAR");
    let second = root.join("task/POSCAR");
    write(&first, "ENCUT = 400\n");
    write(&second, "USER POSCAR\n");
    let plan = plan(
        &root,
        vec![
            item(
                "task/INCAR",
                "ENCUT = 520\n",
                WriteAction::Overwrite,
                Some("ENCUT = 400\n"),
            ),
            item("task/POSCAR", "NEW POSCAR\n", WriteAction::Create, None),
        ],
    );

    let output = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(!output.status.success(), "冲突计划意外成功：{}", failure_details(&output));
    assert_eq!(std::fs::read_to_string(first).unwrap(), "ENCUT = 400\n");
    assert_eq!(std::fs::read_to_string(second).unwrap(), "USER POSCAR\n");
}

#[cfg(windows)]
fn create_dir_link(link: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;

    match symlink_dir(target, link) {
        Ok(()) => Ok(()),
        Err(symlink_error) => {
            let output = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()?;
            if output.status.success() {
                Ok(())
            } else {
                Err(symlink_error)
            }
        }
    }
}

#[cfg(not(windows))]
fn create_dir_link(link: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[test]
fn apply_rejects_symlink_escape() {
    let temp = TestDir::new("symlink-escape");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    if let Err(error) = create_dir_link(&root.join("linked"), &outside) {
        eprintln!("当前环境不能创建目录链接，跳过符号链接回归：{error}");
        return;
    }
    let plan = plan(
        &root,
        vec![item("linked/INCAR", "ESCAPED\n", WriteAction::Create, None)],
    );

    let output = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(!output.status.success(), "链接逃逸仍放行：{}", failure_details(&output));
    assert!(!outside.join("INCAR").exists());
}

#[test]
fn apply_write_failure_does_not_leave_partial_batch() {
    let temp = TestDir::new("io-partial-write");
    let root = temp.path().join("root");
    write(&root.join("blocked"), "not a directory");
    let plan = plan(
        &root,
        vec![
            item("first/INCAR", "FIRST\n", WriteAction::Create, None),
            item("blocked/INCAR", "CANNOT WRITE\n", WriteAction::Create, None),
        ],
    );

    let output = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(!output.status.success(), "不可写计划意外成功：{}", failure_details(&output));
    assert!(!root.join("first/INCAR").exists(), "第二项失败后留下了第一项写入");
}

fn receipt(root: &Path, entry: RestoreEntry, backup_dir: Option<&str>) -> BuildReceipt {
    BuildReceipt {
        root: root.to_string_lossy().into_owned(),
        plan_hash: "regression".into(),
        written: vec![entry.rel_path.clone()],
        skipped: vec![],
        backup_dir: backup_dir.map(str::to_owned),
        restores: vec![entry],
        finished_at: "2026-11-02T00:00:00+00:00".into(),
    }
}

#[test]
fn rollback_preserves_created_file_with_later_changes() {
    let temp = TestDir::new("rollback-created-modified");
    let root = temp.path().join("root");
    let target = root.join("task/INCAR");
    write(&target, "KEEP USER WORK\n");
    let receipt = receipt(
        &root,
        RestoreEntry {
            rel_path: "task/INCAR".into(),
            blob: None,
            applied_hash: Some(vasp_domain::content_hash("APPLIED\n")),
        },
        None,
    );

    let output = run_with_json("rollback", &temp.path().join("receipt.json"), &receipt);

    assert!(!output.status.success(), "后续修改仍被回滚删除：{}", failure_details(&output));
    assert_eq!(std::fs::read_to_string(target).unwrap(), "KEEP USER WORK\n");
}

#[test]
fn rollback_preserves_overwritten_file_with_later_changes() {
    let temp = TestDir::new("rollback-overwritten-modified");
    let root = temp.path().join("root");
    let target = root.join("task/INCAR");
    let blob = blob_name("ORIGINAL\n");
    write(&root.join("backup/batch/blobs").join(&blob), "ORIGINAL\n");
    write(&target, "KEEP USER WORK\n");
    let receipt = receipt(
        &root,
        RestoreEntry {
            rel_path: "task/INCAR".into(),
            blob: Some(blob),
            applied_hash: Some(vasp_domain::content_hash("APPLIED\n")),
        },
        Some("backup/batch"),
    );

    let output = run_with_json("rollback", &temp.path().join("receipt.json"), &receipt);

    assert!(!output.status.success(), "后续修改仍被回滚覆盖：{}", failure_details(&output));
    assert_eq!(std::fs::read_to_string(target).unwrap(), "KEEP USER WORK\n");
}

#[test]
fn rollback_rejects_restore_path_outside_root() {
    let temp = TestDir::new("rollback-outside-root");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside.txt");
    std::fs::create_dir_all(&root).unwrap();
    write(&outside, "DO NOT DELETE\n");
    let receipt = receipt(
        &root,
        RestoreEntry {
            rel_path: "../outside.txt".into(),
            blob: None,
            applied_hash: Some(vasp_domain::content_hash("APPLIED\n")),
        },
        None,
    );

    let output = run_with_json("rollback", &temp.path().join("receipt.json"), &receipt);

    assert!(!output.status.success(), "越界回执仍放行：{}", failure_details(&output));
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "DO NOT DELETE\n");
}

#[test]
fn apply_then_rollback_restores_original_state() {
    let temp = TestDir::new("apply-rollback-roundtrip");
    let root = temp.path().join("root");
    let overwritten = root.join("task/INCAR");
    let created = root.join("task/KPOINTS");
    write(&overwritten, "ENCUT = 400\n");
    let plan = plan(
        &root,
        vec![
            item(
                "task/INCAR",
                "ENCUT = 520\n",
                WriteAction::Overwrite,
                Some("ENCUT = 400\n"),
            ),
            item("task/KPOINTS", "GAMMA\n", WriteAction::Create, None),
        ],
    );

    let applied = run_with_json("apply", &temp.path().join("plan.json"), &plan);
    assert!(applied.status.success(), "正常 apply 被拒绝：{}", failure_details(&applied));
    assert_eq!(std::fs::read_to_string(&overwritten).unwrap(), "ENCUT = 520\n");
    assert_eq!(std::fs::read_to_string(&created).unwrap(), "GAMMA\n");
    let envelope: serde_json::Value =
        serde_json::from_slice(&applied.stdout).expect("apply 输出 JSON");
    let receipt: BuildReceipt =
        serde_json::from_value(envelope["data"].clone()).expect("apply 返回完整回执");

    let rolled_back = run_with_json("rollback", &temp.path().join("receipt.json"), &receipt);

    assert!(
        rolled_back.status.success(),
        "未修改的回执不能回滚：{}",
        failure_details(&rolled_back)
    );
    assert_eq!(std::fs::read_to_string(overwritten).unwrap(), "ENCUT = 400\n");
    assert!(!created.exists());
}

#[test]
fn generated_build_plan_can_apply() {
    let temp = TestDir::new("generated-build-plan");
    let root = temp.path().join("root");
    write(&root.join("source/INCAR"), "ENCUT = 400\n");

    let generated = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("build")
        .arg("source")
        .arg("target")
        .arg("--root")
        .arg(&root)
        .args(["--set", "ENCUT=520"])
        .output()
        .expect("生成构建计划");
    assert!(
        generated.status.success(),
        "build 失败：{}",
        failure_details(&generated)
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&generated.stdout).expect("build 输出 JSON");
    let plan: BuildPlan =
        serde_json::from_value(envelope["data"].clone()).expect("build 返回完整计划");

    let applied = run_with_json("apply", &temp.path().join("plan.json"), &plan);

    assert!(applied.status.success(), "生成的计划不能 apply：{}", failure_details(&applied));
    assert_eq!(
        std::fs::read_to_string(root.join("target/INCAR")).unwrap(),
        "ENCUT = 520\n"
    );
}
