//! 从 OUTCAR 提取每原子力与应力。
//!
//! 两段数据都在文件中部、且每个离子步重复一次，所以用流式状态机
//! （配合 `Fs::scan_lines`），不整读文件。

use crate::{AtomForce, ForceReport, ForceSet, StressReport, StressSet};
use regex::Regex;
use std::sync::OnceLock;

/// 最多保留最近几组（每组含全部原子）。用户真正关心的是最后几步。
const MAX_SETS: usize = 3;

fn force_header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"POSITION\s+TOTAL-FORCE").expect("力表头正则"))
}

/// 6 个数的一行：坐标 + 力
fn force_row_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s*$",
        )
        .expect("力行正则")
    })
}

fn stress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*in kB\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)\s+([-+]?\d*\.?\d+)",
        )
        .expect("应力正则")
    })
}

/// 离子步计数：OUTCAR 里没有直接的离子步编号，
/// 用 `FREE ENERGIE OF THE ION-ELECTRON SYSTEM` 出现次数来打标。
fn ionic_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)FREE ENERGIE OF THE ION-ELECTRON SYSTEM").expect("离子步标记正则")
    })
}

/// 力 + 应力的联合扫描器。两者在同一个文件里、同一轮扫描中提取，
/// 避免为了两段数据读两遍 几百 MB。
#[derive(Debug, Default)]
pub struct ForceStressScanner {
    /// 是否处在「力表头已出现、还在等虚线」的状态
    expect_rows: bool,
    /// 正在累积的原子受力
    buffer: Vec<AtomForce>,
    force_sets: Vec<ForceSet>,
    stress_sets: Vec<StressSet>,
    total_force_sets: usize,
    total_stress_sets: usize,
    ionic_index: u32,
    truncated: bool,
}

impl ForceStressScanner {
    pub fn new() -> Self {
        Self {
            // 离子步从 1 开始编号；单点计算的力段出现在第一个
            // FREE ENERGIE 之前，用 0 会让人误以为是「第零步」
            ionic_index: 1,
            ..Default::default()
        }
    }

    pub fn feed(&mut self, line: &str) {
        // 离子步计数（用于给力和应力打标）
        if ionic_marker_re().is_match(line) {
            self.ionic_index += 1;
        }

        // 力表头 → 下一行是虚线，再之后是数据
        if force_header_re().is_match(line) {
            self.expect_rows = true;
            self.buffer.clear();
            return;
        }

        if self.expect_rows {
            // 虚线行跳过；第一个非虚线行开始算数据
            if line.trim_start().starts_with("---") {
                return;
            }
            if let Some(c) = force_row_re().captures(line) {
                let v = |i: usize| c[i].parse::<f64>().unwrap_or(f64::NAN);
                let (fx, fy, fz) = (v(4), v(5), v(6));
                self.buffer.push(AtomForce {
                    index: self.buffer.len(),
                    position: [v(1), v(2), v(3)],
                    force: [fx, fy, fz],
                    magnitude_ev_per_a: (fx * fx + fy * fy + fz * fz).sqrt(),
                });
                return;
            }
            // 数据段结束（空行、total drift 等）
            self.finish_force_block();
        }

        if let Some(c) = stress_re().captures(line) {
            let v = |i: usize| c[i].parse::<f64>().unwrap_or(f64::NAN);
            let comp = [v(1), v(2), v(3), v(4), v(5), v(6)];
            self.total_stress_sets += 1;
            if self.stress_sets.len() >= MAX_SETS {
                self.stress_sets.remove(0);
                self.truncated = true;
            }
            self.stress_sets.push(StressSet {
                ionic_index: self.ionic_index,
                pressure_kb: (comp[0] + comp[1] + comp[2]) / 3.0,
                components_kb: comp,
            });
        }
    }

    fn finish_force_block(&mut self) {
        self.expect_rows = false;
        if self.buffer.is_empty() {
            return;
        }
        let max = self
            .buffer
            .iter()
            .map(|a| a.magnitude_ev_per_a)
            .fold(0.0f64, f64::max);

        self.total_force_sets += 1;
        if self.force_sets.len() >= MAX_SETS {
            self.force_sets.remove(0);
            self.truncated = true;
        }
        self.force_sets.push(ForceSet {
            ionic_index: self.ionic_index,
            atoms: std::mem::take(&mut self.buffer),
            max_force_ev_per_a: max,
        });
    }

    pub fn finish(mut self) -> (ForceReport, StressReport) {
        // 文件在力数据段中途结束（任务被杀）时，别丢掉已解析的部分
        if self.expect_rows {
            self.finish_force_block();
        }

        // 最新的排前面：看力时永远关心最后一步
        let mut force_sets = self.force_sets;
        force_sets.reverse();
        let mut stress_sets = self.stress_sets;
        stress_sets.reverse();

        (
            ForceReport {
                sets: force_sets,
                truncated: self.truncated,
                total_sets: self.total_force_sets,
            },
            StressReport {
                sets: stress_sets,
                truncated: self.truncated,
                total_sets: self.total_stress_sets,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)
 POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      4.65438      6.00944      3.38929        -1.912784     -0.142551     -1.840043
      8.71375      7.29314      8.00621         2.638826     -2.166500      1.340602

  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)
 in kB      36.83850     9.46328     9.83154    -1.06092     2.16883    12.17631
";

    #[test]
    fn parses_force_block() {
        let mut s = ForceStressScanner::new();
        for l in SAMPLE.lines() {
            s.feed(l);
        }
        let (f, _) = s.finish();
        assert_eq!(f.sets.len(), 1);
        assert_eq!(f.sets[0].atoms.len(), 2);
        assert_eq!(f.sets[0].atoms[0].index, 0);
        // 第一个原子的力大小
        let m = f.sets[0].atoms[0].magnitude_ev_per_a;
        let expect = (1.912784f64.powi(2) + 0.142551f64.powi(2) + 1.840043f64.powi(2)).sqrt();
        assert!((m - expect).abs() < 1e-9);
        assert!((f.sets[0].max_force_ev_per_a - expect.max(3.5)).abs() < 1.0);
    }

    #[test]
    fn parses_stress_and_pressure() {
        let mut s = ForceStressScanner::new();
        for l in SAMPLE.lines() {
            s.feed(l);
        }
        let (_, st) = s.finish();
        assert_eq!(st.sets.len(), 1);
        let c = st.sets[0].components_kb;
        assert!((c[0] - 36.83850).abs() < 1e-6);
        assert!((c[5] - 12.17631).abs() < 1e-6);
        let expect_p = (36.83850 + 9.46328 + 9.83154) / 3.0;
        assert!((st.sets[0].pressure_kb - expect_p).abs() < 1e-9);
    }

    #[test]
    fn keeps_latest_first() {
        // 两组力数据：finish 后最新的应排在最前
        let text = "\
 POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      0.0 0.0 0.0   1.0 0.0 0.0
      0.0 0.0 0.0   1.0 0.0 0.0

 POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      0.0 0.0 0.0   2.0 0.0 0.0
      0.0 0.0 0.0   2.0 0.0 0.0
";
        let mut s = ForceStressScanner::new();
        for l in text.lines() {
            s.feed(l);
        }
        let (f, _) = s.finish();
        assert_eq!(f.sets.len(), 2);
        // 最新的（力=2）在前
        assert!((f.sets[0].max_force_ev_per_a - 2.0).abs() < 1e-9);
        assert!((f.sets[1].max_force_ev_per_a - 1.0).abs() < 1e-9);
    }

    #[test]
    fn truncated_at_last_force_block() {
        // 文件在力数据中途结束：已解析的行不能丢
        let text = "\
 POSITION                                       TOTAL-FORCE (eV/Angst)
 -----------------------------------------------------------------------------------
      0.0 0.0 0.0   1.0 0.0 0.0
";
        let mut s = ForceStressScanner::new();
        for l in text.lines() {
            s.feed(l);
        }
        let (f, _) = s.finish();
        assert_eq!(f.sets.len(), 1);
        assert_eq!(f.sets[0].atoms.len(), 1);
    }
}
