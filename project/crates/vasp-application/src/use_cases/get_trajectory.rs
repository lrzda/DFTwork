//! 读取一个任务的 XDATCAR，转成可播放的轨迹。

use std::path::Path;

use vasp_domain::{TrajectoryReport, TrajectoryScanner};

use crate::ports::Fs;
use crate::{AppError, AppResult};

/// 轨迹文件名。VASP 固定写 `XDATCAR`，没有候选。
pub const TRAJECTORY_FILE: &str = "XDATCAR";

pub struct GetTrajectory<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetTrajectory<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    /// `task_dir` 是任务目录（不是 XDATCAR 路径）。
    pub fn execute(&self, task_dir: &Path) -> AppResult<TrajectoryReport> {
        let path = task_dir.join(TRAJECTORY_FILE);
        if !self.fs.exists(&path) {
            return Err(AppError::NotFound(format!(
                "{} 不存在（只有运行过的任务才有轨迹）",
                TRAJECTORY_FILE
            )));
        }

        let mut scanner = TrajectoryScanner::new();
        let mut scan_err: Option<AppError> = None;
        // 流式：XDATCAR 可能几十 MB，不能整读
        if let Err(e) = self.fs.scan_lines(&path, &mut |line| scanner.feed(line)) {
            scan_err = Some(e);
        }
        if let Some(e) = scan_err {
            return Err(e);
        }

        let mut report = scanner.finish().map_err(|e| AppError::Validation(e.to_string()))?;
        report.source_file = path.to_string_lossy().to_string();
        Ok(report)
    }
}
