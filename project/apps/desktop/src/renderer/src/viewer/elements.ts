/**
 * 元素显示样式：颜色 + 共价半径。
 *
 * 颜色取 Jmol 标准配色（VESTA / Jmol / Avogadro 都用这一套，用户换软件不会认错元素）；
 * 半径取 Cordero 2008 的共价半径（Å），不是范德华半径——范德华半径画出来球太大，
 * 相邻原子会糊成一团，看不出结构。
 *
 * 这份表是**通用化学常识**，与任何具体研究无关。
 */

export interface ElementStyle {
  /** 十六进制颜色。 */
  color: string;
  /** 共价半径（Å），用于换算球的显示大小。 */
  radius: number;
}

/** [符号, 颜色, 共价半径] */
const RAW: ReadonlyArray<readonly [string, string, number]> = [
  ["H", "#FFFFFF", 0.31],
  ["He", "#D9FFFF", 0.28],
  ["Li", "#CC80FF", 1.28],
  ["Be", "#C2FF00", 0.96],
  ["B", "#FFB5B5", 0.84],
  ["C", "#909090", 0.76],
  ["N", "#3050F8", 0.71],
  ["O", "#FF0D0D", 0.66],
  ["F", "#90E050", 0.57],
  ["Ne", "#B3E3F5", 0.58],
  ["Na", "#AB5CF2", 1.66],
  ["Mg", "#8AFF00", 1.41],
  ["Al", "#BFA6A6", 1.21],
  ["Si", "#F0C8A0", 1.11],
  ["P", "#FF8000", 1.07],
  ["S", "#FFFF30", 1.05],
  ["Cl", "#1FF01F", 1.02],
  ["Ar", "#80D1E3", 1.06],
  ["K", "#8F40D4", 2.03],
  ["Ca", "#3DFF00", 1.76],
  ["Sc", "#E6E6E6", 1.7],
  ["Ti", "#BFC2C7", 1.6],
  ["V", "#A6A6AB", 1.53],
  ["Cr", "#8A99C7", 1.39],
  ["Mn", "#9C7AC7", 1.39],
  ["Fe", "#E06633", 1.32],
  ["Co", "#F090A0", 1.26],
  ["Ni", "#50D050", 1.24],
  ["Cu", "#C88033", 1.32],
  ["Zn", "#7D80B0", 1.22],
  ["Ga", "#C28F8F", 1.22],
  ["Ge", "#668F8F", 1.2],
  ["As", "#BD80E3", 1.19],
  ["Se", "#FFA100", 1.2],
  ["Br", "#A62929", 1.2],
  ["Kr", "#5CB8D1", 1.16],
  ["Rb", "#702EB0", 2.2],
  ["Sr", "#00FF00", 1.95],
  ["Y", "#94FFFF", 1.9],
  ["Zr", "#94E0E0", 1.75],
  ["Nb", "#73C2C9", 1.64],
  ["Mo", "#54B5B5", 1.54],
  ["Tc", "#3B9E9E", 1.47],
  ["Ru", "#248F8F", 1.46],
  ["Rh", "#0A7D8C", 1.42],
  ["Pd", "#006985", 1.39],
  ["Ag", "#C0C0C0", 1.45],
  ["Cd", "#FFD98F", 1.44],
  ["In", "#A67573", 1.42],
  ["Sn", "#668080", 1.39],
  ["Sb", "#9E63B5", 1.39],
  ["Te", "#D47A00", 1.38],
  ["I", "#940094", 1.39],
  ["Xe", "#429EB0", 1.4],
  ["Cs", "#57178F", 2.44],
  ["Ba", "#00C900", 2.15],
  ["La", "#70D4FF", 2.07],
  ["Ce", "#FFFFC7", 2.04],
  ["Pr", "#D9FFC7", 2.03],
  ["Nd", "#C7FFC7", 2.01],
  ["Pm", "#A3FFC7", 1.99],
  ["Sm", "#8FFFC7", 1.98],
  ["Eu", "#61FFC7", 1.98],
  ["Gd", "#45FFC7", 1.96],
  ["Tb", "#30FFC7", 1.94],
  ["Dy", "#1FFFC7", 1.92],
  ["Ho", "#00FF9C", 1.92],
  ["Er", "#00E675", 1.89],
  ["Tm", "#00D452", 1.9],
  ["Yb", "#00BF38", 1.87],
  ["Lu", "#00AB24", 1.87],
  ["Hf", "#4DC2FF", 1.75],
  ["Ta", "#4DA6FF", 1.7],
  ["W", "#2194D6", 1.62],
  ["Re", "#267DAB", 1.51],
  ["Os", "#266696", 1.44],
  ["Ir", "#175487", 1.41],
  ["Pt", "#D0D0E0", 1.36],
  ["Au", "#FFD123", 1.36],
  ["Hg", "#B8B8D0", 1.32],
  ["Tl", "#A6544D", 1.45],
  ["Pb", "#575961", 1.46],
  ["Bi", "#9E4FB5", 1.48],
  ["Po", "#AB5C00", 1.4],
  ["At", "#754F45", 1.5],
  ["Rn", "#428296", 1.5],
  ["Fr", "#420066", 2.6],
  ["Ra", "#007D00", 2.21],
  ["Ac", "#70ABFA", 2.15],
  ["Th", "#00BAFF", 2.06],
  ["Pa", "#00A1FF", 2.0],
  ["U", "#008FFF", 1.96],
  ["Np", "#0080FF", 1.9],
  ["Pu", "#006BFF", 1.87],
  ["Am", "#545CF2", 1.8],
  ["Cm", "#785CE3", 1.69],
  ["Bk", "#8A4FE3", 1.6],
  ["Cf", "#A136D4", 1.6],
  ["Es", "#B31FD4", 1.6],
  ["Fm", "#B31FBA", 1.6],
  ["Md", "#B30DA6", 1.6],
  ["No", "#BD0D87", 1.6],
  ["Lr", "#C70066", 1.6],
];

const TABLE = new Map<string, ElementStyle>(
  RAW.map(([symbol, color, radius]) => [symbol, { color, radius }]),
);

/** 未知元素的兜底样式：中性灰 + 中等半径，绝不报错。 */
const FALLBACK: ElementStyle = { color: "#C8C8C8", radius: 1.2 };

/** 取元素样式。符号大小写不敏感（POSCAR 里偶尔出现 `si` / `SI`）。 */
export function elementStyle(symbol: string): ElementStyle {
  const key = symbol.charAt(0).toUpperCase() + symbol.slice(1).toLowerCase();
  return TABLE.get(key) ?? FALLBACK;
}

/** 场景里出现的元素 → 图例条目（按首现顺序去重）。 */
export function legendOf(symbols: readonly string[]): Array<{ symbol: string; color: string }> {
  const seen = new Set<string>();
  const out: Array<{ symbol: string; color: string }> = [];
  for (const s of symbols) {
    if (seen.has(s)) continue;
    seen.add(s);
    out.push({ symbol: s, color: elementStyle(s).color });
  }
  return out;
}
