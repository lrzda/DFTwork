use vasp_domain::Hermes;
use vasp_infrastructure::{HermesCli, LocalFs};
use vasp_application::{GetConvergence, GetStructureScene, ScanProject};

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

pub fn scene(file: &str) -> anyhow::Result<()> {
    let fs = LocalFs::new();
    let scene = GetStructureScene::new(&fs).execute(Path::new(file))?;
    emit(&serde_json::json!({ "ok": true, "data": scene }))
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
