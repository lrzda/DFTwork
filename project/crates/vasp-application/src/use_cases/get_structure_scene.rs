use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{parse_poscar, StructureScene};

/// 把 POSCAR / CONTCAR / *.vasp 读成 3D 场景。
pub struct GetStructureScene<'a> {
    fs: &'a dyn Fs,
}

impl<'a> GetStructureScene<'a> {
    pub fn new(fs: &'a dyn Fs) -> Self {
        Self { fs }
    }

    pub fn execute(&self, file: &Path) -> AppResult<StructureScene> {
        if !self.fs.exists(file) {
            return Err(AppError::NotFound(format!("文件不存在：{}", file.display())));
        }
        let text = self
            .fs
            .read_text(file, MAX_FULL_READ)
            .map_err(|e| AppError::io(file.display().to_string(), e))?;
        let scene = parse_poscar(&text)?;
        Ok(scene)
    }
}
