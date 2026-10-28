//! SCF 电子步、每原子力/应力、振动（频率 + 力常数）三个用例。
//!
//! 它们都读同一个任务目录里的输出文件，放一起便于对照；
//! 大文件（几百 MB / 几百 MB 的 OUTCAR）走流式扫描，不整读。

use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};
use vasp_domain::{
    parse_oszicar_scf, ForceReport, ForceStressScanner, ScfSeries, StressReport, VibrationReport,
    VibrationScanner,
};

/// 把「任务目录或文件」归一化成具体文件路径。
fn resolve(fs: &dyn Fs, task: &Path, name: &str) -> AppResult<PathBuf> {
    if fs.is_dir(task) {
        let candidate = task.join(name);
        if !fs.exists(&candidate) {
            return Err(AppError::NotFound(format!(
                "{} 里没有 {name}",
                task.display()
            )));
        }
        Ok(candidate)
    } else {
        Ok(task.to_path_buf())
    }
}

/// 读 OSZICAR，解析 SCF 电子步序列。
pub struct GetScf<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetScf<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, task: &Path) -> AppResult<ScfSeries> {
        let path = resolve(self.fs, task, "OSZICAR")?;
        // OSZICAR 是纯文本、体积可控（最长的 MD 也就几 MB），整读即可
        let text = self
            .fs
            .read_text(&path, MAX_FULL_READ)
            .map_err(|e| AppError::io(path.display().to_string(), e))?;
        Ok(parse_oszicar_scf(&text))
    }
}

/// 读 OUTCAR，同时取出每原子力与应力（一遍扫描拿两样）。
pub struct GetForces<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetForces<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, task: &Path) -> AppResult<(ForceReport, StressReport)> {
        let path = resolve(self.fs, task, "OUTCAR")?;
        let mut scanner = ForceStressScanner::new();
        self.fs.scan_lines(&path, &mut |line| {
            scanner.feed(line);
            true
        })?;
        Ok(scanner.finish())
    }
}

/// 读 OUTCAR，取出振动频率与力常数矩阵。
pub struct GetVibrations<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetVibrations<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, task: &Path) -> AppResult<VibrationReport> {
        let path = resolve(self.fs, task, "OUTCAR")?;
        let mut scanner = VibrationScanner::new();
        self.fs.scan_lines(&path, &mut |line| {
            scanner.feed(line);
            true
        })?;
        Ok(scanner.finish())
    }
}
