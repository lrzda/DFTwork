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

        // 第一遍统计真实总帧数，第二遍只保留计划中的样本；两遍都不整读文件。
        let total_frames = self.scan(&path, TrajectoryScanner::new())?.total_frames;
        let mut report = self.scan(&path, TrajectoryScanner::sampled(total_frames))?;
        report.source_file = path.to_string_lossy().to_string();
        Ok(report)
    }

    fn scan(
        &self,
        path: &Path,
        mut scanner: TrajectoryScanner,
    ) -> AppResult<TrajectoryReport> {
        self.fs
            .scan_lines(path, &mut |line| scanner.feed(line))?;
        scanner.finish().map_err(AppError::from)
    }
}
