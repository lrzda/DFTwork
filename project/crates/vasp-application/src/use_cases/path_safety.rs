use crate::ports::Fs;
use crate::{AppError, AppResult};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub(crate) fn canonical_root(fs: &dyn Fs, root: &Path) -> AppResult<PathBuf> {
    let canonical = fs.canonicalize(root)?;
    if !fs.is_dir(&canonical) {
        return Err(AppError::Validation(format!(
            "项目根不是目录：{}",
            root.display()
        )));
    }
    Ok(canonical)
}

pub(crate) fn reject_duplicates<'a>(paths: impl Iterator<Item = &'a str>) -> AppResult<()> {
    let mut seen = HashSet::new();
    for path in paths {
        let normalized = path.replace('\\', "/");
        let key = if cfg!(windows) {
            normalized.to_lowercase()
        } else {
            normalized
        };
        if !seen.insert(key) {
            return Err(AppError::Validation(format!(
                "计划或回执包含重复路径：{path}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn business_target_path(fs: &dyn Fs, root: &Path, rel: &str) -> AppResult<PathBuf> {
    if rel
        .replace('\\', "/")
        .split('/')
        .next()
        .is_some_and(|part| part.eq_ignore_ascii_case(".dftwork"))
    {
        return Err(AppError::Validation(
            ".dftwork 是软件状态与恢复数据保留目录，不能作为业务写入目标".into(),
        ));
    }
    target_path(fs, root, rel)
}

pub(crate) fn target_path(fs: &dyn Fs, root: &Path, rel: &str) -> AppResult<PathBuf> {
    if !is_safe_rel_path(rel) {
        return Err(AppError::Validation(format!(
            "路径不安全（为空、含 .. / . 或是绝对路径）：{rel}"
        )));
    }

    let target = root.join(rel);
    let mut current = root.to_path_buf();
    for component in Path::new(rel).components() {
        current.push(component);
        if fs.is_symlink(&current) {
            return Err(AppError::Validation(format!(
                "路径不能经过符号链接或目录联接：{rel}"
            )));
        }
    }

    let mut existing = target.clone();
    while !fs.exists(&existing) {
        existing = existing
            .parent()
            .ok_or_else(|| AppError::Validation(format!("路径没有已存在的父目录：{rel}")))?
            .to_path_buf();
    }
    let resolved = fs.canonicalize(&existing)?;
    if !resolved.starts_with(root) {
        return Err(AppError::Validation(format!(
            "真实路径越过项目根：{rel}"
        )));
    }
    if existing != target && !fs.is_dir(&existing) {
        return Err(AppError::Validation(format!(
            "目标父路径不是目录：{rel}"
        )));
    }
    Ok(target)
}

fn is_safe_rel_path(rel: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    let path = Path::new(rel);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::is_safe_rel_path;

    #[test]
    fn rejects_escape_paths() {
        assert!(is_safe_rel_path("task-1/INCAR"));
        assert!(is_safe_rel_path("INCAR"));
        assert!(!is_safe_rel_path("../outside/INCAR"));
        assert!(!is_safe_rel_path("a/../../b"));
        assert!(!is_safe_rel_path("C:/Windows/system32/x"));
        assert!(!is_safe_rel_path("/etc/passwd"));
        assert!(!is_safe_rel_path(""));
    }
}
