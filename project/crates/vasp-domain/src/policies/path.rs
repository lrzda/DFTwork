use crate::{DomainError, DomainResult};

/// 统一分隔符并去掉空段与 `./`。**不做**真实路径解析——
/// 领域层不碰文件系统，因此也不调用 `canonicalize`。
pub fn normalize_rel(p: &str) -> String {
    let s = p.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "" | "." => continue,
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// 绝对路径判定。注意必须在 [`normalize_rel`] **之前**调用，
/// 因为归一化会把开头的 `/` 丢掉。
pub fn is_absolute(p: &str) -> bool {
    let s = p.replace('\\', "/");
    if s.starts_with('/') {
        return true;
    }
    // Windows 盘符：`C:` / `C:/` / `C:\`
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// 目标路径必须在项目根内。
///
/// 拒绝三种情况：空路径、含 `..` 的路径、落在根之外的绝对路径。
/// 绝对路径若确实在根内，会被换算成相对路径返回。
pub fn assert_inside_project(root: &str, candidate: &str) -> DomainResult<String> {
    let was_absolute = is_absolute(candidate);
    let rel = normalize_rel(candidate);

    if rel.is_empty() {
        return Err(DomainError::InvalidPath(candidate.to_string()));
    }
    if rel.split('/').any(|s| s == "..") {
        return Err(DomainError::PathOutsideProject(candidate.to_string()));
    }
    if !was_absolute {
        return Ok(rel);
    }

    let root_n = normalize_rel(root);
    if root_n.is_empty() {
        return Err(DomainError::PathOutsideProject(candidate.to_string()));
    }
    // Windows 盘符大小写不敏感
    let lower_rel = rel.to_lowercase();
    let lower_root = root_n.to_lowercase();
    let prefix = format!("{lower_root}/");
    if lower_rel.starts_with(&prefix) {
        Ok(rel[prefix.len()..].to_string())
    } else {
        Err(DomainError::PathOutsideProject(candidate.to_string()))
    }
}

/// 把相对路径拼到根上（用于生成回执、日志里的可读路径）。
pub fn join_rel(root: &str, rel: &str) -> String {
    let r = normalize_rel(root);
    let s = normalize_rel(rel);
    if r.is_empty() {
        s
    } else if s.is_empty() {
        r
    } else {
        format!("{r}/{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_segments() {
        assert!(assert_inside_project("/p/root", "../evil").is_err());
        assert!(assert_inside_project("/p/root", "a/../../b").is_err());
    }

    #[test]
    fn accepts_relative_and_normalizes() {
        assert_eq!(
            assert_inside_project("/p/root", "a\\b/./c").unwrap(),
            "a/b/c"
        );
    }

    #[test]
    fn absolute_inside_root_becomes_relative() {
        assert_eq!(
            assert_inside_project("D:/proj", "D:\\proj\\task1\\INCAR").unwrap(),
            "task1/INCAR"
        );
    }

    #[test]
    fn absolute_outside_root_is_rejected() {
        assert!(assert_inside_project("D:/proj", "C:/Windows/system32/x").is_err());
    }
}
