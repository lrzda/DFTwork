// 用 reference/ 里的原 JS 实现生成确定性的收敛对拍基准。
//
// 用法：node scripts/gen-golden.mjs [项目根目录] [--out <输出目录>]
// 无参数时重建 tests/fixtures/ 对应的 tests/golden/reference/*.json。
// 指定外部项目时默认写入已忽略的 tests/golden/local/，不覆盖入库基准。
//
// golden 只由显式运行本脚本更新；测试不会自动重写基准。提交前必须审查 diff。

import { mkdirSync, writeFileSync, readdirSync } from "node:fs";
import { dirname, isAbsolute, join, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..");
const args = process.argv.slice(2);
const rootArg = args[0] && !args[0].startsWith("--") ? args[0] : null;
const scanRoot = rootArg ? resolve(rootArg) : join(repoRoot, "tests", "fixtures");
const outFlag = args.indexOf("--out");
const goldenDir = outFlag >= 0 && args[outFlag + 1]
  ? resolve(args[outFlag + 1])
  : join(repoRoot, "tests", "golden", rootArg ? "local" : "reference");

const parser = await import(new URL("../reference/VASPFlow/parser.js", import.meta.url).href);
mkdirSync(goldenDir, { recursive: true });

const taskDirs = [];
(function walk(dir, depth) {
  if (depth > 10) return;
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch (error) {
    throw new Error(`无法读取 ${dir}：${error.message}`);
  }
  if (entries.some((entry) => entry.isFile() && entry.name === "OUTCAR")) taskDirs.push(dir);
  for (const entry of entries) {
    if (!entry.isDirectory() || entry.name.startsWith(".")) continue;
    walk(join(dir, entry.name), depth + 1);
  }
})(scanRoot, 0);

if (taskDirs.length === 0) {
  console.error(`在 ${scanRoot} 下没有找到任何含 OUTCAR 的任务目录。`);
  process.exit(1);
}

let generated = 0;
for (const dir of taskDirs) {
  const rel = relative(scanRoot, dir).replace(/[\\/]/g, "_") || "root";
  try {
    const [points, maxForces] = await Promise.all([
      parser.parseIonicEnergyPoints(join(dir, "OSZICAR")),
      parser.parseMaxForces(join(dir, "OUTCAR")),
    ]);
    const relativeDir = relative(repoRoot, dir);
    const portableDir = relativeDir.replace(/[\\/]/g, "/");
    const payload = {
      schema_version: 1,
      fixture: rel,
      fixture_dir: portableDir.startsWith("../") || isAbsolute(relativeDir) ? dir : portableDir,
      generator: "reference/VASPFlow/parser.js",
      ionic_points: points,
      max_forces: maxForces,
    };
    writeFileSync(join(goldenDir, `${rel}.json`), `${JSON.stringify(payload, null, 2)}\n`, "utf8");
    generated += 1;
    console.log(`ok    ${rel}  离子步 ${points.length}，力段 ${maxForces.length}`);
  } catch (error) {
    console.error(`fail  ${rel}: ${error.message}`);
    process.exitCode = 1;
  }
}

console.log(`\n生成 ${generated} / ${taskDirs.length} 个 golden → ${goldenDir}`);
