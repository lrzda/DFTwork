//! 与旧实现的确定性对拍测试。
//!
//! `tests/parity-fixtures.json` 显式列出必需输入和基准。输入或 golden 缺失、
//! 读取失败都必须使测试失败，不能在 CI 中静默跳过。

use serde::Deserialize;
use std::path::{Path, PathBuf};
use vasp_domain::{parse_oszicar, parse_outcar_forces};

#[derive(Deserialize)]
struct ParityManifest {
    fixtures: Vec<ParityFixture>,
}

#[derive(Deserialize)]
struct ParityFixture {
    name: String,
    directory: String,
    golden: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/vasp-cli 应该有两级父目录")
        .to_path_buf()
}

#[test]
fn convergence_matches_reference_golden() {
    let tests_dir = repo_root().join("tests");
    let manifest_path = tests_dir.join("parity-fixtures.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("读取对拍清单 {} 失败：{error}", manifest_path.display()));
    let manifest: ParityManifest = serde_json::from_str(&manifest_text).expect("对拍清单必须是 JSON");
    assert!(!manifest.fixtures.is_empty(), "对拍清单至少需要一个 fixture");

    for case in &manifest.fixtures {
        let fixture_dir = tests_dir.join(&case.directory);
        let golden_path = tests_dir.join(&case.golden);
        let golden_text = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|error| panic!("{}：读取 golden {} 失败：{error}；先运行 node scripts/gen-golden.mjs", case.name, golden_path.display()));
        let golden: serde_json::Value = serde_json::from_str(&golden_text)
            .unwrap_or_else(|error| panic!("{}：golden 不是有效 JSON：{error}", case.name));
        assert_eq!(
            golden["fixture"].as_str(),
            Some(case.name.as_str()),
            "{}：golden 与清单名称不一致",
            case.name
        );

        let oszicar_path = fixture_dir.join("OSZICAR");
        let outcar_path = fixture_dir.join("OUTCAR");
        let oszicar = std::fs::read_to_string(&oszicar_path)
            .unwrap_or_else(|error| panic!("{}：读取 {} 失败：{error}", case.name, oszicar_path.display()));
        let outcar = std::fs::read_to_string(&outcar_path)
            .unwrap_or_else(|error| panic!("{}：读取 {} 失败：{error}", case.name, outcar_path.display()));

        let steps = parse_oszicar(&oszicar);
        let expected_points = golden["ionic_points"]
            .as_array()
            .unwrap_or_else(|| panic!("{}：ionic_points 必须是数组", case.name));
        assert_eq!(
            steps.len(),
            expected_points.len(),
            "{}：离子步数量不一致",
            case.name
        );
        for (index, (got, want)) in steps.iter().zip(expected_points.iter()).enumerate() {
            let expected_index = want["step"]
                .as_u64()
                .unwrap_or_else(|| panic!("{}：第 {index} 个 golden 缺少 step", case.name));
            assert_eq!(
                u64::from(got.index),
                expected_index,
                "{}：第 {index} 个离子步编号不一致",
                case.name
            );
            let expected_energy = want["energy"]
                .as_f64()
                .unwrap_or_else(|| panic!("{}：第 {index} 个 golden 缺少 energy", case.name));
            let actual_energy = got.energy_ev.unwrap_or_else(|| {
                panic!("{}：第 {index} 步能量未被 Rust 解析", case.name)
            });
            assert!(
                (actual_energy - expected_energy).abs() < 1e-9,
                "{}：第 {index} 步能量 Rust={actual_energy}，JS={expected_energy}",
                case.name
            );
        }

        let forces = parse_outcar_forces(&outcar);
        let expected_forces = golden["max_forces"]
            .as_array()
            .unwrap_or_else(|| panic!("{}：max_forces 必须是数组", case.name));
        assert_eq!(
            forces.len(),
            expected_forces.len(),
            "{}：力段数量不一致",
            case.name
        );
        for (index, (actual, expected)) in forces.iter().zip(expected_forces.iter()).enumerate() {
            let expected = expected
                .as_f64()
                .unwrap_or_else(|| panic!("{}：第 {index} 个力值不是数值", case.name));
            assert!(
                (actual - expected).abs() < 1e-9,
                "{}：第 {index} 个力值 Rust={actual}，JS={expected}",
                case.name
            );
        }
    }

    eprintln!("对拍通过：{} 个 fixture", manifest.fixtures.len());
}
