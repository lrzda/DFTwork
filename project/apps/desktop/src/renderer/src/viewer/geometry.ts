import type { Vec3 } from "@dftwork/shared";

/** 3×3 矩阵，按「行」存：（row-major，数学上的行）。 */
export type Mat3 = [Vec3, Vec3, Vec3];

/**
 * 晶格矢量 → 矩阵。
 *
 * POSCAR 前三行是 `a`、`b`、`c` 三个矢量，笛卡尔坐标由
 * `pos = f_a·a + f_b·b + f_c·c` 得到，所以按**列**摆放：
 *
 * ```
 *      | a_x  b_x  c_x |
 *  M = | a_y  b_y  c_y |
 *      | a_z  b_z  c_z |
 * ```
 *
 * 于是 `pos = M · frac`，`frac = M⁻¹ · pos`。
 */
export function latticeMatrix(cell: [Vec3, Vec3, Vec3]): Mat3 {
  return [
    [cell[0][0], cell[1][0], cell[2][0]],
    [cell[0][1], cell[1][1], cell[2][1]],
    [cell[0][2], cell[1][2], cell[2][2]],
  ];
}

/** 3×3 求逆（伴随矩阵法）。奇异矩阵返回 `null`——三斜晶胞也可能退化，不能假设可逆。 */
export function invert3x3(m: Mat3): Mat3 | null {
  const [[a, b, c], [d, e, f], [g, h, i]] = m;

  const A = e * i - f * h;
  const B = -(d * i - f * g);
  const C = d * h - e * g;
  const det = a * A + b * B + c * C;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;

  const inv = 1 / det;
  return [
    [A * inv, (c * h - b * i) * inv, (b * f - c * e) * inv],
    [B * inv, (a * i - c * g) * inv, (c * d - a * f) * inv],
    [C * inv, (b * g - a * h) * inv, (a * e - b * d) * inv],
  ];
}

/** 矩阵乘向量。 */
export function mulMatVec(m: Mat3, v: Vec3): Vec3 {
  return [
    m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
    m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
    m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
  ];
}

/** 笛卡尔 → 分数坐标。 */
export function toFractional(inv: Mat3, position: Vec3): Vec3 {
  return mulMatVec(inv, position);
}

/**
 * 把笛卡尔坐标折回原胞（**wrap**）。
 *
 * 做法：笛卡尔 → 分数 → 每个分量取 `x - floor(x)`（落在 `[0,1)`）→ 转回笛卡尔。
 *
 * 为什么用 `floor` 而不是 `round`：`round` 会把 `[0.5,1.5)` 折到 `[-0.5,0.5)`，
 * 于是有一半原子跑到盒子外面去；`floor` 保证全部落在 `[0,1)`。
 *
 * ⚠️ **副作用，必须知道**：wrap 会把跨越边界的分子/团簇劈成两半。
 * 例如一个分子有一个原子在 `f=0.99`、另一个在 `f=0.01`，wrap 后它们在盒子的
 * 两侧，成键线会横穿整个盒子。专业做法（VESTA、原 VASPFlow）是在盒子外
 * 额外画「镜像原子」而不是把原子硬折回来——那是另一种显示策略，与此开关互补。
 *
 * @param position 笛卡尔坐标（Å）
 * @param cell 三条晶格矢量（Å）
 */
export function wrapToCell(position: Vec3, cell: [Vec3, Vec3, Vec3]): Vec3 {
  const M = latticeMatrix(cell);
  const Minv = invert3x3(M);
  if (!Minv) return position; // 退化晶胞：原样返回，不比乱折好

  const frac = mulMatVec(Minv, position);
  // 1e-9 容差避免 0.9999999 因浮点误差被折成 0.9999999 而非 0
  const wrapped: Vec3 = [
    frac[0] - Math.floor(frac[0] + 1e-9),
    frac[1] - Math.floor(frac[1] + 1e-9),
    frac[2] - Math.floor(frac[2] + 1e-9),
  ];
  return mulMatVec(M, wrapped);
}

/** 最小镜像搜索的结果。 */
export interface MinImageResult {
  /**
   * 最优整数平移 `n`，使得 `(fb + n)` 离 `fa` 最近（分数坐标下的整数个晶格矢量）。
   * 全为 0 表示两个原子本来就在同一个像里。
   */
  offset: Vec3;
  /** 平移后的真实距离（Å）。 */
  distance: number;
}

/**
 * 求两原子之间的最小镜像偏移与距离。
 *
 * `offset = n` 的含义：**B 的镜像原子在 `fb + n` 处，离 A 最近**；
 * 对称地，A 靠近 B 的镜像在 `fa - n` 处。
 *
 * 成键判定与渲染都要用它：
 * - 判定用 `distance`（跨界相邻的原子确实是键）；
 * - 渲染用 `offset`（在盒子外补一个镜像球，键就不会横穿整个盒子）。
 */
export function minImageOffset(
  fa: Vec3,
  fb: Vec3,
  cell: [Vec3, Vec3, Vec3],
): MinImageResult {
  const m = latticeMatrix(cell);
  let bestSq = Infinity;
  let best: Vec3 = [0, 0, 0];

  for (let i = -1; i <= 1; i += 1) {
    for (let j = -1; j <= 1; j += 1) {
      for (let k = -1; k <= 1; k += 1) {
        const d: Vec3 = [fb[0] + i - fa[0], fb[1] + j - fa[1], fb[2] + k - fa[2]];
        const cart = mulMatVec(m, d);
        const r2 = cart[0] * cart[0] + cart[1] * cart[1] + cart[2] * cart[2];
        if (r2 < bestSq) {
          bestSq = r2;
          best = [i, j, k];
        }
      }
    }
  }
  return { offset: best, distance: Math.sqrt(bestSq) };
}

export function isInsideCell(position: Vec3, cell: [Vec3, Vec3, Vec3], tol = 1e-4): boolean {
  const Minv = invert3x3(latticeMatrix(cell));
  if (!Minv) return true;
  const frac = mulMatVec(Minv, position);
  return frac.every((x) => x >= -tol && x <= 1 + tol);
}
