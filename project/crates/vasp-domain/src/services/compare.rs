//! 对比多个任务的 INCAR。
//!
//! 这里是纯函数：只做「表格对齐 + 求差异」，不读文件。
//! 读文件由用例层做，便于对 diff 逻辑单独写测试。

use crate::Incar;
use std::collections::BTreeSet;

/// 一项参数在各任务里的取值，以及是否完全相同。
pub struct DiffRow {
    pub key: String,
    pub values: Vec<Option<String>>,
    pub same: bool,
}

/// 把多份 INCAR 对齐成一张表，返回（差异行, 相同项数）。
///
/// - 用 `BTreeSet` 收集全部键再排序：INCAR 是人为书写的，顺序毫无意义，
///   按字母序排才能横向比对。
/// - 值按**原文**比较（不去空格、不规范化大小写）：`ENCUT = 500` 和 `ENCUT = 500.0`
///   在 VASP 眼里等价，但这里如实显示差异，让用户自己判断。
pub fn diff_incars(incars: &[Incar]) -> (Vec<DiffRow>, usize) {
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    for incar in incars {
        for (k, _) in incar.entries() {
            keys.insert(k);
        }
    }

    let mut rows = Vec::new();
    let mut same_count = 0usize;

    for key in keys {
        let values: Vec<Option<String>> = incars
            .iter()
            .map(|i| i.raw(&key).map(|s| s.to_string()))
            .collect();

        // 全同判定：所有值（含 None）逐项相等
        let same = values
            .iter()
            .all(|v| v.as_deref() == values[0].as_deref());

        if same {
            same_count += 1;
        } else {
            rows.push(DiffRow {
                key: key.to_string(),
                values,
                same,
            });
        }
    }

    (rows, same_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_changed_value() {
        let a = Incar::parse("ENCUT = 500\nISMEAR = 1\n");
        let b = Incar::parse("ENCUT = 520\nISMEAR = 1\n");
        let (rows, same) = diff_incars(&[a, b]);
        assert_eq!(same, 1, "ISMEAR 相同");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "ENCUT");
        assert_eq!(rows[0].values[0].as_deref(), Some("500"));
        assert_eq!(rows[0].values[1].as_deref(), Some("520"));
    }

    #[test]
    fn missing_key_is_a_difference() {
        let a = Incar::parse("ENCUT = 500\nLREAL = .FALSE.\n");
        let b = Incar::parse("ENCUT = 500\n");
        let (rows, same) = diff_incars(&[a, b]);
        assert_eq!(same, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "LREAL");
        assert!(rows[0].values[1].is_none(), "缺失要显示成 None 而不是空串");
    }

    #[test]
    fn identical_inputs_yield_no_rows() {
        let a = Incar::parse("ENCUT = 500\n");
        let b = Incar::parse("ENCUT = 500\n");
        let (rows, same) = diff_incars(&[a, b]);
        assert!(rows.is_empty());
        assert_eq!(same, 1);
    }

    #[test]
    fn rows_are_sorted() {
        let a = Incar::parse("NSW = 10\nENCUT = 500\nIBRION = 2\n");
        let b = Incar::parse("NSW = 20\nENCUT = 600\nIBRION = 5\n");
        let (rows, _) = diff_incars(&[a, b]);
        let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["ENCUT", "IBRION", "NSW"]);
    }
}
