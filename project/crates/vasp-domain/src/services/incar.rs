//! INCAR 解析与计算类型判定。
//!
//! INCAR 是 `KEY = VALUE` 的纯文本，支持 `#` / `!` 注释和 `;` 分隔多个赋值。
//! 这里只做「取值」这一件事，**不做任何规则校验**——校验属于 input-checker，
//! 两者的区别是：这里回答「IBRION 是多少」，那里回答「这个 IBRION 合不合规矩」。

use crate::TaskKind;
use std::collections::HashMap;

/// 解析后的 INCAR。键统一大写，值保留原文（去首尾空白）。
#[derive(Debug, Clone, Default)]
pub struct Incar {
    keys: HashMap<String, String>,
}

impl Incar {
    /// 解析 INCAR 文本。坏行直接跳过——一个手滑的行不该让整份 INCAR 失效。
    pub fn parse(text: &str) -> Self {
        let mut keys = HashMap::new();
        for line in text.lines() {
            // 注释：`#` 与 `!` 都是 VASP 认可的行内注释起始符
            let body = line.split(['#', '!']).next().unwrap_or("");
            for part in body.split(';') {
                let Some((k, v)) = part.split_once('=') else {
                    continue;
                };
                let key = k.trim().to_uppercase();
                if key.is_empty() {
                    continue;
                }
                keys.insert(key, v.trim().to_string());
            }
        }
        Self { keys }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// 原始字符串值。
    pub fn raw(&self, key: &str) -> Option<&str> {
        self.keys.get(&key.to_uppercase()).map(|s| s.as_str())
    }

    /// 整数值。VASP 里 `IBRION = 2` 与 `IBRION = 2 ! comment` 都合法；
    /// 也允许 `2.0` 这种写法（有些脚本会这么写），所以先试整数再退化到浮点。
    pub fn int(&self, key: &str) -> Option<i64> {
        let raw = self.raw(key)?;
        if let Ok(v) = raw.parse::<i64>() {
            return Some(v);
        }
        raw.parse::<f64>().ok().map(|v| v as i64)
    }

    /// 浮点值。
    pub fn float(&self, key: &str) -> Option<f64> {
        self.raw(key)?.parse::<f64>().ok()
    }

    /// 逻辑值。VASP 接受 `.TRUE.` / `T` / `.T.` / `TRUE` 及对应的小写与 `.FALSE.`。
    pub fn flag(&self, key: &str) -> Option<bool> {
        let raw = self.raw(key)?.trim().trim_matches('.').to_uppercase();
        match raw.as_str() {
            "T" | "TRUE" => Some(true),
            "F" | "FALSE" => Some(false),
            _ => None,
        }
    }

    /// 已解析出的键值对（调试与校验用）。
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.keys.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// 由 INCAR 推断计算类型。
///
/// 判定顺序与 `reference/VASPFlow/scanner.js::detectTaskType` 一致：
/// NEB → 频率 → AIMD → 结构优化 → 静态。
/// **顺序有意义**：`IBRION=5` 配 `NSW=1` 是频率而不是「短路的结构优化」，
/// 所以频率必须排在优化前面。
pub fn detect_task_kind(incar: Option<&Incar>) -> TaskKind {
    let Some(incar) = incar else {
        return TaskKind::Unknown;
    };
    if incar.is_empty() {
        return TaskKind::Unknown;
    }

    if incar.int("IMAGES").unwrap_or(0) > 0 {
        return TaskKind::Neb;
    }

    // NSW 的 VASP 默认值是 0；IBRION 默认 -1
    let nsw = incar.int("NSW").unwrap_or(0);
    let ibrion = incar.int("IBRION").unwrap_or(-1);

    if matches!(ibrion, 5 | 6 | 7 | 8) {
        return TaskKind::Frequency;
    }
    if ibrion == 0 && nsw > 0 {
        return TaskKind::Aimd;
    }
    if matches!(ibrion, 1 | 2 | 3) && nsw > 0 {
        return TaskKind::Relaxation;
    }
    TaskKind::StaticScf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_assignment() {
        let incar = Incar::parse("IBRION = 5\nNSW = 1\n");
        assert_eq!(incar.int("IBRION"), Some(5));
        assert_eq!(incar.int("NSW"), Some(1));
    }

    #[test]
    fn strips_comments() {
        let incar = Incar::parse("ENCUT = 500   # plane wave cutoff\nISMEAR = 1 ! smearing\n");
        assert_eq!(incar.int("ENCUT"), Some(500));
        assert_eq!(incar.int("ISMEAR"), Some(1));
    }

    #[test]
    fn handles_semicolon_and_case() {
        let incar = Incar::parse("ibrion=2; nsw=100\n");
        assert_eq!(incar.int("IBRION"), Some(2));
        assert_eq!(incar.int("NSW"), Some(100));
    }

    #[test]
    fn parses_flags() {
        let incar = Incar::parse("LREAL = .FALSE.\nLWAVE = T\nLELF = .true.\n");
        assert_eq!(incar.flag("LREAL"), Some(false));
        assert_eq!(incar.flag("LWAVE"), Some(true));
        assert_eq!(incar.flag("LELF"), Some(true));
    }

    #[test]
    fn detects_kinds() {
        let f = |s: &str| detect_task_kind(Some(&Incar::parse(s)));
        assert_eq!(f("IBRION = 5\nNSW = 1"), TaskKind::Frequency);
        assert_eq!(f("IBRION = 8"), TaskKind::Frequency);
        assert_eq!(f("IBRION = 2\nNSW = 100"), TaskKind::Relaxation);
        assert_eq!(f("IBRION = 0\nNSW = 1000"), TaskKind::Aimd);
        assert_eq!(f("IBRION = -1\nNSW = 0"), TaskKind::StaticScf);
        // 只写 IBRION 没写 NSW 时，NSW 按 VASP 默认 0 处理 → 静态
        assert_eq!(f("IBRION = 1"), TaskKind::StaticScf);
    }

    #[test]
    fn neb_takes_priority() {
        // 即使 IBRION 是 3，只要有 IMAGES 就是 NEB
        let k = detect_task_kind(Some(&Incar::parse("IMAGES = 4\nIBRION = 3\nNSW = 50")));
        assert_eq!(k, TaskKind::Neb);
    }

    #[test]
    fn missing_incar_is_unknown() {
        assert_eq!(detect_task_kind(None), TaskKind::Unknown);
        assert_eq!(detect_task_kind(Some(&Incar::default())), TaskKind::Unknown);
    }
}
