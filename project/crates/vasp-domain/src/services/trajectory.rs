//! 解析 XDATCAR，得到可播放的轨迹。
//!
//! 格式（VASP 标准）：
//! ```text
//!  <注释>                      1 行
//!  <scale>                     1 行
//!  <a> <b> <c>                 3 行晶格
//!  <元素名...>                  1 行
//!  <各元素原子数...>             1 行
//!  Direct configuration=     1  ← 之后每帧：1 行标记 + N 行坐标
//!   ...
//! ```
//!
//! 坐标系可能是 `Direct`（分数）或 `Cartesian`，两种情况都处理：
//! 分数坐标乘以晶格才得到 Å，否则渲染出来的盒子尺寸会是错的。

use crate::{DomainError, DomainResult, TrajectoryFrame, TrajectoryReport};

/// 帧数上限。超过就均匀抽样——前端播放器真正能看清的也就几百帧，
/// 而几万帧的数据会让 JSON 变成几十 MB，IPC 直接卡死。
pub const MAX_FRAMES: usize = 600;

/// 流式扫描器。第一遍只计数，第二遍按计划保留样本，内存不随总帧数增长。
pub struct TrajectoryScanner {
    line_no: usize,
    scale: f64,
    cell: [[f64; 3]; 3],
    elements: Vec<String>,
    counts: Vec<usize>,
    comment: String,
    n_atoms: usize,
    cur: Vec<[f64; 3]>,
    cur_index: usize,
    fractional: bool,
    frames: Vec<TrajectoryFrame>,
    total_frames: usize,
    retained_frames: Option<std::collections::HashSet<usize>>,
    /// 中途重写了头部（变胞 MD），记下来提醒调用方。
    header_rewritten: bool,
}

impl Default for TrajectoryScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectoryScanner {
    /// 计数模式：完整扫描，但不保存坐标帧。
    pub fn new() -> Self {
        Self {
            line_no: 0,
            scale: 1.0,
            cell: [[0.0; 3]; 3],
            elements: Vec::new(),
            counts: Vec::new(),
            comment: String::new(),
            n_atoms: 0,
            cur: Vec::new(),
            cur_index: 0,
            fractional: true,
            frames: Vec::new(),
            total_frames: 0,
            retained_frames: None,
            header_rewritten: false,
        }
    }

    /// 抽样模式：均匀覆盖完整区间，并始终保留首尾帧。
    pub fn sampled(total_frames: usize) -> Self {
        let sample_count = total_frames.min(MAX_FRAMES);
        let retained_frames = (0..sample_count)
            .map(|i| {
                if sample_count == 1 {
                    0
                } else {
                    i * (total_frames - 1) / (sample_count - 1)
                }
            })
            .collect();
        Self {
            retained_frames: Some(retained_frames),
            ..Self::new()
        }
    }

    /// 必须读到文件末尾，才能取得真实总帧数和最后一帧。
    pub fn wants_more(&self) -> bool {
        true
    }

    pub fn feed(&mut self, line: &str) -> bool {
        self.line_no += 1;
        let t = line.trim();

        match self.line_no {
            1 => self.comment = t.to_string(),
            2 => self.scale = t.parse().unwrap_or(1.0),
            3..=5 => {
                let idx = self.line_no - 3;
                if let Some(v) = parse_vec3(t) {
                    self.cell[idx] = v;
                }
            }
            6 => self.elements = t.split_whitespace().map(|s| s.to_string()).collect(),
            7 => {
                self.counts = t
                    .split_whitespace()
                    .filter_map(|s| s.parse::<usize>().ok())
                    .collect();
                self.n_atoms = self.counts.iter().sum();
            }
            _ => {
                if t.is_empty() {
                    return true;
                }

                let lower = t.to_ascii_lowercase();
                if lower.contains("configuration") {
                    if lower.contains("direct") || lower.contains("cartesian") {
                        self.fractional = lower.contains("direct");
                    }
                    self.flush_frame();
                    self.cur_index += 1;
                    if let Some(n) = t
                        .split('=')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<usize>().ok())
                    {
                        self.cur_index = n;
                    }
                    return true;
                }

                if self.cur.len() < self.n_atoms {
                    if let Some(v) = parse_vec3(t) {
                        self.cur.push(v);
                    }
                } else if self.n_atoms > 0 && parse_vec3(t).is_some() {
                    // 完整坐标帧之后、下一帧标记之前的额外套头晶格。
                    self.header_rewritten = true;
                }
                // 等下一帧的 configuration 或文件结束时收尾，避免重复保存末帧。
            }
        }
        true
    }

    fn flush_frame(&mut self) {
        if self.cur.len() != self.n_atoms || self.n_atoms == 0 {
            self.cur.clear();
            return;
        }
        let retain = self
            .retained_frames
            .as_ref()
            .is_some_and(|indices| indices.contains(&self.total_frames));
        if retain {
            let cell = self.scaled_cell();
            let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
            let positions = if self.fractional {
                self.cur.iter().map(|f| frac_to_cart(f, &cell)).collect()
            } else {
                self.cur
                    .iter()
                    .map(|p| [p[0] * scale, p[1] * scale, p[2] * scale])
                    .collect()
            };
            self.frames.push(TrajectoryFrame {
                index: self.cur_index.max(1),
                positions,
            });
        }
        self.total_frames += 1;
        self.cur.clear();
    }

    fn scaled_cell(&self) -> [[f64; 3]; 3] {
        let s = if self.scale > 0.0 { self.scale } else { 1.0 };
        [
            [self.cell[0][0] * s, self.cell[0][1] * s, self.cell[0][2] * s],
            [self.cell[1][0] * s, self.cell[1][1] * s, self.cell[1][2] * s],
            [self.cell[2][0] * s, self.cell[2][1] * s, self.cell[2][2] * s],
        ]
    }

    pub fn finish(mut self) -> DomainResult<TrajectoryReport> {
        self.flush_frame();
        let n_atoms = self.n_atoms;
        let cell = self.scaled_cell();
        let total = self.total_frames;

        if total == 0 {
            return Err(DomainError::InvalidStructure(
                "XDATCAR 里没有解析到任何结构帧".to_string(),
            ));
        }
        if self.scale < 0.0 {
            return Err(DomainError::InvalidStructure(
                "XDATCAR 的 scale 是负数（按目标体积缩放），首版不支持".to_string(),
            ));
        }

        let sampled = self.retained_frames.is_some() && total > MAX_FRAMES;
        let mut note = format!(
            "{} 帧，{} 个原子，坐标系 {}",
            total,
            n_atoms,
            if self.fractional { "Direct（分数）" } else { "Cartesian" }
        );
        if sampled {
            note.push_str(&format!("；帧数过多，已均匀抽样到 {} 帧", self.frames.len()));
        }
        if self.header_rewritten {
            note.push_str("；文件中途重写过头部（变胞），后段帧仍按首帧晶格渲染");
        }

        Ok(TrajectoryReport {
            source_file: "XDATCAR".to_string(),
            comment: self.comment,
            elements: self.elements,
            counts: self.counts,
            cell,
            frames: self.frames,
            total_frames: total,
            sampled,
            note,
        })
    }
}

fn parse_vec3(s: &str) -> Option<[f64; 3]> {
    let mut it = s.split_whitespace();
    let a = it.next()?.parse::<f64>().ok()?;
    let b = it.next()?.parse::<f64>().ok()?;
    let c = it.next()?.parse::<f64>().ok()?;
    Some([a, b, c])
}

/// 分数 → 笛卡尔：`cart = f0*a + f1*b + f2*c`。
fn frac_to_cart(f: &[f64; 3], cell: &[[f64; 3]; 3]) -> [f64; 3] {
    [
        f[0] * cell[0][0] + f[1] * cell[1][0] + f[2] * cell[2][0],
        f[0] * cell[0][1] + f[1] * cell[1][1] + f[2] * cell[2][1],
        f[0] * cell[0][2] + f[1] * cell[1][2] + f[2] * cell[2][2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
demo
   1.0
    10.0 0.0 0.0
    0.0 10.0 0.0
    0.0 0.0 10.0
 O U
 2 1
 Direct configuration=     1
  0.0 0.0 0.0
  0.5 0.5 0.5
  0.25 0.0 0.0
 Direct configuration=     2
  0.1 0.0 0.0
  0.5 0.5 0.5
  0.25 0.0 0.0
";

    fn feed_all(text: &str, scanner: &mut TrajectoryScanner) {
        for line in text.lines() {
            if !scanner.feed(line) {
                break;
            }
        }
    }

    fn scan(text: &str) -> TrajectoryReport {
        let mut counter = TrajectoryScanner::new();
        feed_all(text, &mut counter);
        let total_frames = counter.finish().unwrap().total_frames;
        let mut scanner = TrajectoryScanner::sampled(total_frames);
        feed_all(text, &mut scanner);
        scanner.finish().unwrap()
    }

    #[test]
    fn parses_header_and_frames() {
        let r = scan(SAMPLE);
        assert_eq!(r.comment, "demo");
        assert_eq!(r.elements, vec!["O", "U"]);
        assert_eq!(r.counts, vec![2, 1]);
        assert_eq!(r.frames.len(), 2);
        assert_eq!(r.total_frames, 2);
        assert!(!r.sampled);
        assert_eq!(r.frames[0].index, 1);
        assert_eq!(r.frames[1].index, 2);
    }

    #[test]
    fn converts_fractional_to_cartesian() {
        let r = scan(SAMPLE);
        // 0.5 分数 × 10 Å 晶格 = 5.0 Å
        assert!((r.frames[0].positions[1][0] - 5.0).abs() < 1e-9);
        assert!((r.frames[1].positions[0][0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn applies_scale_to_cell_and_positions() {
        let scaled = SAMPLE.replace("   1.0\n", "   2.0\n");
        let r = scan(&scaled);
        assert!((r.cell[0][0] - 20.0).abs() < 1e-9);
        // 0.5 × 20 = 10
        assert!((r.frames[0].positions[1][0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn large_trajectory_counts_and_keeps_the_actual_last_frame() {
        let header = SAMPLE.split(" Direct configuration").next().unwrap();
        let mut text = String::from(header);
        for i in 1..=2501 {
            text.push_str(&format!(" Direct configuration= {i}\n"));
            text.push_str("  0.0 0.0 0.0\n  0.5 0.5 0.5\n  0.25 0.0 0.0\n");
        }

        let r = scan(&text);

        assert_eq!(r.total_frames, 2501);
        assert_eq!(r.frames.first().unwrap().index, 1);
        assert_eq!(r.frames.last().unwrap().index, 2501);
        assert!(r.frames.len() <= MAX_FRAMES + 2);
    }

    #[test]
    fn cartesian_coordinates_include_positive_scale() {
        let cartesian = SAMPLE.replace("   1.0\n", "   2.0\n").replace("Direct", "Cartesian");
        let r = scan(&cartesian);

        assert_eq!(r.frames[0].positions[0], [0.0, 0.0, 0.0]);
        assert_eq!(r.frames[0].positions[1], [1.0, 1.0, 1.0]);
    }

    #[test]
    fn negative_scale_is_rejected() {
        let negative = SAMPLE.replace("   1.0\n", "  -1.0\n");
        let mut scanner = TrajectoryScanner::new();
        feed_all(&negative, &mut scanner);

        let error = scanner.finish().unwrap_err();
        assert!(error.to_string().contains("scale 是负数"));
    }

    #[test]
    fn repeated_cell_header_warns_about_variable_cell() {
        let repeated = SAMPLE.replace(
            " Direct configuration=     2",
            "demo\n1.0\n10.0 0.0 0.0\n0.0 10.0 0.0\n0.0 0.0 10.0\nO U\n2 1\nDirect configuration=     2",
        );
        let r = scan(&repeated);

        assert_eq!(r.total_frames, 2);
        assert!(r.note.contains("变胞"), "缺少变胞提示：{}", r.note);
    }

    #[test]
    fn empty_file_is_an_error() {
        let mut s = TrajectoryScanner::new();
        s.feed("only a comment");
        assert!(s.finish().is_err());
    }

    #[test]
    fn downsampling_keeps_first_and_last() {
        let mut text = String::from(SAMPLE.split(" Direct configuration").next().unwrap());
        for i in 1..=(MAX_FRAMES + 200) {
            text.push_str(&format!(" Direct configuration= {:>5}\n", i));
            text.push_str("  0.0 0.0 0.0\n  0.5 0.5 0.5\n  0.25 0.0 0.0\n");
        }
        let r = scan(&text);
        assert!(r.sampled);
        assert!(r.frames.len() <= MAX_FRAMES + 2);
        assert_eq!(r.frames.first().unwrap().index, 1);
        assert_eq!(
            r.frames.last().unwrap().index,
            r.total_frames,
            "必须保留最后一帧"
        );
    }
}
