use crate::ports::Fs;
use crate::{AppError, AppResult};
use std::path::Path;
use vasp_domain::{merge_series, ConvergenceSeries, OszicarScanner, OutcarForceScanner};

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

        let mut oszicar = OszicarScanner::new();
        self.fs.scan_lines(&oszicar_path, &mut |line| {
            oszicar.feed(line);
            true
        })?;
        let energies = oszicar.finish();

        // 只有 OUTCAR 不存在时力才是可选空值；存在但读取失败必须报错。
        let outcar_path = task_dir.join("OUTCAR");
        let forces = if self.fs.exists(&outcar_path) {
            let mut outcar = OutcarForceScanner::new();
            self.fs.scan_lines(&outcar_path, &mut |line| {
                outcar.feed(line);
                true
            })?;
            outcar.finish()
        } else {
            Vec::new()
        };

        Ok(merge_series("OSZICAR", energies, forces))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::DirEntryInfo;
    use std::path::PathBuf;

    struct OutcarReadFailure;

    impl Fs for OutcarReadFailure {
        fn list_dir(&self, _: &Path) -> Result<Vec<DirEntryInfo>, AppError> {
            unreachable!()
        }

        fn read_text(&self, path: &Path, _: u64) -> Result<String, AppError> {
            if path.ends_with("OUTCAR") {
                Err(AppError::io(
                    path.display().to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, "read failed"),
                ))
            } else {
                Ok("1 F= -10.0 E0= -10.0\n".into())
            }
        }

        fn read_tail(&self, _: &Path, _: u64) -> Result<String, AppError> {
            unreachable!()
        }

        fn scan_lines(
            &self,
            path: &Path,
            visit: &mut dyn FnMut(&str) -> bool,
        ) -> Result<usize, AppError> {
            if path.ends_with("OUTCAR") {
                Err(AppError::io(
                    path.display().to_string(),
                    std::io::Error::new(std::io::ErrorKind::Other, "read failed"),
                ))
            } else {
                visit("1 F= -10.0 E0= -10.0");
                Ok(1)
            }
        }

        fn exists(&self, _: &Path) -> bool {
            true
        }

        fn canonicalize(&self, path: &Path) -> Result<PathBuf, AppError> {
            Ok(path.into())
        }

        fn is_symlink(&self, _: &Path) -> bool {
            false
        }

        fn is_dir(&self, _: &Path) -> bool {
            true
        }

        fn write_text(&self, _: &Path, _: &str) -> Result<(), AppError> {
            unreachable!()
        }

        fn remove_file(&self, _: &Path) -> Result<(), AppError> {
            unreachable!()
        }

        fn remove_dir_all(&self, _: &Path) -> Result<(), AppError> {
            unreachable!()
        }
    }

    #[test]
    fn outcar_read_failure_is_not_an_empty_success() {
        let result = GetConvergence::new(&OutcarReadFailure).execute(Path::new("task"));

        assert!(result.is_err(), "OUTCAR 读取失败被吞成了空力数组");
    }
}
