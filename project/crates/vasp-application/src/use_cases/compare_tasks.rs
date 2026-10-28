//! 对比两个以上任务的 INCAR 与收敛序列。

use crate::ports::{Fs, MAX_FULL_READ};
use crate::use_cases::GetConvergence;
use crate::{AppError, AppResult};
use std::path::PathBuf;
use vasp_domain::{diff_incars, CompareReport, Incar, InputDiff};

/// 对比多个任务。
///
/// INCAR 用纯函数 [`diff_incars`] 对齐求差异；收敛序列复用
/// [`GetConvergence`]，保证「对比里看到的曲线」与「单看一个任务时的曲线」
/// 完全同源——两套解析迟早会飘。
pub struct CompareTasks<'a> {
    fs: &'a dyn Fs,
}

impl<'a> CompareTasks<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, tasks: &[PathBuf]) -> AppResult<CompareReport> {
        if tasks.len() < 2 {
            return Err(AppError::Validation(
                "对比至少需要两个任务".to_string(),
            ));
        }

        let mut names = Vec::with_capacity(tasks.len());
        let mut incars = Vec::with_capacity(tasks.len());
        let mut series = Vec::with_capacity(tasks.len());

        for task in tasks {
            let name = task
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| task.display().to_string());
            names.push(name);

            // INCAR 缺失不该让对比失败——它本身就是一条差异（「这个任务没 INCAR」）
            let incar = self
                .fs
                .read_text(&task.join("INCAR"), MAX_FULL_READ)
                .map(|t| Incar::parse(&t))
                .unwrap_or_default();
            incars.push(incar);

            series.push(GetConvergence::new(self.fs).execute(task)?);
        }

        let (rows, same_count) = diff_incars(&incars);
        let input_diff: Vec<InputDiff> = rows
            .into_iter()
            .map(|r| InputDiff {
                key: r.key,
                values: r.values,
            })
            .collect();

        Ok(CompareReport {
            tasks: names,
            input_diff,
            same_count,
            series,
        })
    }
}
