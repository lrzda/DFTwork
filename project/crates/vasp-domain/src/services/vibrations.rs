//! 从 OUTCAR 提取振动频率与力常数矩阵。
//!
//! 两段数据都在文件中部（频率块前面可能有几十万行），所以必须流式扫描。

use crate::{HessianMatrix, VibrationReport, VibrationalMode};
use regex::Regex;
use std::sync::OnceLock;

/// 频率行：`   1 f  =    5.432100 THz    34.132685 2PiTHz  181.222123 cm-1    22.465903 meV`
///
/// 捕获组：序号 / 是否虚频 / THz / 2PiTHz / cm-1 / meV
fn mode_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*(\d+)\s+f(/i)?\s*=\s*([-+]?\d*\.?\d+)\s+THz\s+([-+]?\d*\.?\d+)\s+2PiTHz\s+([-+]?\d*\.?\d+)\s+cm-1\s+([-+]?\d*\.?\d+)\s+meV",
        )
        .expect("频率正则")
    })
}

fn hessian_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"SECOND DERIVATIVES").expect("Hessian 表头正则"))
}

/// Hessian 数据行：行标签（如 `1X`）+ 一串数。
/// 列头行（`1X  1Y  1Z ...`）的标签后面不是数字，所以不会被误匹配。
fn hessian_row_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*\d+[XYZxyz]\s+([-+]?\d+\.\d+)").expect("Hessian 行正则"))
}

/// 频率 + Hessian 的联合扫描器。
#[derive(Debug, Default)]
pub struct VibrationScanner {
    modes: Vec<VibrationalMode>,
    /// 是否处在 Hessian 数据段
    in_hessian: bool,
    hessian: Vec<f64>,
    hessian_rows: usize,
}

impl VibrationScanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, line: &str) {
        if let Some(c) = mode_re().captures(line) {
            let imaginary = c.get(2).is_some();
            let sign = if imaginary { -1.0 } else { 1.0 };
            let thz: f64 = c[3].parse().unwrap_or(f64::NAN);
            self.modes.push(VibrationalMode {
                index: c[1].parse().unwrap_or(0),
                frequency_thz: sign * thz,
                is_imaginary: imaginary,
                // 虚频的 cm⁻¹ 也取负，保证两列符号一致
                frequency_cm1: sign * c[5].parse::<f64>().unwrap_or(f64::NAN),
                energy_mev: c[6].parse().unwrap_or(f64::NAN),
            });
            return;
        }

        if hessian_header_re().is_match(line) {
            self.in_hessian = true;
            self.hessian.clear();
            self.hessian_rows = 0;
            return;
        }

        if self.in_hessian {
            if hessian_row_re().is_match(line) {
                // 一行是矩阵的一行：按空白切分，丢掉行标签
                let mut it = line.split_whitespace();
                it.next(); // 行标签 1X
                for token in it {
                    if let Ok(v) = token.parse::<f64>() {
                        self.hessian.push(v);
                    }
                }
                self.hessian_rows += 1;
                return;
            }
            // 遇到非数据行：Hessian 段结束。空行不算结束（VASP 有时会插空行）。
            if !line.trim().is_empty() && self.hessian_rows > 0 {
                self.in_hessian = false;
            }
        }
    }

    pub fn finish(self) -> VibrationReport {
        let imaginary_count = self.modes.iter().filter(|m| m.is_imaginary).count();

        let hessian = if self.hessian_rows > 0 {
            let n_dof = self.hessian_rows;
            let expected = n_dof * n_dof;
            let actual = self.hessian.len();
            // 完整性校验：矩阵必须是方阵。不完整的宁可不要，
            // 画出来会是一张错位的热图，比没有更糟。
            if actual == expected {
                Some(HessianMatrix {
                    n_dof,
                    data: self.hessian,
                })
            } else {
                None
            }
        } else {
            None
        };

        let note = if self.modes.is_empty() {
            "没有频率输出：这个任务不是频率计算（IBRION 需为 5/6/7/8）".to_string()
        } else if imaginary_count > 0 {
            format!(
                "{} 个振动模式，其中 {imaginary_count} 个虚频——结构在简谐近似下不稳定",
                self.modes.len()
            )
        } else {
            format!("{} 个振动模式，全部为实频", self.modes.len())
        };

        // 数量不完整时补一句，免得用户以为漏解析了
        let note = match &hessian {
            Some(h) => format!("{note}；力常数矩阵 {}×{}", h.n_dof, h.n_dof),
            None if self.hessian_rows > 0 => {
                format!("{note}；力常数矩阵不完整（解析到 {} 行，未能凑成方阵）", self.hessian_rows)
            }
            None => note,
        };

        VibrationReport {
            modes: self.modes,
            imaginary_count,
            hessian,
            note,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_and_imaginary_modes() {
        let mut s = VibrationScanner::new();
        s.feed("   1 f  =    5.432100 THz    34.132685 2PiTHz  181.222123 cm-1    22.465903 meV");
        s.feed("  56 f/i=    0.044329 THz     0.278526 2PiTHz    1.478647 cm-1     0.183329 meV");
        let r = s.finish();
        assert_eq!(r.modes.len(), 2);
        assert!(!r.modes[0].is_imaginary);
        assert!((r.modes[0].frequency_thz - 5.432100).abs() < 1e-9);
        assert!(r.modes[1].is_imaginary);
        // 虚频取负
        assert!((r.modes[1].frequency_thz + 0.044329).abs() < 1e-9);
        assert!((r.modes[1].frequency_cm1 + 1.478647).abs() < 1e-9);
        assert_eq!(r.imaginary_count, 1);
        assert!(r.note.contains("1 个虚频"));
    }

    #[test]
    fn parses_hessian_matrix() {
        let text = "\
 SECOND DERIVATIVES (NOT SYMMETRIZED)
 ------------------------------------
               1X          1Y          1Z
  1X    -4.618971    0.863326    0.887778
  1Y     0.831145   -5.553266    0.386467
  1Z     0.882002    0.394964   -5.435387
";
        let mut s = VibrationScanner::new();
        for l in text.lines() {
            s.feed(l);
        }
        let r = s.finish();
        let h = r.hessian.expect("3×3 矩阵应被识别");
        assert_eq!(h.n_dof, 3);
        assert_eq!(h.data.len(), 9);
        assert!((h.data[0] - (-4.618971)).abs() < 1e-9);
        assert!((h.data[8] - (-5.435387)).abs() < 1e-9);
    }

    #[test]
    fn rejects_incomplete_matrix() {
        // 只有两行 3 个数 → 不是方阵，宁可为 None 也不要画错位的热图
        let text = "\
 SECOND DERIVATIVES (NOT SYMMETRIZED)
 ------------------------------------
  1X    -4.6    0.8    0.8
  1Y     0.8   -5.5    0.3
";
        let mut s = VibrationScanner::new();
        for l in text.lines() {
            s.feed(l);
        }
        let r = s.finish();
        assert!(r.hessian.is_none());
        assert!(r.note.contains("不完整"));
    }

    #[test]
    fn no_frequency_output() {
        let mut s = VibrationScanner::new();
        s.feed("something else");
        let r = s.finish();
        assert!(r.modes.is_empty());
        assert!(r.note.contains("IBRION"));
    }
}
