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

    Ok(StructureScene {
        cell,
        atoms,
        // 成键留空：见下方 `bonds()` 的说明。首版由前端按半径画，不做服务端成键。
        bonds: Vec::new(),
        comment,
        selective_dynamics,
        scale,
        // 文件名由用例层填（解析不认识文件名）
        source_file: None,
    })
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

/// 成键关系。
///
/// **首版不实现**，返回空。原因：成键判据（共价半径之和 × 容差、跨盒 ghost 原子、
/// 是否做最近邻截断）必须逐条与 `reference/VASPFlow/structure.js` 的
/// `getStructureScene`（385 行、零 IO）核对后才能确定；凭猜写会直接导致 3D 画错。
/// 见 docs/rust-rewrite.md §11.2。
///
/// 调用方拿到空 `bonds` 时应降级为「只画原子、不画键」，而不是报错。
pub fn bonds(_scene: &StructureScene) -> DomainResult<Vec<Bond>> {
    Err(DomainError::NotImplemented("bonds（待与 structure.js 对拍）"))
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
