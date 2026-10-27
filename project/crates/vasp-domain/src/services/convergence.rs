use crate::{ConvergenceSeries, IonicStep};
use regex::Regex;
use std::sync::OnceLock;

/// OSZICAR 的离子步行：
/// ```text
///    1 F= -.12345678E+02 E0= -.12345678E+02  d E =-.123456E-03
/// ```
/// 注意 F 前可能紧贴负号、指数可能用 `E` 或 `D`（Fortran 双精度写法）。
fn ionic_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*(\d+)\s+F=\s*([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?)").expect("离子步正则")
    })
}

fn force_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*([-+]?\d+\.\d+)\s+([-+]?\d+\.\d+)\s+([-+]?\d+\.\d+)\s+([-+]?\d+\.\d+)\s+([-+]?\d+\.\d+)\s+([-+]?\d+\.\d+)\s*$").expect("力矩阵正则"))
}

/// Fortran 浮点：`-.123E+02` / `0.123D-02` → f64。
fn parse_fortran_float(s: &str) -> Option<f64> {
    let cleaned = s.replace('D', "E").replace('d', "e");
    // `.5` 这种省略整数位的写法 Rust 不接受
    let fixed = if cleaned.starts_with('.') {
        format!("0{cleaned}")
    } else if cleaned.starts_with("-.") || cleaned.starts_with("+.") {
        format!("{}0{}", &cleaned[..1], &cleaned[1..])
    } else {
        cleaned
    };
    fixed.parse::<f64>().ok()
}

/// 解析 OSZICAR 里的离子步能量。
///
/// **只读 F 值**。它到底是自由能还是总能取决于 ISIF，代码不替它决定，
/// 界面上按 INCAR 的 ISIF 显示对应标签。
pub fn parse_oszicar(text: &str) -> Vec<IonicStep> {
    let re = ionic_re();
    let mut steps = Vec::new();
    for line in text.lines() {
        if let Some(c) = re.captures(line) {
            let index = c[1].parse::<u32>().ok();
            let energy = parse_fortran_float(&c[2]);
            if let Some(index) = index {
                steps.push(IonicStep {
                    index,
                    energy_ev: energy,
                    max_force_ev_per_a: None,
                });
            }
        }
    }
    steps
}

/// VASP 自己写在 OUTCAR 里的每步汇总行：
/// `FORCES: max atom, RMS     0.123456   0.045678`（VASP 5，同一行）
///
/// **必须优先用它**——这是 VASP 自己算好的值，而数据段解析要靠
/// `total drift:` / 分隔线判断块的起止，多一个空行就可能错一步。
fn max_atom_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)FORCES:\s+max atom,\s+RMS\s+([\d.\-+EDed]+)").expect("max atom 正则")
    })
}

/// 从 OUTCAR 提取每个离子步的最大力（eV/Å）。
///
/// 两条路径，与原实现 `parser.js::parseMaxForces` 对齐：
/// 1. **优先** `FORCES: max atom, RMS <值>` 行（每步一行）；
/// 2. 取不到时，退化到 `POSITION  TOTAL-FORCE (eV/Angst)` 数据段，
///    以 `total drift:` 或数据结束为分块标志。
pub fn parse_outcar_forces(text: &str) -> Vec<f64> {
    // 1) 汇总行
    let mut inline = Vec::new();
    for line in text.lines() {
        if let Some(c) = max_atom_re().captures(line) {
            if let Some(v) = parse_fortran_float(&c[1]) {
                inline.push(v);
            }
        }
    }
    if !inline.is_empty() {
        return inline;
    }

    // 2) 退化：数据段
    let re = force_block_re();
    let mut out = Vec::new();
    let mut in_block = false;
    let mut block_max: f64 = 0.0;
    let mut rows = 0usize;

    for line in text.lines() {
        if line.contains("TOTAL-FORCE") {
            if in_block && rows > 0 {
                out.push(block_max);
            }
            in_block = true;
            block_max = 0.0;
            rows = 0;
            continue;
        }
        if !in_block {
            continue;
        }
        if let Some(c) = re.captures(line) {
            let fx = parse_fortran_float(&c[4]).unwrap_or(0.0);
            let fy = parse_fortran_float(&c[5]).unwrap_or(0.0);
            let fz = parse_fortran_float(&c[6]).unwrap_or(0.0);
            let norm = (fx * fx + fy * fy + fz * fz).sqrt();
            if norm > block_max {
                block_max = norm;
            }
            rows += 1;
        } else if rows > 0 {
            out.push(block_max);
            in_block = false;
            block_max = 0.0;
            rows = 0;
        }
    }
    if in_block && rows > 0 {
        out.push(block_max);
    }
    out
}

/// 合并能量与力。两者长度可能不同（OUTCAR 只写了尾部、或力段缺失），
/// 以能量步数为准，力按顺序对齐，不足处留 `None`——**不补 0**。
pub fn merge_series(source: &str, energies: Vec<IonicStep>, forces: Vec<f64>) -> ConvergenceSeries {
    let steps = energies
        .into_iter()
        .enumerate()
        .map(|(i, mut s)| {
            s.max_force_ev_per_a = forces.get(i).copied();
            s
        })
        .collect::<Vec<_>>();

    ConvergenceSeries {
        steps,
        reached_required_accuracy: None,
        source: source.to_string(),
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OSZICAR: &str = "\
      N       E                     dE             d eps       ncg     rms          rms(c)
DAV:   1    -0.123456789012E+03   -0.123456E+03   -0.500000E+02   320   0.123E+00
DAV:   2    -0.123456800000E+03   -0.123456E-04   -0.123456E-05   310   0.456E-03
   1 F= -.12345678E+03 E0= -.12345678E+03  d E =-.123456E-03
DAV:   1    -0.123456700000E+03   -0.123456E+03   -0.500000E+02   318   0.111E+00
   2 F= -.12345670E+03 E0= -.12345670E+03  d E =-.100000E-03
";

    #[test]
    fn parses_ionic_steps_only() {
        let steps = parse_oszicar(OSZICAR);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].index, 1);
        assert!((steps[0].energy_ev.unwrap() + 123.45678).abs() < 1e-6);
        assert_eq!(steps[1].index, 2);
    }

    #[test]
    fn handles_fortran_exponent_forms() {
        let t = "   1 F= -0.123456D+03\n   2 F=  .123456E+03\n";
        let steps = parse_oszicar(t);
        assert_eq!(steps.len(), 2);
        assert!((steps[0].energy_ev.unwrap() + 123.456).abs() < 1e-6);
        assert!((steps[1].energy_ev.unwrap() - 123.456).abs() < 1e-6);
    }

    #[test]
    fn parses_force_blocks() {
        let outcar = "\
  POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      0.00000      0.00000      0.00000         0.000000      0.000000      0.000000
      1.00000      1.00000      1.00000         0.100000      0.000000      0.000000
  POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      0.00000      0.00000      0.00000         0.300000      0.400000      0.000000
      1.00000      1.00000      1.00000         0.000000      0.000000      0.000000
";
        let f = parse_outcar_forces(outcar);
        assert_eq!(f.len(), 2);
        assert!((f[0] - 0.1).abs() < 1e-9);
        assert!((f[1] - 0.5).abs() < 1e-9);
    }
}
