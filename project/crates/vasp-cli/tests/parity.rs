//! 与旧实现的对拍测试。
//!
//! 流程：`node scripts/gen-golden.mjs` 用 reference 里的原 JS 生成
//! `tests/golden/*.json`；这里跑同一组 fixture 的 Rust 实现，逐字段比对。
//!
//! 没有 golden 时测试跳过而不是失败——golden 是生成物，不进版本库。

use std::path::{Path, PathBuf};
use vasp_domain::{parse_oszicar, parse_outcar_forces};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/vasp-cli 应该有两级父目录")
        .to_path_buf()
}

#[test]
fn convergence_matches_reference_golden() {
    let golden_dir = repo_root().join("tests").join("golden");
    if !golden_dir.exists() {
        eprintln!("跳过：{} 不存在，先跑 node scripts/gen-golden.mjs", golden_dir.display());
        return;
    }

    let mut checked = 0usize;
    let entries = std::fs::read_dir(&golden_dir).expect("读 golden 目录");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("读 golden");
        let json: serde_json::Value = serde_json::from_str(&text).expect("golden 是 JSON");

        let fixture = json["fixture"].as_str().expect("fixture 字段").to_string();
        let fixture_dir = repo_root().join("tests").join("fixtures").join(&fixture);

        // 能量：比步数与数值
        let oszicar = std::fs::read_to_string(fixture_dir.join("OSZICAR"))
            .unwrap_or_default();
        let steps = parse_oszicar(&oszicar);

        let expected_points = json["ionic_points"]
            .as_array()
            .expect("ionic_points 是数组");
        assert_eq!(
            steps.len(),
            expected_points.len(),
            "{fixture}: 离子步数量不一致"
        );

        for (i, (got, want)) in steps.iter().zip(expected_points.iter()).enumerate() {
            let want_e = want
                .get("energy")
                .or_else(|| want.get("e"))
                .and_then(|v| v.as_f64());
            if let (Some(g), Some(w)) = (got.energy_ev, want_e) {
                assert!(
                    (g - w).abs() < 1e-6,
                    "{fixture} 第 {i} 步能量：Rust={g}，JS={w}"
                );
            }
        }

        // 力：比段数
        let outcar = std::fs::read_to_string(fixture_dir.join("OUTCAR")).unwrap_or_default();
        let forces = parse_outcar_forces(&outcar);
        let expected_forces = json["max_forces"].as_array().expect("max_forces 是数组");
        assert_eq!(
            forces.len(),
            expected_forces.len(),
            "{fixture}: 力段数量不一致"
        );

        checked += 1;
    }

    assert!(checked > 0, "一个 golden 都没比对到");
    eprintln!("对拍通过：{checked} 个 fixture");
}
