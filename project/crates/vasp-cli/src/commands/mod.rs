use vasp_domain::{BondMode, Hermes};
use vasp_application::ports::Fs as FsPort;
use vasp_infrastructure::{HermesCli, HermesRunner, LocalFs};
use vasp_application::{
    GetTrajectory,
    AnalyzeTask, ApplyPlan, BuildInputs, BuildRequest, CompareTasks, GetConvergence, GetForces, GetMagnetization, GetScf, GetStructureScene,
    GetVibrations, Rollback, ScanProject,
};

use crate::support::SystemClock;
use std::path::Path;

/// 所有命令的唯一输出约定：
/// - **stdout**：恰好一个 JSON 对象，`{"ok":true,"data":…}` 或 `{"ok":false,"error":…}`
/// - **stderr**：日志
///
/// Electron 侧只解析 stdout，因此这里不允许出现任何裸打印。
fn emit<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

pub fn scan(root: &str, max_depth: usize) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let clock = SystemClock;
    let report = ScanProject::new(&fs, &clock)
        .with_max_depth(max_depth)
        .execute(Path::new(root), None)?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn convergence(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let series = GetConvergence::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": series }))
}

pub fn scene(file: &str, no_pbc: bool) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let mode = if no_pbc {
        BondMode::Real
    } else {
        BondMode::Periodic
    };
    let scene = GetStructureScene::new(&fs).execute(Path::new(file), mode)?;
    emit(&serde_json::json!({ "ok": true, "data": scene }))
}

pub fn magnetization(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let report = GetMagnetization::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn scf(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let series = GetScf::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": series }))
}

pub fn forces(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let (forces, stress) = GetForces::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({
        "ok": true,
        "data": { "forces": forces, "stress": stress }
    }))
}

pub fn vibrations(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let report = GetVibrations::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn trajectory(task: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let report = GetTrajectory::new(&fs).execute(Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn compare(tasks: &[String]) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let paths: Vec<std::path::PathBuf> = tasks.iter().map(std::path::PathBuf::from).collect();
    let report = CompareTasks::new(&fs).execute(&paths)?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn build(
    source: &str,
    target: &str,
    sets: &[String],
    root_opt: Option<&str>,
    files: &[String],
) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let clock = SystemClock;

    let src = std::path::PathBuf::from(source);
    let (root, source_rel) = match root_opt {
        Some(r) => (std::path::PathBuf::from(r), source.to_string()),
        None => {
            if src.is_absolute() {
                let parent = src
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("源任务路径没有父目录"))?;
                let name = src
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("源任务路径没有目录名"))?
                    .to_string_lossy()
                    .to_string();
                (parent.to_path_buf(), name)
            } else {
                (std::env::current_dir()?, source.to_string())
            }
        }
    };

    // target 必须是相对路径；给了绝对路径就试着剥掉 root 前缀
    let target_rel = {
        let t = std::path::Path::new(target);
        if t.is_absolute() {
            t.strip_prefix(&root)
                .map_err(|_| anyhow::anyhow!("目标目录不在项目根内：{target}"))?
                .to_string_lossy()
                .to_string()
        } else {
            target.to_string()
        }
    };

    let overrides = sets
        .iter()
        .map(|s| {
            let (k, v) = s
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("--set 需要 KEY=VALUE 形式：{s}"))?;
            Ok((k.trim().to_string(), v.trim().to_string()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let req = BuildRequest {
        root,
        source_rel,
        target_rel,
        overrides,
        files: files.to_vec(),
    };
    let plan = BuildInputs::new(&fs, &clock).execute(&req)?;
    emit(&serde_json::json!({ "ok": true, "data": plan }))
}

pub fn apply(plan_path: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let clock = SystemClock;
    let text = fs.read_text(
        std::path::Path::new(plan_path),
        vasp_application::ports::MAX_FULL_READ,
    )?;
    let plan: vasp_domain::BuildPlan = serde_json::from_str(&text)?;
    let receipt = ApplyPlan::new(&fs, &clock).execute(&plan)?;
    emit(&serde_json::json!({ "ok": true, "data": receipt }))
}

pub fn rollback(receipt_path: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let clock = SystemClock;
    let text = fs.read_text(
        std::path::Path::new(receipt_path),
        vasp_application::ports::MAX_FULL_READ,
    )?;
    let receipt: vasp_domain::BuildReceipt = serde_json::from_str(&text)?;
    let root = std::path::PathBuf::from(&receipt.root);
    let report = Rollback::new(&fs, &clock).execute(&receipt, &root)?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn analyze(
    task: &str,
    model: Option<&str>,
    provider: Option<&str>,
    hermes_bin: Option<&str>,
    max_turns: u32,
    timeout: u64,
) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let bin = hermes_bin
        .map(|s| s.to_string())
        .or_else(|| std::env::var("HERMES_BIN").ok())
        .unwrap_or_else(|| "hermes".to_string());
    let runner = HermesRunner::new(bin)
        .with_model(model.map(|s| s.to_string()))
        .with_provider(provider.map(|s| s.to_string()))
        .with_max_turns(max_turns)
        .with_timeout(timeout);
    let report = AnalyzeTask::new(&fs, &runner).execute(std::path::Path::new(task))?;
    emit(&serde_json::json!({ "ok": true, "data": report }))
}

pub fn hermes_probe() -> anyhow::Result<()> {
    let cap = HermesCli::from_path().probe();
    emit(&serde_json::json!({ "ok": true, "data": cap }))
}

pub fn version() -> anyhow::Result<()> {
    emit(&serde_json::json!({
        "ok": true,
        "data": {
            "name": "vaspcore",
            "version": env!("CARGO_PKG_VERSION"),
            "schema_version": 1
        }
    }))
}
