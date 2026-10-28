//! 从已有任务生成一份新的输入计划。
//!
//! 只产出计划，**不落盘**——落盘是 [`ApplyPlan`](crate::ApplyPlan) 的事。
//! 这样「生成」可以反复调用、可以丢弃，用户看到 diff 之前磁盘上什么都没发生。

use crate::ports::{Fs, MAX_FULL_READ};
use crate::{AppError, AppResult};
use std::collections::HashSet;
use std::path::PathBuf;
use vasp_domain::{plan_hash, BuildItem, BuildPlan, Clock, WriteAction};

/// 默认复制的文件。
///
/// **不含 POTCAR**：它受 VASP 许可保护且单份可达几十 MB，而 `BuildItem.content`
/// 是完整文件内容——塞进计划会让 JSON 膨胀到不可用。用户自己处理 POTCAR
/// （同一项目内通常已有，或硬链接）。
pub const DEFAULT_FILES: [&str; 3] = ["INCAR", "POSCAR", "KPOINTS"];

#[derive(Debug, Clone)]
pub struct BuildRequest {
    pub root: PathBuf,
    /// 源任务，相对项目根。
    pub source_rel: String,
    /// 目标任务目录，相对项目根。
    pub target_rel: String,
    /// INCAR 参数覆盖，`(KEY, VALUE)`。
    pub overrides: Vec<(String, String)>,
    /// 要复制的文件名；留空用 [`DEFAULT_FILES`]。
    pub files: Vec<String>,
}

pub struct BuildInputs<'a> {
    fs: &'a dyn Fs,
    clock: &'a dyn Clock,
}

impl<'a> BuildInputs<'a> {
    pub fn new(fs: &'a dyn Fs, clock: &'a dyn Clock) -> Self {
        Self { fs, clock }
    }

    pub fn execute(&self, req: &BuildRequest) -> AppResult<BuildPlan> {
        if req.target_rel.trim().is_empty() {
            return Err(AppError::Validation("目标目录不能为空".to_string()));
        }
        if req.target_rel == req.source_rel {
            // 写自己会把源文件改掉，而计划里的内容正是从它读的——纯自噬
            return Err(AppError::Validation(
                "目标任务不能与源任务相同".to_string(),
            ));
        }

        let src_dir = req.root.join(&req.source_rel);
        if !self.fs.is_dir(&src_dir) {
            return Err(AppError::NotFound(format!(
                "源任务目录不存在：{}",
                src_dir.display()
            )));
        }

        let names = if req.files.is_empty() {
            DEFAULT_FILES.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        } else {
            req.files.clone()
        };

        let mut items = Vec::new();
        for name in &names {
            let src = src_dir.join(name);
            if !self.fs.exists(&src) {
                continue;
            }

            let mut content = self.fs.read_text(&src, MAX_FULL_READ)?;
            let mut reason = format!("复制自 {}", req.source_rel);

            if name.eq_ignore_ascii_case("INCAR") && !req.overrides.is_empty() {
                content = apply_overrides(&content, &req.overrides);
                let changed: Vec<String> = req
                    .overrides
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                reason = format!("复制自 {}，改动 {}", req.source_rel, changed.join(" "));
            }

            let rel_path = format!("{}/{}", req.target_rel.trim_end_matches('/'), name);
            let target = req.root.join(&rel_path);
            let (action, previous_content) = if self.fs.exists(&target) {
                let old = self.fs.read_text(&target, MAX_FULL_READ)?;
                if old == content {
                    (WriteAction::Skip, Some(old))
                } else {
                    (WriteAction::Overwrite, Some(old))
                }
            } else {
                (WriteAction::Create, None)
            };

            items.push(BuildItem {
                rel_path,
                content,
                action,
                reason,
                previous_content,
            });
        }

        if items.is_empty() {
            return Err(AppError::NotFound(
                "源任务里没有可复制的输入文件".to_string(),
            ));
        }

        let hash = plan_hash::compute(&items);
        Ok(BuildPlan {
            root: req.root.display().to_string(),
            items,
            plan_hash: hash,
            created_at: self.clock.now_iso(),
        })
    }
}

/// 在 INCAR 文本里替换/追加参数。
///
/// 保持原有行序与注释：VASP 的 INCAR 常带说明性注释，重排会让 diff 变成一团噪音，
/// 用户就看不出「到底改了什么」——而 diff 可读正是放行环节的意义。
pub fn apply_overrides(incar: &str, overrides: &[(String, String)]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for line in incar.lines() {
        if let Some(key) = key_of(line) {
            if let Some((k, v)) = overrides
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&key))
            {
                lines.push(format!("{k} = {v}"));
                seen.insert(k.to_uppercase());
                continue;
            }
        }
        lines.push(line.to_string());
    }

    for (k, v) in overrides {
        if !seen.contains(&k.to_uppercase()) {
            lines.push(format!("{k} = {v}"));
        }
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 取 INCAR 一行的参数名：跳过注释（`#` / `!`），按 `=` 或空白切。
fn key_of(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
        return None;
    }
    let head = trimmed.split(['=', ' ', '\t']).next()?.trim();
    if head.is_empty() || !head.chars().next()?.is_alphabetic() {
        return None;
    }
    Some(head.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_key_in_place() {
        let incar = "ENCUT = 400\nISMEAR = 0\n";
        let out = apply_overrides(incar, &[("ENCUT".into(), "520".into())]);
        assert_eq!(out, "ENCUT = 520\nISMEAR = 0\n");
    }

    #[test]
    fn appends_missing_key() {
        let out = apply_overrides("ENCUT = 400\n", &[("SIGMA".into(), "0.1".into())]);
        assert_eq!(out, "ENCUT = 400\nSIGMA = 0.1\n");
    }

    #[test]
    fn keeps_comments_and_order() {
        let incar = "# 结构优化\nENCUT = 400\n! 下面这行别动\nISMEAR = 0\n";
        let out = apply_overrides(incar, &[("ENCUT".into(), "520".into())]);
        // 注释与行序必须原样保留，否则 diff 会变成一堆无关噪音
        assert_eq!(out, "# 结构优化\nENCUT = 520\n! 下面这行别动\nISMEAR = 0\n");
    }

    #[test]
    fn case_insensitive_match_preserves_written_form() {
        let out = apply_overrides("encut = 400\n", &[("ENCUT".into(), "520".into())]);
        assert_eq!(out, "ENCUT = 520\n");
    }

    #[test]
    fn multi_value_line_replaced_whole() {
        // LDAUU 这种多值参数整行替换，不做逐项合并
        let out = apply_overrides("LDAUU = 0 4.0 0\n", &[("LDAUU".into(), "0 5.0 0".into())]);
        assert_eq!(out, "LDAUU = 0 5.0 0\n");
    }

    #[test]
    fn trailing_newline_added() {
        let out = apply_overrides("ENCUT = 400", &[]);
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn comment_lines_are_not_keys() {
        assert_eq!(key_of("# ENCUT = 400"), None);
        assert_eq!(key_of("! ENCUT = 400"), None);
        assert_eq!(key_of(""), None);
        assert_eq!(key_of(" = 400"), None);
    }
}
