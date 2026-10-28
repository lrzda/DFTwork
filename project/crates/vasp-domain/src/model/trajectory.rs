//! 结构变化轨迹（XDATCAR）。
//!
//! XDATCAR 比 OUTCAR 小三个量级（实测同一个 MD 任务：0.7 MB vs 584 MB），
//! 而且格式就是「头 + 每帧 N 行坐标」，所以它才是播放的正确数据源——
//! 从 OUTCAR 里抠 POSITION 块要扫几百 MB 才能拿到同样的东西。

/// 一帧结构。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrajectoryFrame {
    /// 文件里的序号，从 1 开始（和 `Direct configuration= N` 一致）。
    pub index: usize,
    /// 笛卡尔坐标（Å）。XDATCAR 里通常是分数坐标，解析时已经乘过晶格。
    pub positions: Vec<[f64; 3]>,
}

/// 整个轨迹。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrajectoryReport {
    /// 数据来自哪个文件。
    pub source_file: String,
    /// XDATCAR 第一行的注释。
    pub comment: String,
    /// 元素名，按 POSCAR 顺序。
    pub elements: Vec<String>,
    /// 各元素原子数，与 `elements` 一一对应。
    pub counts: Vec<usize>,
    /// 晶格矢量（Å）。已乘过 scale。
    ///
    /// 首版假定整个轨迹用同一套晶格——NPT 或变胞 MD 的 XDATCAR 会在中途
    /// 重写头部，那种情况目前不处理，会在 `note` 里说明。
    pub cell: [[f64; 3]; 3],
    pub frames: Vec<TrajectoryFrame>,
    /// 文件里实际有多少帧（降采样前）。
    pub total_frames: usize,
    /// 是否做过降采样（帧数超过上限时均匀抽样）。
    pub sampled: bool,
    /// 给人看的说明：来源、坐标系、有没有降采样。
    pub note: String,
}

impl TrajectoryReport {
    pub fn atom_count(&self) -> usize {
        self.counts.iter().sum()
    }

    /// 展开成逐原子元素名，与 `positions` 同序。
    pub fn element_per_atom(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.atom_count());
        for (el, n) in self.elements.iter().zip(self.counts.iter()) {
            for _ in 0..*n {
                out.push(el.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_per_atom_follows_poscar_order() {
        let r = TrajectoryReport {
            source_file: "XDATCAR".into(),
            comment: "test".into(),
            elements: vec!["O".into(), "U".into()],
            counts: vec![2, 1],
            cell: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            frames: vec![],
            total_frames: 0,
            sampled: false,
            note: String::new(),
        };
        assert_eq!(r.atom_count(), 3);
        assert_eq!(r.element_per_atom(), vec!["O", "O", "U"]);
    }
}
