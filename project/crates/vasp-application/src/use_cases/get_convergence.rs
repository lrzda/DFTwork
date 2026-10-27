use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{merge_series, parse_oszicar, parse_outcar_forces, ConvergenceSeries};

/// 读一个任务的收敛序列（离子步能量 + 最大力）。
pub struct GetConvergence<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetConvergence<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, task_dir: &Path) -> AppResult<ConvergenceSeries> {
        let oszicar_path = task_dir.join("OSZICAR");
        if !self.fs.exists(&oszicar_path) {
            return Err(AppError::NotFound(format!(
                "任务目录里没有 OSZICAR：{}",
                task_dir.display()
            )));
        }

        let oszicar = self
            .fs
            .read_text(&oszicar_path, MAX_FULL_READ)
            .map_err(|e| AppError::io(oszicar_path.display().to_string(), e))?;
        let energies = parse_oszicar(&oszicar);

        // 力是可选的：拿不到就留 None，曲线只画能量，不补 0。
        let outcar_path = task_dir.join("OUTCAR");
        let forces = if self.fs.exists(&outcar_path) {
            match self.fs.read_text(&outcar_path, MAX_FULL_READ) {
                Ok(text) => parse_outcar_forces(&text),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(merge_series("OSZICAR", energies, forces))
    }
}
