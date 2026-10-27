//! 晶格几何：矩阵求逆、笛卡尔↔分数转换、最小镜像距离。
//!
//! 这些函数只做数学，不知道 VASP 的任何约定——所以它们既服务于成键判定，
//! 将来也能给 XDATCAR 轨迹、PBC 折回、导出 PDB 复用。

/// 三维向量。
pub type Vec3 = [f64; 3];

/// 3×3 矩阵，按**行**存。
pub type Mat3 = [[f64; 3]; 3];

/// 晶格矢量 → 矩阵。
///
/// POSCAR 前三行是 `a`、`b`、`c`，笛卡尔坐标由 `pos = f_a·a + f_b·b + f_c·c` 得到，
/// 因此三个矢量按**列**摆放：
///
/// ```text
///      | a_x  b_x  c_x |
///  M = | a_y  b_y  c_y |
///      | a_z  b_z  c_z |
/// ```
pub fn lattice_matrix(cell: &[[f64; 3]; 3]) -> Mat3 {
    [
        [cell[0][0], cell[1][0], cell[2][0]],
        [cell[0][1], cell[1][1], cell[2][1]],
        [cell[0][2], cell[1][2], cell[2][2]],
    ]
}

/// 3×3 求逆（伴随矩阵法）。奇异矩阵返回 `None`——共面/共线的晶格是坏数据，
/// 调用方应该降级而不是除出一个 `inf`。
pub fn invert3x3(m: &Mat3) -> Option<Mat3> {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;

    let ca = e * i - f * h;
    let cb = -(d * i - f * g);
    let cc = d * h - e * g;
    let det = a * ca + b * cb + c * cc;
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let s = 1.0 / det;

    Some([
        [ca * s, (c * h - b * i) * s, (b * f - c * e) * s],
        [cb * s, (a * i - c * g) * s, (c * d - a * f) * s],
        [cc * s, (b * g - a * h) * s, (a * e - b * d) * s],
    ])
}

/// 矩阵乘向量。
pub fn mul_mat_vec(m: &Mat3, v: &Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// 笛卡尔 → 分数坐标。
pub fn to_fractional(inv: &Mat3, position: &Vec3) -> Vec3 {
    mul_mat_vec(inv, position)
}

/// 两点间的**最小镜像距离**（Å）。
///
/// 做法：把分数坐标差平移整数个晶格矢量，取所有候选里最短的那个。
/// 这里搜 `{-1,0,1}³` 共 27 个像——对任何夹角不至于极端的晶胞都够用；
/// 夹角极小时（例如 c 轴特别短）需要扩到 `{-2..2}³`，首版不做。
///
/// 跨界成键的正确性完全依赖这个函数：两个原子在盒子两侧、笛卡尔距离很远，
/// 但分数坐标只差 0.02，它们**是**相邻原子。
pub fn min_image_distance(fa: &Vec3, fb: &Vec3, lattice: &Mat3) -> f64 {
    let mut best_sq = f64::INFINITY;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                let d = [
                    fb[0] + i as f64 - fa[0],
                    fb[1] + j as f64 - fa[1],
                    fb[2] + k as f64 - fa[2],
                ];
                let cart = mul_mat_vec(lattice, &d);
                let r2 = cart[0] * cart[0] + cart[1] * cart[1] + cart[2] * cart[2];
                if r2 < best_sq {
                    best_sq = r2;
                }
            }
        }
    }
    best_sq.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverts_identity() {
        let i: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let inv = invert3x3(&i).unwrap();
        assert!((inv[0][0] - 1.0).abs() < 1e-12);
        assert!(inv[0][1].abs() < 1e-12);
    }

    #[test]
    fn rejects_singular() {
        // 三行共面 → 行列式为 0
        let m: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
        assert!(invert3x3(&m).is_none());
    }

    #[test]
    fn cartesian_roundtrip() {
        let cell = [[3.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 5.0]];
        let m = lattice_matrix(&cell);
        let inv = invert3x3(&m).unwrap();
        let pos = [1.5, 2.0, 2.5];
        let frac = to_fractional(&inv, &pos);
        assert!((frac[0] - 0.5).abs() < 1e-12);
        assert!((frac[1] - 0.5).abs() < 1e-12);
        assert!((frac[2] - 0.5).abs() < 1e-12);
        let back = mul_mat_vec(&m, &frac);
        assert!((back[0] - pos[0]).abs() < 1e-12);
    }

    #[test]
    fn min_image_handles_crossing() {
        // 10 Å 的立方盒子。f=0.02 与 f=0.98 的原子在盒子两侧，
        // 直接算笛卡尔距离是 9.6 Å，最小镜像距离应为 0.4 Å。
        let cell = [[10.0, 0.0, 0.0], [0.0, 10.0, 0.0], [0.0, 0.0, 10.0]];
        let m = lattice_matrix(&cell);
        let d = min_image_distance(&[0.02, 0.5, 0.5], &[0.98, 0.5, 0.5], &m);
        assert!((d - 0.4).abs() < 1e-9, "期望 0.4，实得 {d}");
    }

    #[test]
    fn min_image_is_symmetric() {
        let cell = [[4.0, 0.0, 0.0], [0.3, 5.0, 0.0], [0.2, 0.4, 6.0]];
        let m = lattice_matrix(&cell);
        let a = [0.1, 0.9, 0.3];
        let b = [0.95, 0.05, 0.7];
        let d1 = min_image_distance(&a, &b, &m);
        let d2 = min_image_distance(&b, &a, &m);
        assert!((d1 - d2).abs() < 1e-9);
    }
}
