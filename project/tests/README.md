# tests

对拍输入与基准。默认测试只依赖本目录中的小型合成 VASP 输出，不需要本机真实计算项目。

## 目录约定

```text
tests/
├── parity-fixtures.json   必需 fixture、输入目录和 golden 的显式清单
├── fixtures/              入库的合成 OSZICAR / OUTCAR
└── golden/
    ├── reference/         入库的确定性旧实现基准
    └── local/             外部真实项目生成的基准（已 gitignore）
```

合成夹具覆盖 OUTCAR 汇总力优先、TOTAL-FORCE 力矩阵回退、空 OSZICAR，以及 Fortran 指数格式。它们不是实际科研数据。

## 重建并运行对拍

在 `project/` 下执行：

```bash
node scripts/gen-golden.mjs
cargo test -p vasp-cli --test parity -- --nocapture
```

无参数生成命令读取 `tests/fixtures/`，重建 `tests/golden/reference/`。生成结果不含时间戳或本机绝对路径，重复运行应无差异。CI 会重新生成并检查基准是否漂移。

测试只读取 `parity-fixtures.json` 列出的项目；缺少清单、输入或 golden，以及文件读取或 JSON 解析失败都会直接失败，不会静默跳过。当前逐项比较离子步编号、能量和全部最大力值，不代表状态、结构、应力等字段已完成对拍。

golden 只能由显式生成命令更新，测试不会自动改写。更新后必须人工审查 diff；Rust 缺陷应修 Rust，不能用新输出覆盖正确基准来消除失败。

## 使用外部真实项目

```bash
node scripts/gen-golden.mjs D:/projects/my-vasp-project
```

外部项目结果默认写入已忽略的 `tests/golden/local/`，不会覆盖入库基准，也不属于默认 CI 的必需输入。

需要新增入库样例时，使用小型合成或充分脱敏的数据，并同步更新 `parity-fixtures.json`。不要提交真实未发表数据；`POTCAR` 受 VASP 许可保护，任何情况下都不要提交。
