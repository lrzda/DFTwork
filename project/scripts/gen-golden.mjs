// 用 reference/ 里的原 JS 实现生成对拍基准（golden JSON）。
//
// 用法：node scripts/gen-golden.mjs <项目根目录> [--out <输出目录>]
// 产物：tests/golden/<相对路径转下划线>.json（默认）
//
// 这个脚本是「重写后行为不变」的唯一裁判：它跑的是**旧**实现，
// Rust 侧跑同一批真实目录，两边逐字段比对（crates/vasp-cli/tests/parity.rs）。
//
// 注意：本仓库不携带任何 VASP 样例数据，需指向你本机真实的计算目录。

import { mkdirSync, writeFileSync, readdirSync, statSync, readFileSync } from "node:fs";
import { dirname, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..");

const args = process.argv.slice(2);
const rootArg = args.find((a) => !a.startsWith("--"));
if (!rootArg) {
  console.error("用法: node scripts/gen-golden.mjs <项目根目录> [--out <输出目录>]");
  console.error("例:   node scripts/gen-golden.mjs D:/projects/my-vasp-project");
  process.exit(1);
}
const scanRoot = resolve(rootArg);
const outFlag = args.indexOf("--out");
const goldenDir = outFlag >= 0 && args[outFlag + 1]
  ? resolve(args[outFlag + 1])
  : join(repoRoot, "tests", "golden");

const parser = await import(new URL("../reference/VASPFlow/parser.js", import.meta.url).href);
mkdirSync(goldenDir, { recursive: true });

// 找到所有「含 OUTCAR」的目录 —— 与 domain 的判定口径一致
const taskDirs = [];
(function walk(dir, depth) {
  if (depth > 10) return;
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  if (entries.some((e) => e.isFile() && e.name === "OUTCAR")) taskDirs.push(dir);
  for (const e of entries) {
    if (!e.isDirectory() || e.name.startsWith(".")) continue;
    walk(join(dir, e.name), depth + 1);
  }
})(scanRoot, 0);

if (taskDirs.length === 0) {
  console.error(`在 ${scanRoot} 下没有找到任何含 OUTCAR 的任务目录。`);
  process.exit(1);
}

let ok = 0;
for (const dir of taskDirs) {
  const rel = relative(scanRoot, dir).replace(/[\\/]/g, "_") || "root";
  try {
    const oszicar = join(dir, "OSZICAR");
    const outcar = join(dir, "OUTCAR");
    const [points, maxForces] = await Promise.all([
      safe(() => parser.parseIonicEnergyPoints(oszicar)),
      safe(() => parser.parseMaxForces(outcar)),
    ]);
    const payload = {
      fixture: rel,
      fixture_dir: dir,
      generated_at: new Date().toISOString(),
      generator: "reference/VASPFlow/parser.js",
      ionic_points: points,
      max_forces: maxForces,
    };
    writeFileSync(join(goldenDir, `${rel}.json`), JSON.stringify(payload, null, 2) + "\n", "utf8");
    ok += 1;
    console.log(`ok    ${rel}  离子步 ${points.length}，力段 ${maxForces.length}`);
  } catch (e) {
    console.error(`fail  ${rel}: ${e.message}`);
    process.exitCode = 1;
  }
}

console.log(`\n生成 ${ok} / ${taskDirs.length} 个 golden → ${goldenDir}`);

async function safe(fn) {
  try {
    return await fn();
  } catch {
    return [];
  }
}
