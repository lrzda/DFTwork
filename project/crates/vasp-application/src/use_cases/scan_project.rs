use crate::dto::{ScanReport, TaskSummary};
use crate::ports::{Fs, TAIL_PROBE_BYTES};
use crate::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::time::Instant;
use vasp_domain::{classify, detect, Clock, EventSink, ProgressEvent};

/// 扫描时跳过的目录名。这些永远不是 VASP 任务，进去只会拖慢扫描。
const SKIP_DIRS: [&str; 8] = [
    ".dftwork",
    ".git",
    "node_modules",
    "target",
    "dist",
    "out",
    "__pycache__",
    ".venv",
];

const DEFAULT_MAX_DEPTH: usize = 8;

/// 扫描项目根下所有 VASP 任务。
///
/// 走增量回调（[`EventSink`]），大目录可以先出前几十个再补——
/// 不是「扫完才返回」。
pub struct ScanProject<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
    max_depth: usize,
}

impl<'a> ScanProject<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self {
            fs,
            clock,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn execute(
        &self,
        root: &Path,
        sink: Option<&dyn EventSink>,
    ) -> AppResult<ScanReport> {
        let started = Instant::now();

        if !self.fs.exists(root) {
            return Err(AppError::NotFound(format!("项目根不存在：{}", root.display())));
        }

        let mut acc = Acc::default();
        self.walk(root, root, "", 0, &mut acc, sink)?;

        let observed_at = self.clock.now_iso();
        let tasks = acc
            .tasks
            .into_iter()
            .map(|(rel, inputs, outputs, record)| {
                TaskSummary::from_parts(
                    rel,
                    record.status,
                    record.reason,
                    record.evidence,
                    inputs,
                    outputs,
                    Some(observed_at.clone()),
                )
            })
            .collect::<Vec<_>>();

        Ok(ScanReport {
            root: root.display().to_string(),
            dirs_scanned: acc.dirs_scanned,
            tasks,
            pending_dirs: acc.pending_dirs,
            elapsed_ms: started.elapsed().as_millis(),
            schema_version: ScanReport::SCHEMA_VERSION,
        })
    }

    fn walk(
        &self,
        root: &Path,
        dir: &Path,
        rel: &str,
        depth: usize,
        acc: &mut Acc,
        sink: Option<&dyn EventSink>,
    ) -> AppResult<()> {
        if depth > self.max_depth {
            return Ok(());
        }

        let entries = match self.fs.list_dir(dir) {
            Ok(e) => e,
            // 单个目录读不了不该让整次扫描失败（权限、正在被写…）
            Err(_) => return Ok(()),
        };
        acc.dirs_scanned += 1;

        let files: Vec<String> = entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.name.clone())
            .collect();

        let abs: PathBuf = dir.to_path_buf();
        match detect(rel, abs, &files) {
            Some(task) => {
                let outcar = if task.outputs.outcar {
                    self.read_probe(&dir.join("OUTCAR"))
                } else {
                    None
                };
                let oszicar = if task.outputs.oszicar {
                    self.read_probe(&dir.join("OSZICAR"))
                } else {
                    None
                };
                let record = classify(&task.outputs, outcar.as_deref(), oszicar.as_deref());

                if let Some(s) = sink {
                    s.emit(&ProgressEvent::Discovered {
                        rel_path: task.rel_path.clone(),
                        is_task: true,
                    });
                }
                acc.tasks
                    .push((task.rel_path, task.inputs, task.outputs, record));
            }
            None => {
                // 只有输入、没有输出 → 未开始的项目，单独列出
                let (inputs, outputs) = vasp_domain::presence(&files);
                if inputs.poscar && inputs.incar && !outputs.any() && !rel.is_empty() {
                    acc.pending_dirs.push(rel.to_string());
                    if let Some(s) = sink {
                        s.emit(&ProgressEvent::Discovered {
                            rel_path: rel.to_string(),
                            is_task: false,
                        });
                    }
                }
            }
        }

        for e in entries.iter().filter(|e| e.is_dir) {
            if SKIP_DIRS.iter().any(|s| e.name.eq_ignore_ascii_case(s)) {
                continue;
            }
            let child_rel = if rel.is_empty() {
                e.name.clone()
            } else {
                format!("{rel}/{}", e.name)
            };
            let _ = root; // 保留参数，将来做「路径必须在根内」的二次校验
            self.walk(root, &dir.join(&e.name), &child_rel, depth + 1, acc, sink)?;
        }

        Ok(())
    }

    /// 读文件尾部；读不到就返回 `None`——状态判定会退化成「无法判断」，
    /// 但不会让整次扫描失败。
    fn read_probe(&self, path: &Path) -> Option<String> {
        self.fs.read_tail(path, TAIL_PROBE_BYTES).ok()
    }
}

#[derive(Default)]
struct Acc {
    dirs_scanned: usize,
    tasks: Vec<(
        String,
        vasp_domain::InputPresence,
        vasp_domain::OutputPresence,
        vasp_domain::TaskStatusRecord,
    )>,
    pending_dirs: Vec<String>,
}
