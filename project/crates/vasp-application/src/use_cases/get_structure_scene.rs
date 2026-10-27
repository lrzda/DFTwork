use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{parse_poscar_with, BondMode, StructureScene};

/// 结构文件按优先级尝试的顺序。
///
/// 任务目录里不一定有 POSCAR：跑过续算的任务常常只剩 CONTCAR，
/// 而优化没结束时 POSCAR 才是最新的输入。先查 POSCAR，没有再退 CONTCAR。
const STRUCTURE_CANDIDATES: [&str; 2] = ["POSCAR", "CONTCAR"];

/// 把 POSCAR / CONTCAR / *.vasp 读成 3D 场景。
pub struct GetStructureScene<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetStructureScene<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    /// `path` 可以是结构文件本身，也可以是任务目录（自动挑 POSCAR / CONTCAR）。
    ///
    /// `mode` 决定成键是否考虑周期边界——见 [`BondMode`]。
    pub fn execute(&self, path: &Path, mode: BondMode) -> AppResult<StructureScene> {
        let file = self.resolve(path)?;
        let text = self
            .fs
            .read_text(&file, MAX_FULL_READ)
            .map_err(|e| AppError::io(file.display().to_string(), e))?;
        let mut scene = parse_poscar_with(&text, mode)?;
        scene.source_file = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        Ok(scene)
    }

    /// 入参归一化：目录 → 具体文件。
    fn resolve(&self, path: &Path) -> AppResult<std::path::PathBuf> {
        if !self.fs.exists(path) {
            return Err(AppError::NotFound(format!("路径不存在：{}", path.display())));
        }
        if !self.fs.is_dir(path) {
            return Ok(path.to_path_buf());
        }
        for name in STRUCTURE_CANDIDATES {
            let candidate = path.join(name);
            if self.fs.exists(&candidate) {
                return Ok(candidate);
            }
        }
        Err(AppError::NotFound(format!(
            "{} 里既没有 POSCAR 也没有 CONTCAR",
            path.display()
        )))
    }
}
