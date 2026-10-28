//! 解析 OSZICAR 里的 SCF 电子步序列。
//!
//! 与 `convergence.rs` 的分工：那里取的是**离子步**（`F=` 行，看整体是否收敛），
//! 这里取的是**离子步内部**的电子步迭代（看单次 SCF 收敛得好不好）。
//! 两者读同一个文件，但回答的是不同问题。

use crate::{ScfBlock, ScfSeries, ScfStep};
use regex::Regex;
use std::sync::OnceLock;

/// 每个离子步最多保留多少电子步（防止 NELM 设成几千时内存爆掉）。
const MAX_STEPS_PER_BLOCK: usize = 3000;

/// 最多保留多少个离子步的 SCF 明细。2611 个离子步的 MD 全留会太大，
/// 而用户真正关心的是「最近几步为什么难收敛」。
const MAX_BLOCKS: usize = 20;

/// `DAV:   1     -0.123456789012E+03    0.47236E+04   -0.19626E+05  1328   0.126E+03`
fn scf_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*(DAV|RMM):\s*(\d+)\s+([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?)\s+([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?)\s+([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?)\s+(\d+)\s+([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?)(?:\s+([-+]?\d*\.?\d+(?:[EeDd][-+]?\d+)?))?",
        )
        .expect("SCF 正则")
    })
}

/// 离子步结束标志：`   1 F= -.23456789E+03 E0= ...`
fn ionic_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*(\d+)\s+F=").expect("离子步正则")
    })
}

fn num(s: &str) -> Option<f64> {
    let cleaned = s.replace('D', "E").replace('d', "e");
    let fixed = if cleaned.starts_with('.') {
        format!("0{cleaned}")
    } else if cleaned.starts_with("-.") || cleaned.starts_with("+.") {
        format!("{}0{}", &cleaned[..1], &cleaned[1..])
    } else {
        cleaned
    };
    fixed.parse().ok()
}

/// 解析 OSZICAR 文本，得到逐离子步的 SCF 序列。
pub fn parse_oszicar_scf(text: &str) -> ScfSeries {
    let scf_re = scf_re();
    let ionic_re = ionic_re();

    let mut blocks: Vec<ScfBlock> = Vec::new();
    let mut current: Vec<ScfStep> = Vec::new();
    let mut ionic_index: u32 = 0;
    let mut total_blocks = 0usize;
    let mut truncated = false;

    for line in text.lines() {
        if let Some(c) = scf_re.captures(line) {
            // 步数超限就丢弃最早的，只留尾部
            if current.len() >= MAX_STEPS_PER_BLOCK {
                current.remove(0);
                truncated = true;
            }
            current.push(ScfStep {
                index: c[2].parse().unwrap_or(0),
                algorithm: c[1].to_uppercase(),
                energy_ev: num(&c[3]).unwrap_or(f64::NAN),
                delta_e_ev: num(&c[4]),
                delta_eps_ev: num(&c[5]),
                ncg: c[6].parse().ok(),
                rms: num(&c[7]),
                rms_c: c.get(8).and_then(|m| num(m.as_str())),
            });
            continue;
        }

        if let Some(c) = ionic_re.captures(line) {
            ionic_index = c[1].parse().unwrap_or(ionic_index + 1);
            if !current.is_empty() {
                total_blocks += 1;
                // 块数超限就丢最早的
                if blocks.len() >= MAX_BLOCKS {
                    blocks.remove(0);
                    truncated = true;
                }
                blocks.push(ScfBlock {
                    ionic_index,
                    steps: std::mem::take(&mut current),
                });
            }
        }
    }

    // 文件末尾还没遇到 `F=` 行（正在跑的任务）→ 把最后一个块也交出去
    if !current.is_empty() {
        total_blocks += 1;
        if blocks.len() >= MAX_BLOCKS {
            blocks.remove(0);
            truncated = true;
        }
        blocks.push(ScfBlock {
            ionic_index: ionic_index + 1,
            steps: current,
        });
    }

    ScfSeries {
        blocks,
        truncated,
        total_blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
    DAV:   1     -0.123456789012E+03    0.47236E+04   -0.19626E+05  1328   0.126E+03
    DAV:   2     -0.111111111111E+03   -0.41734E+04   -0.37783E+04  1488   0.295E+02
      1 F= -.23456789E+03 E0= -.23456789E+03  d E =-.442682E+03
    DAV:   1    -0.442682027688E+03    0.41684E-04   -0.45625E-05  1544   0.110E-02    0.620E-02
      2 F= -.23456789E+03 E0= -.23456789E+03  d E =-.442682E+03
";

    #[test]
    fn splits_scf_into_ionic_blocks() {
        let s = parse_oszicar_scf(SAMPLE);
        assert_eq!(s.blocks.len(), 2);
        assert_eq!(s.blocks[0].ionic_index, 1);
        assert_eq!(s.blocks[0].steps.len(), 2);
        assert_eq!(s.blocks[1].steps.len(), 1);
        assert_eq!(s.total_blocks, 2);
        assert!(!s.truncated);
    }

    #[test]
    fn parses_all_fields() {
        let s = parse_oszicar_scf(SAMPLE);
        let st = &s.blocks[0].steps[0];
        assert_eq!(st.algorithm, "DAV");
        assert_eq!(st.index, 1);
        assert!((st.energy_ev - -123.456789).abs() < 1e-6);
        assert!((st.delta_e_ev.unwrap() - -123.456789).abs() < 1e-3);
        assert_eq!(st.ncg, Some(1328));
        assert!((st.rms.unwrap() - 126.0).abs() < 1e-6);
        assert!(st.rms_c.is_none(), "第一行没有 rms(c)");
    }

    #[test]
    fn parses_optional_rms_c() {
        let s = parse_oszicar_scf(SAMPLE);
        let st = &s.blocks[1].steps[0];
        assert!((st.rms_c.unwrap() - 0.0062).abs() < 1e-9);
    }

    #[test]
    fn unfinished_block_is_returned() {
        // 正在跑的任务：最后一个离子步还没写出 F= 行
        let t = "    DAV:   1    -0.1E+03    0.1E+01   -0.1E+01  100   0.1E+00\n";
        let s = parse_oszicar_scf(t);
        assert_eq!(s.blocks.len(), 1);
        assert_eq!(s.blocks[0].steps.len(), 1);
    }

    #[test]
    fn empty_input_yields_empty_series() {
        let s = parse_oszicar_scf("");
        assert!(s.blocks.is_empty());
        assert_eq!(s.total_blocks, 0);
    }
}
