use super::elements::covalent_radius;
use super::geometry::{invert3x3, lattice_matrix, min_image_distance, to_fractional, Vec3};
use crate::{Atom, Bond, DomainError, DomainResult, StructureScene};

/// 解析 POSCAR / CONTCAR 文本。
///
/// 覆盖 VASP4（无元素行）与 VASP5（有元素行）、Direct 与 Cartesian、
/// 含/不含 Selective dynamics 四种组合。
/// **不处理**：负数缩放因子（按目标体积给定），遇到时返回错误而不是猜。
pub fn parse_poscar(text: &str) -> DomainResult<StructureScene> {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim_end_matches(['\r', '\n']))
        .filter(|l| !l.trim().is_empty())
        .collect();

    if lines.len() < 8 {
        return Err(DomainError::InvalidStructure(format!(
            "行数不足，只有 {} 行",
            lines.len()
        )));
    }

    let comment = lines[0].trim().to_string();

    let scale: f64 = lines[1]
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| DomainError::InvalidStructure("第二行缩放因子无法解析".into()))?;
    if scale <= 0.0 {
        return Err(DomainError::InvalidStructure(
            "负数缩放因子（按体积给定）首版不支持".into(),
        ));
    }

    let mut cell = [[0.0f64; 3]; 3];
    for i in 0..3 {
        let parts: Vec<&str> = lines[2 + i].split_whitespace().collect();
        if parts.len() < 3 {
            return Err(DomainError::InvalidStructure(format!(
                "晶格第 {} 行不足 3 个数",
                i + 1
            )));
        }
        for j in 0..3 {
            let v: f64 = parts[j].parse().map_err(|_| {
                DomainError::InvalidStructure(format!("晶格第 {} 行第 {} 列不是数字", i + 1, j + 1))
            })?;
            cell[i][j] = v * scale;
        }
    }

    // 第 6 行（下标 5）：全是整数 → VASP4 格式，没有元素行
    let row5: Vec<&str> = lines[5].split_whitespace().collect();
    let vasp4 = !row5.is_empty() && row5.iter().all(|t| t.parse::<usize>().is_ok());

    let (elements, counts, cursor) = if vasp4 {
        let counts = parse_counts(&row5)?;
        let total: usize = counts.iter().sum();
        let els = vec!["X".to_string(); counts.len()];
        let _ = total;
        (els, counts, 6usize)
    } else {
        let els: Vec<String> = row5.iter().map(|s| s.to_string()).collect();
        let count_row: Vec<&str> = lines[6].split_whitespace().collect();
        let counts = parse_counts(&count_row)?;
        if counts.len() != els.len() {
            return Err(DomainError::InvalidStructure(format!(
                "元素 {} 个，但计数 {} 个",
                els.len(),
                counts.len()
            )));
        }
        (els, counts, 7usize)
    };

    let total_atoms: usize = counts.iter().sum();
    if total_atoms == 0 {
        return Err(DomainError::InvalidStructure("原子数为 0".into()));
    }

    let mut idx = cursor;

    // 可选的 Selective dynamics
    let mut selective_dynamics = false;
    if lines
        .get(idx)
        .map(|l| l.trim_start().starts_with(['S', 's']))
        .unwrap_or(false)
    {
        selective_dynamics = true;
        idx += 1;
    }

    let mode = lines
        .get(idx)
        .map(|l| l.trim().to_lowercase())
        .ok_or_else(|| DomainError::InvalidStructure("缺少坐标模式行".into()))?;
    let direct = match mode.chars().next() {
        Some('d') => true,
        Some('c') | Some('k') => false,
        _ => {
            return Err(DomainError::InvalidStructure(format!(
                "无法识别的坐标模式：{mode}"
            )))
        }
    };
    idx += 1;

    let mut atoms = Vec::with_capacity(total_atoms);
    for (species_index, count) in counts.iter().enumerate() {
        for _ in 0..*count {
            let line = lines.get(idx).ok_or_else(|| {
                DomainError::InvalidStructure(format!("坐标行不够，需要 {total_atoms} 个原子"))
            })?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(DomainError::InvalidStructure(format!(
                    "第 {} 个原子坐标不足 3 列",
                    idx + 1
                )));
            }
            let mut v = [0.0f64; 3];
            for j in 0..3 {
                v[j] = parts[j].parse().map_err(|_| {
                    DomainError::InvalidStructure(format!("原子坐标不是数字：{}", line.trim()))
                })?;
            }
            let pos = if direct {
                // 分数坐标 → 笛卡尔：f0*a0 + f1*a1 + f2*a2
                [
                    v[0] * cell[0][0] + v[1] * cell[1][0] + v[2] * cell[2][0],
                    v[0] * cell[0][1] + v[1] * cell[1][1] + v[2] * cell[2][1],
                    v[0] * cell[0][2] + v[1] * cell[1][2] + v[2] * cell[2][2],
                ]
            } else {
                [v[0] * scale, v[1] * scale, v[2] * scale]
            };
            atoms.push(Atom {
                element: elements[species_index].clone(),
                position: pos,
                species_index,
            });
            idx += 1;
        }
    }

    let mut scene = StructureScene {
        cell,
        atoms,
        bonds: Vec::new(),
        comment,
        selective_dynamics,
        scale,
        // 文件名由用例层填（解析不认识文件名）
        source_file: None,
    };
    // 场景要给前端直接画，所以成键在这里一次算完。
    // 算不出来（坏晶格、原子过多）就当没有键——宁可少画几十根线，
    // 也不要因为成键失败让用户连结构都看不到。
    scene.bonds = bonds(&scene).unwrap_or_default();
    Ok(scene)
}

fn parse_counts(row: &[&str]) -> DomainResult<Vec<usize>> {
    let mut out = Vec::with_capacity(row.len());
    for t in row {
        let n: usize = t
            .parse()
            .map_err(|_| DomainError::InvalidStructure(format!("原子数不是整数：{t}")))?;
        out.push(n);
    }
    if out.is_empty() {
        return Err(DomainError::InvalidStructure("原子数行为空".into()));
    }
    Ok(out)
}

/// 成键判据的容差系数。
///
/// 判据：`最小镜像距离 < (共价半径_i + 共价半径_j) × BOND_TOLERANCE`。
///
/// 为什么不是 1.0：共价半径是单键的典型值，而实测键长会在它附近浮动
/// （配位改变、氧化态改变、部分键级都会拉长）。1.2 是常见晶体的经验值，
/// 在 测试体系的 测试体系 体系上给出 一批键（同元素对 111、异元素对 37、异元素对 24、同元素对 15），
/// 与"某结构特征"的直觉相符。
///
/// 为什么不照抄 `reference/VASPFlow` 的 `(vdw_i + vdw_j) / 2`：
/// 那个判据在同一份数据上给出 **0 根键**——阈值只有 1.66–1.93 Å，
/// 而体系里每个原子的最近邻中位距离是 2.80 Å。它既除了 2（范德华半径之和
/// 本身才是接触距离），元素表又只覆盖到 Rn(Z=86)，锕系全部走默认值 1.77 Å。
/// 属于原实现的 bug，登记在 `docs/rust-rewrite.md` 的已知差异表里。
pub const BOND_TOLERANCE: f64 = 1.2;

/// 超过这个原子数就不算成键。
///
/// 当前是 O(N²) × 27 镜像，2048 个原子约 5.6 千万次距离计算，已是可接受的上限。
/// 更大的体系要换 cell list（分格邻域搜索）；在那之前宁可返回空，
/// 也不要让界面卡住几十秒。
pub const BOND_ATOM_LIMIT: usize = 2048;

/// 计算成键关系。
///
/// 规则：
/// 1. 笛卡尔坐标乘晶格逆矩阵得到分数坐标；
/// 2. 对每一对原子求**最小镜像距离**（跨界相邻的原子必须能连上）；
/// 3. 距离小于两者共价半径之和 × [`BOND_TOLERANCE`] 即判为成键。
///
/// 返回的 `Bond` 只有原子下标对，不含"连的是哪个镜像"的信息——
/// 所以同一对原子即使有多个周期镜像同时成键，也只记录一次。
///
/// 退化情况一律返回空而不是报错：晶格不可逆、原子过多、原子数小于 2。
/// 调用方拿到空 `bonds` 时应降级为「只画原子」。
pub fn bonds(scene: &StructureScene) -> DomainResult<Vec<Bond>> {
    let n = scene.atoms.len();
    if n < 2 || n > BOND_ATOM_LIMIT {
        return Ok(Vec::new());
    }

    let lattice = lattice_matrix(&scene.cell);
    let Some(inv) = invert3x3(&lattice) else {
        // 三行共面之类的坏晶格：算出来的距离没有意义
        return Ok(Vec::new());
    };

    let frac: Vec<Vec3> = scene
        .atoms
        .iter()
        .map(|a| to_fractional(&inv, &a.position))
        .collect();
    let radii: Vec<f64> = scene
        .atoms
        .iter()
        .map(|a| covalent_radius(&a.element))
        .collect();

    let mut out = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let threshold = (radii[i] + radii[j]) * BOND_TOLERANCE;
            if threshold <= 0.0 {
                continue;
            }
            if min_image_distance(&frac[i], &frac[j], &lattice) < threshold {
                out.push(Bond { a: i, b: j });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSCAR_V5_DIRECT: &str = "\
Silicon diamond
1.0
5.43 0.0 0.0
0.0 5.43 0.0
0.0 0.0 5.43
Si
2
Direct
0.0 0.0 0.0
0.25 0.25 0.25
";

    #[test]
    fn parses_vasp5_direct() {
        let s = parse_poscar(POSCAR_V5_DIRECT).unwrap();
        assert_eq!(s.atoms.len(), 2);
        assert_eq!(s.atoms[0].element, "Si");
        assert_eq!(s.atoms[1].element, "Si");
        assert_eq!(s.comment, "Silicon diamond");
        // 0.25,0.25,0.25 × 5.43 → 1.3575
        assert!((s.atoms[1].position[0] - 1.3575).abs() < 1e-9);
    }

    #[test]
    fn bonds_connect_close_atoms() {
        // 立方盒子边长 2.5 Å。O 的共价半径 0.66，阈值 = (0.66+0.66)×1.2 = 1.584 Å。
        // 两原子间距 0.6×2.5 = 1.5 Å < 1.584 → 应成键。
        let text = "\
bonds
1.0
2.5 0.0 0.0
0.0 2.5 0.0
0.0 0.0 2.5
O
2
Direct
0.0 0.0 0.0
0.6 0.0 0.0
";
        let s = parse_poscar(text).unwrap();
        assert_eq!(s.bonds.len(), 1, "1.5 Å 应被判为 O-O 成键");
        assert_eq!((s.bonds[0].a, s.bonds[0].b), (0, 1));
    }

    #[test]
    fn bonds_cross_periodic_boundary() {
        // f=0.05 与 f=0.95 在盒子两侧：笛卡尔距离 0.9×2.5=2.25 Å（超阈值），
        // 但跨界只差 0.1×2.5=0.25 Å。必须用最小镜像才连得上——
        // 这正是 MD 快照里最常见的形态。
        let text = "\
crossing
1.0
2.5 0.0 0.0
0.0 2.5 0.0
0.0 0.0 2.5
O
2
Direct
0.05 0.5 0.5
0.95 0.5 0.5
";
        let s = parse_poscar(text).unwrap();
        assert_eq!(s.bonds.len(), 1, "跨界相邻原子必须成键");
    }

    #[test]
    fn far_atoms_stay_unbonded() {
        // 10 Å 盒子里差半个周期 = 5 Å，远大于 O-O 阈值 1.584 Å
        let text = "\
far
1.0
10.0 0.0 0.0
0.0 10.0 0.0
0.0 0.0 10.0
O
2
Direct
0.0 0.0 0.0
0.5 0.0 0.0
";
        let s = parse_poscar(text).unwrap();
        assert_eq!(s.bonds.len(), 0, "5 Å 不该成键");
    }

    #[test]
    fn parses_vasp4_without_elements() {
        let t = "\
water
1.0
10.0 0.0 0.0
0.0 10.0 0.0
0.0 0.0 10.0
3
Cartesian
0.0 0.0 0.0
1.0 0.0 0.0
0.0 1.0 0.0
";
        let s = parse_poscar(t).unwrap();
        assert_eq!(s.atoms.len(), 3);
        assert_eq!(s.atoms[0].element, "X");
        assert!((s.atoms[1].position[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parses_selective_dynamics() {
        let t = "\
slab
1.0
3.0 0.0 0.0
0.0 3.0 0.0
0.0 0.0 20.0
O
2
Selective dynamics
Direct
0.0 0.0 0.1 T T F
0.5 0.5 0.2 F F F
";
        let s = parse_poscar(t).unwrap();
        assert!(s.selective_dynamics);
        assert_eq!(s.atoms.len(), 2);
        assert!((s.atoms[0].position[2] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_negative_scale_for_now() {
        assert!(parse_poscar("x\n-2.0\n1 0 0\n0 1 0\n0 0 1\nX\n1\nDirect\n0 0 0\n").is_err());
    }
}
