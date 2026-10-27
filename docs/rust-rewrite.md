# Rust 内核重写方案

> 决定（ADR-010）：外壳 **Electron**，前端 **React 不重写**，内核 **Rust 重写**，原 JS 代码只作行为参考。
> 本文定：怎么组织 Rust 工程、怎么集成进 Electron、怎么保证行为不变、工期与风险。

## 1. 一句话方案

**Rust workspace 产出 4 个 crate，编译出一个 `vaspcore.exe`；Electron 主进程 spawn 它、用 JSON 通信。原 JS 代码移入 `reference/`（只读、不参与构建），只用于对拍与查阅。**

```
Electron 主进程 ──spawn──> vaspcore.exe ──JSON──> D:/project/.../OUTCAR
      ↑ 前端 React（不变）              ↑ Hermes 插件也调它
```

## 2. 为什么先做 CLI，不做 napi 原生模块

| 方式 | 优点 | 代价 | 结论 |
|---|---|---|---|
| **独立二进制 + JSON CLI** | 崩溃隔离（Rust 挂了不拖垮 Electron）；调试简单（命令行可直接跑）；Electron 升级不用重编；Hermes 插件与测试共用 | 每次调用约 5–20 ms 进程开销 | **首版采用** |
| napi-rs 原生模块 | 零进程开销、零序列化 | 绑定 Electron 的 Node ABI，升级要重编；Rust panic 可能带崩主进程；调试链路长 | 二期可选 |

进程开销相对于「读几十 MB OUTCAR」可以忽略；而崩溃隔离对科研软件是硬需求。

## 3. Workspace 划分

```
crates/
├── vasp-domain/           # 纯逻辑：解析、判定、规则、计划生成。不读文件
├── vasp-application/      # 用例编排：扫描、查看、检查、构建、分析
├── vasp-infrastructure/   # 实现 domain 定义的 trait：文件、存储、Hermes 子进程、日志
└── vasp-cli/              # 二进制入口（clap）：解析参数 → 调用例 → 输出 JSON
```

依赖方向由 **Cargo 强制**（不是约定）：

- `vasp-domain`：Cargo.toml 里不写任何内部依赖 → 物理上无法引用上层。
- `vasp-application`：只依赖 `vasp-domain`。
- `vasp-infrastructure`：只依赖 `vasp-domain`（实现它的 trait）。
- `vasp-cli`：依赖三者，是唯一的**组合根**（把所有实现装配起来）。

这比 JS 版的 dependency-cruiser 更硬：写错 Cargo.toml 就编译不过。

## 4. 依赖库

| 用途 | crate | 说明 |
|---|---|---|
| 序列化 | `serde` + `serde_json` | CLI 输出、配置、快照 |
| 错误 | `thiserror`（库内）、`anyhow`（CLI 边界） | 领域错误用枚举，入口处统一转 JSON |
| 目录遍历 | `walkdir` 或 `ignore` | 后者自带跳过规则（.git / node_modules） |
| 并行 | `rayon` | 扫描多个目录并行；**不引入 tokio**，同步并行足够 |
| 正则 | `regex` | OUTCAR / OSZICAR 行匹配 |
| 参数 | `clap` 4 | 子命令：scan / convergence / scene / files / read / build / check / discover |
| 日志 | `tracing` + `tracing-subscriber` | 写文件日志，stderr 输出 |
| 时间 | `time`（或 `chrono`） | 时间戳与 ISO 8601 |
| 哈希 | `sha2` | 计划指纹、内容寻址备份 |
| 测试 | `assert_cmd` + `insta`（快照） | CLI 端到端 + JSON 快照 |
| 基准 | `criterion`（可选） | 解析性能回归 |

release 配置：`lto = "thin"`、`strip = true`、`panic = "abort"` → 单二进制约 **3–6 MB**。

## 5. 模块对照（原 JS → Rust）

| 原 JS（`reference/lib/host/`） | Rust 目标 | 说明 |
|---|---|---|
| `scanner.js`（631 行） | `domain/scan/detect.rs` + `infrastructure/fs/walker.rs` | 遍历与判定分开 |
| `parser.js`（185 行） | `domain/convergence.rs` | 文本入参，纯函数 |
| `structure.js`（562 行） | `domain/structure/{parse,bonds,scene}.rs` | 成键算法单独模块 + 夹具 |
| `input-builder.js`（585 行） | `domain/build/{poscar,incar,plan}.rs` | 只产计划，不写盘 |
| `input-checker.js`（365 行） | `domain/validate.rs` | 规则表 + 校验 |
| `incar-validate.js` / `outcar-parse.js` | `domain/{incar,outcar}.rs` | 纯函数 |
| `task-store.js`（228 行） | `domain/index.rs` + `infrastructure/store.rs` | 内存索引 / 持久化分开 |
| `task-files.js` / `scan-templates.js` / `src-inspect.js` | `domain/fs_view.rs` / `domain/templates.rs` / `domain/discovery.rs` | |
| `index.js` 的 12 个工具 | `application/use_cases/*.rs` + `cli/commands/*.rs` | 工具 = 用例的 CLI 出口 |
| `src/client/*`（React） | **不改**，只改 `api.ts` 的调用方式 | 前端保留 |

## 6. 对拍：保证「重写后行为不变」

这是整个工程的安全网，不能省。

```
reference/                    # 原 JS 代码（只读，不参与构建）
tests/
├── fixtures/                 # 真实项目子集 + 合成用例（POSCAR/OSZICAR/OUTCAR/INCAR）
├── golden/                   # 用 Node 跑原实现生成的 JSON 快照
└── parity.rs                 # Rust 跑同一 fixture，逐字段比对 golden
```

流程：

1. `scripts/gen-golden.mjs <项目根目录>`（Node）调用原 JS 函数，对每个**真实任务目录**生成 JSON，写入 `tests/golden/`。样例数据不进仓库，使用时指向本机目录。
2. `cargo test --test parity` 读取同一 fixture，生成本地 JSON，与 golden 逐字段比对。
3. 差异分三类处理：Rust 的 bug（修 Rust）；原实现的 bug（记录到本文 §11.4，并在 golden 里标注 `known_divergence`）；字段顺序/浮点格式（规范化后比较）。
4. CI 里 golden 更新必须人工 review，防止「用错误覆盖正确」。

## 7. 工期

| 阶段 | 内容 | 工期 |
|---|---|---|
| R0 | workspace、CLI 骨架、JSON 契约、golden 生成脚本 | 3–4 天 |
| R1 | domain 解析（POSCAR / OSZICAR / OUTCAR / INCAR / KPOINTS） | 1.5–2 周 |
| R2 | domain 规则（状态判定、成键、输入校验、计划生成） | 1.5–2 周 |
| R3 | application 用例 + infrastructure（walker / store / hermes / log） | 1–1.5 周 |
| R4 | 对拍收口（差异清零或记录）+ 性能调优 | 1 周 |
| **合计** | | **5.5–7.5 周** |

比 JS 版 M1（7–10 天）多约 5 周，但换来：单二进制、无 GC、解析快 3–8×、内核与 Electron 完全解耦。

## 8. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 行为漂移（最可能） | 曲线/状态与旧版不一致 | 对拍 + golden 快照 + 真实项目全量比对 |
| 成键算法（`structure.js` 385 行纯逻辑） | 3D 结构画错 | 单独夹具：多种晶格、元素、周期边界；与原实现逐原子/逐键比对 |
| Windows 路径与编码 | 中文路径、CRLF、GBK 文件 | 全程 `PathBuf` + 显式 UTF-8 解码策略；夹具里放中文路径 |
| 大文件解析 | OUTCAR 几十 MB、XDATCAR GB 级 | 流式读取 + 只读尾部；必要时 mmap |
| Rust 开发速度 | 首版推迟 | 先做 domain 纯函数（最容易、收益最大）；application 用简单结构，不做过度抽象 |
| 编译时间 | 迭代变慢 | workspace 增量编译；`cargo check` 为主 |
| 团队不熟 Rust | 卡住 | 只在不熟悉的地方用最直白写法（避免泛型花活）；CLI 层保持薄 |

## 9. 验收

1. `vaspcore scan <真实项目> --json` 与原 JS 实现输出**逐字段一致**（任务数、状态码、路径、INCAR 摘要）。
2. `cargo test` 全绿，覆盖原 77 项测试对应的行为。
3. `tests/parity` 在 3 个真实项目 + 20 个合成 fixture 上零差异（或差异全部记录为 `known_divergence`）。
4. release 二进制 < 8 MB，冷启动 < 50 ms。
5. Electron 主进程只通过 `vaspcore.exe` 访问内核，`apps/desktop` 不出现任何 Rust 绑定（首版不做 napi）。

## 10. 与原 JS 的关系

- **不复制代码**：JS 只作行为参考（算法步骤、边界条件、输出字段）。
- **保留可运行**：`reference/` 里的 JS 保持可 `node` 运行，用于生成 golden 与排查差异。
- **不再维护**：修 bug 只改 Rust；如果确认是原实现的 bug，记录到本文 §11.4。

## 11. 行为规格：重写前必须逐条核对的清单

原 JS 有 54 个导出符号。下面不是代码清单，是**判据**——每条都要对着 `reference/` 确认答案之后再写 Rust，不能凭直觉。

### 11.1 三类耦合点（原实现的结构问题，重写时消灭）

| 类型 | 表现 | 处理 |
|---|---|---|
| 路径式 API | 约 15 个函数内部自己 `readFile`，无法纯测 | 拆成「文本入参的纯函数」+「application 读文件」 |
| 模块级可变状态 | `scanner.js` 的 `taskCache`、`input-builder.js` 的 `potcarCache`、`task-store.js` 的 `inputCheck` | 移到 infrastructure，键带 mtime/size；domain 无状态 |
| DSH 注册 | `registerOutcarParse(ctx)` / `registerIncarValidate(ctx)` | 丢弃；工具改成 CLI 子命令 |

### 11.2 四个关键函数

| 函数 | 实测 | 为什么关键 | 必须核对的点 |
|---|---|---|---|
| `getStructureScene` | 385 行 / 0 IO | 最大块纯逻辑，可直接进 domain | 晶格→笛卡尔的换算顺序；成键判据（共价半径和×容差，还是固定截断？）；跨盒键是否复制 ghost 原子；输出字段（atoms/bonds/cell）的每项含义 |
| `buildOneTask` | 195 行 / 15 IO | 唯一必须拆「纯计划 + 写盘」的地方 | 哪些输入决定文件内容；POTCAR 按什么顺序拼；INCAR 行内 `;` 的语义；`extraFiles` 来自哪里；`create/overwrite/skip` 怎么判 |
| `buildPoscarWithSd` | 91 行 / 0 IO | SD 旗标展开规则 | 哪些 flag 触发展开；展开后行序与列序 |
| `scanSingleDir` | 66 行 / 9 IO | 任务判定标准 | 哪组文件名算一个 VASP 任务；只有 INCAR、没有 OUTCAR 算不算「未开始」；子目录下探深度 |

### 11.3 必须保留的细节

- 500 KB 预览截断，且不能切在 UTF-8 多字节字符中间。
- OSZICAR 里 `DAV` / `RMM` 电子步行与离子步行的区分。
- OUTCAR 能量取哪一组 `free  energy   TOTEN`（第一组还是最后一组，影响收敛曲线）。
- 三种边界状态的分类：空目录 / 只有输入没输出 / 跑过但 OUTCAR 无正常结束。
- Windows 中文路径、CRLF、可能的 GBK 编码文件。

### 11.4 已知差异登记

重写中发现原实现行为不合预期（即原实现的 bug），**不改 Rust 去迁就**，而是在 `tests/golden/<fixture>.json` 标 `known_divergence: "<理由>"`，并在本节追加一行：日期 / fixture / 原行为 / Rust 行为 / 判断依据。不允许静默 diverging。
