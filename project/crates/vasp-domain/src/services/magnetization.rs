//! 从 OUTCAR 提取磁矩。
//!
//! ⚠️ **关于逐原子磁矩的现实**：`ISPIN=2` 的 OUTCAR 里，逐原子磁矩表
//! （以 `magnetization (x)` 开头、带 `# of ion` 表头）并不是无条件输出的。
//! 实测本机几份 `ISPIN=2` 的任务（VASP 5.4.4，`NWRITE` 分别 1 和 2）
//! **都没有这张表**，只有每个电子步的总磁矩标量。
//!
//! 所以这里只解析**能确定存在**的量：逐离子步的总磁矩与球间磁矩。
//! 逐原子磁矩需要 `LORBIT=11`（生成 PROCAR）或确认 OUTCAR 里确实有该块，
//! 拿到那样的数据之后再补解析器——不凭猜测写。

use crate::MagneticStep;
use regex::Regex;
use std::sync::OnceLock;

/// `number of electron  123.4567890 magnetization  12.3456789`
///
/// 第 1 个捕获组是电子数（这里用不上），第 2 个才是总磁矩。
fn total_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)number of electron\s+([-\d.]+)\s+magnetization\s+([-\d.]+)")
            .expect("总磁矩正则")
    })
}

/// `augmentation part  9.4486309 magnetization  6.7890123`
fn interstitial_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)augmentation part\s+([-\d.]+)\s+magnetization\s+([-\d.]+)")
            .expect("球间磁矩正则")
    })
}

/// 离子步结束的标志。每个离子步出现一次，用它把电子步的磁矩「结算」成一步。
fn ionic_end_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)FREE ENERGIE OF THE ION-ELECTRON SYSTEM").expect("离子步结束正则")
    })
}

/// 逐原子磁矩表的表头。用来判断「到底有没有」。
fn per_atom_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)magnetization \(x\)").expect("逐原子磁矩正则"))
}

/// 流式扫描器：喂 OUTCAR 的每一行，最后取出结果。
///
/// 为什么是状态机而不是一次性解析：单份 OUTCAR 可达几百 MB，
/// 不能整读进内存。配合 [`Fs::scan_lines`] 使用。
///
/// [`Fs::scan_lines`]: vasp_application::ports::Fs::scan_lines
#[derive(Debug, Default)]
pub struct MagnetizationScanner {
    /// 当前离子步内最后一次看到的总磁矩
    total: Option<f64>,
    /// 当前离子步内最后一次看到的球间磁矩
    interstitial: Option<f64>,
    steps: Vec<MagneticStep>,
    per_atom_seen: bool,
    /// 是否见过任何磁矩行——用于区分「ISPIN=1」和「格式不认识」
    saw_any: bool,
}

impl MagnetizationScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂一行。返回值恒为 `true`：磁矩可能分布在整个文件里，
    /// 不能因为已经拿到几步就提前停止。
    pub fn feed(&mut self, line: &str) {
        if per_atom_re().is_match(line) {
            self.per_atom_seen = true;
            return;
        }

        if let Some(c) = total_re().captures(line) {
            if let Ok(v) = c[2].parse::<f64>() {
                self.total = Some(v);
                self.saw_any = true;
            }
            return;
        }

        if let Some(c) = interstitial_re().captures(line) {
            if let Ok(v) = c[2].parse::<f64>() {
                self.interstitial = Some(v);
            }
            return;
        }

        if ionic_end_re().is_match(line) {
            // 一个离子步结束：把该步内最后一次的磁矩结算成一步
            if let Some(total) = self.total.take() {
                let index = self.steps.len() as u32 + 1;
                self.steps.push(MagneticStep {
                    index,
                    total_mu_b: total,
                    interstitial_mu_b: self.interstitial.take(),
                });
            } else {
                self.interstitial = None;
            }
        }
    }

    /// 是否见过任何磁矩输出。
    pub fn saw_any(&self) -> bool {
        self.saw_any
    }

    pub fn per_atom_seen(&self) -> bool {
        self.per_atom_seen
    }

    /// 取出结果。`note` 会直接显示给用户，所以措辞要能自己解释清楚。
    pub fn finish(self) -> (Vec<MagneticStep>, bool, String) {
        let note = if self.steps.is_empty() && !self.saw_any {
            "这份 OUTCAR 没有磁矩输出（多半是 ISPIN=1 的非自旋极化计算）".to_string()
        } else if self.per_atom_seen {
            "含逐原子磁矩表".to_string()
        } else {
            // 这是本机数据里最常见的情形，必须说清楚，否则用户会以为磁矩丢了
            "只有总磁矩：这份 OUTCAR 没有逐原子磁矩表（NWRITE 或 VASP 输出级别所致）。\
             要看每个原子的磁矩，需在 INCAR 里设 LORBIT=11 重算以生成 PROCAR"
                .to_string()
        };
        (self.steps, self.per_atom_seen, note)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_one_step_from_typical_block() {
        let mut s = MagnetizationScanner::new();
        s.feed(" number of electron     123.4567890 magnetization      12.3456789");
        s.feed(" augmentation part        4.5678901 magnetization       6.7890123");
        s.feed("  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)");
        let (steps, per_atom, _note) = s.finish();
        assert_eq!(steps.len(), 1);
        assert!((steps[0].total_mu_b - 12.3456789).abs() < 1e-9);
        assert!((steps[0].interstitial_mu_b.unwrap() - 6.7890123).abs() < 1e-9);
        assert!(!per_atom);
    }

    #[test]
    fn keeps_last_electronic_step_of_each_ionic_step() {
        let mut s = MagnetizationScanner::new();
        // 第一个离子步：两个电子步，应取后一个
        s.feed(" number of electron     123.0 magnetization      53.0");
        s.feed(" number of electron     123.0 magnetization      40.0");
        s.feed("  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)");
        // 第二个离子步
        s.feed(" number of electron     123.0 magnetization      35.0");
        s.feed("  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)");
        let (steps, _, _) = s.finish();
        assert_eq!(steps.len(), 2);
        assert!((steps[0].total_mu_b - 40.0).abs() < 1e-9);
        assert!((steps[1].total_mu_b - 35.0).abs() < 1e-9);
        assert_eq!(steps[1].index, 2);
    }

    #[test]
    fn reports_missing_magnetization() {
        let mut s = MagnetizationScanner::new();
        s.feed(" something unrelated");
        let (steps, per_atom, note) = s.finish();
        assert!(steps.is_empty());
        assert!(!per_atom);
        assert!(note.contains("ISPIN=1"), "note 应解释原因，实得：{note}");
    }

    #[test]
    fn detects_per_atom_table() {
        let mut s = MagnetizationScanner::new();
        s.feed(" magnetization (x)");
        s.feed(" number of electron     123.0 magnetization      30.0");
        s.feed("  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)");
        let (_, per_atom, note) = s.finish();
        assert!(per_atom);
        assert!(note.contains("逐原子"));
    }

    #[test]
    fn negative_magnetization_is_parsed() {
        // 反铁磁体系的总磁矩可能接近 0 甚至为负
        let mut s = MagnetizationScanner::new();
        s.feed(" number of electron     100.0 magnetization      -0.031");
        s.feed("  FREE ENERGIE OF THE ION-ELECTRON SYSTEM (eV)");
        let (steps, _, _) = s.finish();
        assert!((steps[0].total_mu_b + 0.031).abs() < 1e-9);
    }
}
