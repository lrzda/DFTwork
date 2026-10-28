use crate::ports::Fs;
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{MagnetizationReport, MagnetizationScanner};

/// 从任务的 OUTCAR 里提取磁矩（逐离子步的总磁矩）。
///
/// 走流式扫描：`某个大体系` 的 OUTCAR 有 几百 MB、`某 MD 轨迹` 有 几百 MB，
/// 既不能整读也不能只读尾部（磁矩分布在整个文件里）。
pub struct GetMagnetization<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetMagnetization<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    /// `task_dir` 可以是任务目录，也可以直接给 OUTCAR 的路径。
    pub fn execute(&self, task_dir: &Path) -> AppResult<MagnetizationReport> {
        let outcar = if self.fs.is_dir(task_dir) {
            let candidate = task_dir.join("OUTCAR");
            if !self.fs.exists(&candidate) {
                return Err(AppError::NotFound(format!(
                    "{} 里没有 OUTCAR",
                    task_dir.display()
                )));
            }
            candidate
        } else {
            task_dir.to_path_buf()
        };

        let mut scanner = MagnetizationScanner::new();
        // 闭包恒返回 true：磁矩贯穿整个文件，没有可以提前退出的点
        let lines_scanned = self
            .fs
            .scan_lines(&outcar, &mut |line| {
                scanner.feed(line);
                true
            })?;

        let (steps, per_atom_available, note) = scanner.finish();
        Ok(MagnetizationReport {
            steps,
            per_atom_available,
            note: format!("{note}（扫描 {lines_scanned} 行）"),
        })
    }
}
