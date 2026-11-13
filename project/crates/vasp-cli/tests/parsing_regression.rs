use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "dftwork-parsing-{}-{name}-{id}",
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

fn failure_details(output: &Output) -> String {
    format!(
        "status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn convergence_reads_force_summary_beyond_full_read_limit() {
    let temp = TestDir::new("large-outcar");
    let task = temp.path().join("task");
    std::fs::create_dir_all(&task).unwrap();
    std::fs::write(task.join("OSZICAR"), "1 F= -10.0 E0= -10.0\n").unwrap();
    let mut outcar = std::io::BufWriter::new(
        std::fs::File::create(task.join("OUTCAR")).expect("创建大 OUTCAR"),
    );
    let padding = " ignored convergence padding\n".repeat(4096);
    for _ in 0..(65 * 1024 * 1024 / padding.len()) {
        outcar.write_all(padding.as_bytes()).expect("写入 OUTCAR");
    }
    outcar
        .write_all(b"FORCES: max atom, RMS     0.125000   0.045678\n")
        .expect("写入力汇总");
    outcar.flush().expect("刷新 OUTCAR");
    drop(outcar);

    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("convergence")
        .arg(&task)
        .output()
        .expect("运行 convergence");
    assert!(output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("解析 JSON");
    assert_eq!(envelope["data"]["steps"][0]["max_force_ev_per_a"], 0.125);
}

#[test]
fn trajectory_counts_and_samples_the_complete_file() {
    let temp = TestDir::new("long-trajectory");
    let task = temp.path().join("task");
    std::fs::create_dir_all(&task).unwrap();
    let mut xdatcar = String::from(
        "demo\n2.0\n10 0 0\n0 10 0\n0 0 10\nSi\n1\n",
    );
    for index in 1..=2501 {
        xdatcar.push_str(&format!("Cartesian configuration= {index}\n1 2 3\n"));
    }
    std::fs::write(task.join("XDATCAR"), xdatcar).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vaspcore"))
        .arg("trajectory")
        .arg(&task)
        .output()
        .expect("运行 trajectory");
    assert!(output.status.success(), "{}", failure_details(&output));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("解析 JSON");
    let report = &envelope["data"];
    assert_eq!(report["total_frames"], 2501);
    assert_eq!(report["sampled"], true);
    assert_eq!(report["frames"].as_array().unwrap().len(), 600);
    assert_eq!(report["frames"][0]["positions"][0], serde_json::json!([2.0, 4.0, 6.0]));
    assert_eq!(report["frames"][599]["index"], 2501);
}
