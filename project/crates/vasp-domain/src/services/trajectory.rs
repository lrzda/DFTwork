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

/// 头部固定 7 行（注释 / scale / 晶格 3 行 / 元素 / 计数）。
const HEADER_LINES: usize = 7;

/// 帧数上限。超过就均匀抽样——前端播放器真正能看清的也就几百帧，
/// 而几万帧的数据会让 JSON 变成几十 MB，IPC 直接卡死。
pub const MAX_FRAMES: usize = 600;

/// 流式扫描器：喂一行、攒一帧。
pub struct TrajectoryScanner {
    line_no: usize,
    scale: f64,
    cell: [[f64; 3]; 3],
    elements: Vec<String>,
    counts: Vec<usize>,
    comment: String,
    n_atoms: usize,
    /// 当前帧正在攒的坐标。
    cur: Vec<[f64; 3]>,
    cur_index: usize,
    fractional: bool,
    frames: Vec<TrajectoryFrame>,
    /// 中途重写了头部（变胞 MD），记下来提醒调用方。
    header_rewritten: bool,
}

impl Default for TrajectoryScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TrajectoryScanner {
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
            header_rewritten: false,
        }
    }

    /// 是否还要继续读（攒够帧后就不再读到底，XDATCAR 可能很大）。
    pub fn wants_more(&self) -> bool {
        // 多读一点再停，便于统计 total_frames 的真实量级
        self.frames.len() < MAX_FRAMES * 4
    }

    /// 喂一行。返回是否还要继续（`false` 让调用方提前结束扫描）。
    pub fn feed(&mut self, line: &str) -> bool {
        self.line_no += 1;
        let t = line.trim();

        match self.line_no {
            1 => self.comment = t.to_string(),
            2 => {
                self.scale = t.parse().unwrap_or(1.0);
            }
            3..=5 => {
                let idx = self.line_no - 3;
                if let Some(v) = parse_vec3(t) {
                    self.cell[idx] = v;
                }
            }
            6 => {
                self.elements = t.split_whitespace().map(|s| s.to_string()).collect();
            }
            7 => {
                self.counts = t
                    .split_whitespace()
                    .filter_map(|s| s.parse::<usize>().ok())
                    .collect();
                self.n_atoms = self.counts.iter().sum();
            }
            _ => {
                if t.is_empty() {
                    return self.wants_more();
                }

                let lower = t.to_ascii_lowercase();
                if lower.contains("configuration") {
                    // 变胞 MD 会在中途重写整个头部（comment/scale/cell/elements/counts）
                    // 首版不重建，但要让人知道看到的后续帧用的是旧晶格。
                    if lower.contains("direct") || lower.contains("cartesian") {
                        self.fractional = lower.contains("direct");
                    }
                    // 上一帧收尾
                    self.flush_frame();
                    self.cur_index += 1;
                    // 序号取行里的数字，取不到就用计数
                    if let Some(n) = t
                        .split('=')
                        .nth(1)
                        .and_then(|s| s.trim().parse::<usize>().ok())
                    {
                        self.cur_index = n;
                    }
                    return self.wants_more();
                }

                // 坐标行
                if self.cur.len() < self.n_atoms {
                    if let Some(v) = parse_vec3(t) {
                        self.cur.push(v);
                    }
                }
                // 坐标攒满不立刻 flush：等下一帧的 configuration 行或文件结束，
                // 免得把「刚好攒满」和「最后一帧」两种情况写两遍。
            }
        }
        self.wants_more()
    }

    fn flush_frame(&mut self) {
        if self.cur.len() != self.n_atoms || self.n_atoms == 0 {
            self.cur.clear();
            return;
        }
        let positions = if self.fractional {
            self.cur
                .iter()
                .map(|f| frac_to_cart(f, &self.scaled_cell()))
                .collect()
        } else {
            self.cur.clone()
        };
        self.frames.push(TrajectoryFrame {
            index: self.cur_index.max(1),
            positions,
        });
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

    /// 收尾：把最后一帧也收进来，然后按上限降采样。
    pub fn finish(mut self) -> DomainResult<TrajectoryReport> {
        self.flush_frame();
        // 后面 comment/elements/counts 会被 move 出去，先把要用到的都取出来
        let n_atoms = self.n_atoms;
        let cell = self.scaled_cell();

        if self.frames.is_empty() {
            return Err(DomainError::InvalidStructure(
                "XDATCAR 里没有解析到任何结构帧".to_string(),
            ));
        }
        if self.scale < 0.0 {
            return Err(DomainError::InvalidStructure(
                "XDATCAR 的 scale 是负数（按目标体积缩放），首版不支持".to_string(),
            ));
        }

        let total = self.frames.len();
        let sampled = total > MAX_FRAMES;
        let frames = if sampled {
            // 均匀抽样，但一定保留首尾两帧——否则看不出「从哪来、到哪去」
            let stride = (total as f64 / MAX_FRAMES as f64).ceil() as usize;
            let mut picked: Vec<TrajectoryFrame> = self
                .frames
                .iter()
                .step_by(stride.max(1))
                .cloned()
                .collect();
            if picked.last().map(|f| f.index) != self.frames.last().map(|f| f.index) {
                picked.push(self.frames.last().unwrap().clone());
            }
            picked
        } else {
            self.frames.clone()
        };

        let mut note = format!(
            "{} 帧，{} 个原子，坐标系 {}",
            total,
            n_atoms,
            if self.fractional { "Direct（分数）" } else { "Cartesian" }
        );
        if sampled {
            note.push_str(&format!("；帧数过多，已均匀抽样到 {} 帧", frames.len()));
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
            frames,
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

    fn scan(text: &str) -> TrajectoryReport {
        let mut s = TrajectoryScanner::new();
        for line in text.lines() {
            s.feed(line);
        }
        s.finish().unwrap()
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
