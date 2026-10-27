use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atom {
    /// 元素符号（从 POTCAR 或 POSCAR 的注释行/化学式推断）。
    pub element: String,
    /// 笛卡尔坐标（Å）。首版统一换算成笛卡尔，分数坐标由 `cell` 反推。
    pub position: [f64; 3],
    /// 该原子属于第几个物种（POSCAR 的 species 分组下标）。
    pub species_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bond {
    /// 原子下标（`atoms` 里从 0 开始）。
    pub a: usize,
    pub b: usize,
}

/// 3D 看板要的完整场景。前端拿到它就能画，不需要再算任何东西。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureScene {
    /// 三行晶格矢量（Å）。
    pub cell: [[f64; 3]; 3],
    pub atoms: Vec<Atom>,
    /// 成键关系。首版可能为空数组——见 `services::structure::bonds` 的说明。
    pub bonds: Vec<Bond>,
    /// POSCAR 第一行的注释。
    pub comment: String,
    /// 是否含选择性动力学标记（POSCAR 第 7 行首字符为 `S`）。
    pub selective_dynamics: bool,
    /// 缩放因子（POSCAR 第二行）。
    pub scale: f64,
}

impl StructureScene {
    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }
}
