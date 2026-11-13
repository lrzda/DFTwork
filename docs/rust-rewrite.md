# Rust 内核实现与对拍

> 决策见 ADR-010：外壳 Electron，前端保留 React，内核使用 Rust，原 JS 只作行为参考。
> 本文同步于 2026-11-06，记录当前工程形状、对拍基线和仍需遵守的行为规格；修复优先级见 [roadmap.md](roadmap.md)。

## 1. 当前工程形状

Rust workspace 位于 `project/`，由四个 crate 组成并产出一个 `vaspcore.exe`：

```text
crates/
├── vasp-domain/           模型、解析、状态、几何、构建策略与领域端口
├── vasp-application/      用例编排、DTO、AppError 与 Fs 端口
├── vasp-infrastructure/   LocalFs、Hermes、日志和时钟等适配器
└── vasp-cli/              clap 参数、组合与分发、JSON 边界
```

真实依赖方向：

- `vasp-domain` 不依赖其他内部 crate；
- `vasp-application` 依赖 `vasp-domain`；
- `vasp-infrastructure` **同时依赖 `vasp-application` 和 `vasp-domain`**，因为它既要实现 application 声明的 `Fs`，也要实现 domain 的 `Clock` / `Logger` / `Hermes` 等端口；
- `vasp-cli` 依赖前三者。

这仍由 Cargo 在编译期强制，但不能再沿用早期“infrastructure 只依赖 domain”的描述。

## 2. Electron 集成

采用独立二进制 + JSON CLI，不使用 napi：

```text
Renderer → contextBridge → Electron Main → spawn vaspcore.exe → JSON
```

优点是崩溃隔离、命令行可独立调试、Electron 升级不要求重编原生模块，Hermes 与测试也可复用同一二进制。代价是每次调用的进程启动和 JSON 序列化开销。

当前约定：

- 除 help/version 外，成功和失败都在 stdout 输出一行 JSON 信封，stderr 只放日志；
- Electron 保留原始 `CONFLICT` / `VALIDATION` / `NOT_FOUND` 等错误码；
- IPC Main 使用 Zod 校验实际请求，非法输入在启动内核前拒绝；
- IPC 的计划和回执通过临时 JSON、Hermes 消息通过临时 UTF-8 文本传给 CLI，调用结束后删除。

## 3. 当前 CLI 能力

| 类别 | 子命令 |
|---|---|
| 扫描与查看 | `scan`、`convergence`、`scene`、`magnetization`、`scf`、`forces`、`vibrations`、`trajectory`、`compare` |
| 构建写入 | `build`、`apply`、`rollback` |
| 项目写入状态 | `project-state`、`discard-plan`、`recover`、`prune-backups` |
| AI 与系统 | `analyze`、`hermes-chat`、`hermes-probe`、`version` |

早期规划中的 `files` / `read` / `check` / `discover` 等并未全部形成可用 CLI；`pending.rs` 和 `services/stubs.rs` 中的占位不能计为完成。

P0 写入安全验收已完成：`build` 只保存 pending 计划，不修改业务文件；apply 在业务写入前持久化 manifest 与恢复 journal，回执和恢复状态可跨进程找回。保留策略、故障注入和真实 Electron apply→rollback 验收见 [roadmap.md](roadmap.md) P0。备份仍不是独立数据备份的替代品；桌面单实例锁已验收，但外部 CLI 或其他进程并发写同一项目仍不受保护。

## 4. 对拍现状

目标流程仍是：

```text
reference/VASPFlow + fixture
        ↓ scripts/gen-golden.mjs
tests/golden JSON
        ↓ cargo test --test parity
Rust 输出与 golden 比较
```

当前事实（2026-11-04）：

- `cargo test --workspace` 共 **128 项通过**：81 domain、13 application、2 infrastructure、1 parity、31 个 CLI 集成测试（10 个写入安全 + 8 个持久化/故障注入 + 2 个解析回归 + 11 个契约回归）；
- parity 使用 3 个入库合成 fixture，`tests/parity-fixtures.json` 显式列出必需输入和 golden，缺少或读取失败会直接失败；
- `node scripts/gen-golden.mjs` 从旧 JS 实现确定性重建 `tests/golden/reference/`，结果不含时间戳或本机绝对路径；连续重建哈希一致，CI 会检查生成物漂移；
- 当前逐项比较离子步编号、能量和全部最大力值，覆盖汇总力优先、力矩阵回退与空 OSZICAR；状态、路径、结构、应力等字段仍未对拍；
- 因而“128 项全绿”不能写成“原 77 项行为全部覆盖”或“全字段对拍零差异已验收”。

### 对拍收敛要求

1. golden 必须可重建、可审查，并明确哪些夹具是测试必需输入；CI 缺少必需输入时应失败。
2. golden 更新需要人工 review，不能用新输出自动覆盖正确基准。
3. 逐步覆盖任务数、状态码、路径、INCAR 摘要、能量、力值、应力和结构等关键字段。
4. Rust 缺陷应修 Rust；参考实现缺陷应记录 `known_divergence`，包含日期、fixture、两种行为和判断依据。
5. 对拍不能代替本仓库自己的回归测试；本次发现的写入、解析和契约问题都需要直接新增 Rust/CLI 用例。

## 5. 当前验证与未验证项

已验证：

- Rust workspace 与桌面/npm 包版本统一为 **0.2.0**，`vaspcore --version` 输出 `vaspcore 0.2.0`；Cargo.lock 与 pnpm-lock.yaml 已同步；
- `cargo test --workspace --locked`：81 domain + 13 application + 2 infrastructure + 1 parity + 31 CLI 集成测试，共 128 项通过；
- `cargo check --workspace`：通过，有 2 个原有的未使用代码警告；
- TypeScript typecheck、Electron 42.11.16 / electron-vite 6.0.0 / Vite 8.3.5 生产构建和真实 Electron 手动切换项目后生成计划通过；桌面单实例双进程验收通过，第二实例退出且首窗口恢复并聚焦；
- 升级后重新运行 `apps/desktop/tests/` 的 4 组永久 Electron 场景并全部通过：写入恢复/回滚、单实例、四类模拟 IPC 乱序，以及真实 Rust + mock Hermes 的自由对话、cwd、续接、项目切换和 IPC 契约；
- 生产与全量 `pnpm audit` 均无已知漏洞，限定范围的 minimatch override 已生效；
- release 内核及 0.2.0 NSIS / portable 构建成功；解包版、定制目录安装版和 portable 均在隔离用户目录完成启动与扫描，包内内核版本及三份许可已核验，测试安装已卸载；
- 真实 Electron + Rust + Hermes 0.22.6 的两轮中文自由对话通过，第二轮记得第一轮标记，页面显示 tokens 与耗时；重载后聊天清空，仅保留模型/提供方设置；固定任务分析未单独调用真实模型；
- 扫描、结构、收敛和对比乱序永久回归使用真实生产 React/preload 与可控模拟 IPC，不是真实后端延迟测试；PATH 中 `.cmd`、文本碎片、坏 JSONL、进程退出不一致与进程树超时另有 CLI 回归；
- 大 OUTCAR 汇总力值、长轨迹完整计数与抽样均有真实 CLI 回归。

本轮未验证：

- `cargo clippy`、`cargo audit`；`cargo fmt --all -- --check` 因当前 Rust 工具链缺少 rustfmt 组件而未能执行；
- release 二进制体积、冷启动时间和相对旧 JS 的性能倍数；
- 其他干净 Windows 机器的安装与运行、代码签名；
- 真实付费固定任务 `analyze`、H2 插件工具面和工具面白名单。

早期文档中的 release < 8 MB、冷启动 < 50 ms、解析快 3–8× 等仍是性能目标或历史估算，不是本轮实测结论。

## 6. 仍需遵守的行为规格

这些是从 VASPFlow 重写时必须持续核对的判据，不因主体代码已经存在而取消。

### 6.1 分层与耦合

- 领域解析优先接收文本或纯数据，文件读取留在 application/infrastructure；
- 不用模块级可变缓存隐藏扫描、构建和输入检查状态；
- 新能力通过用例与薄 CLI/IPC 出口暴露，不在 Electron 或 Hermes 侧另写一份领域判断；
- `reference/` 不参与构建，修复产品缺陷只改当前实现，并记录有意的行为差异。

### 6.2 关键函数与文件格式

- 结构：晶格到笛卡尔坐标、Direct/Cartesian scale、周期镜像、成键容差和 ghost 原子必须分别有夹具；
- 构建：明确 `create` / `overwrite` / `skip`、POTCAR 顺序、INCAR 行内 `;` 和额外文件来源；计划生成与写盘保持分离；
- 扫描：明确何种目录算任务、只有输入时的状态和最大下探深度；
- 收敛：区分 OSZICAR 的 `DAV` / `RMM` 电子步行与离子步行，确认 OUTCAR 能量块选择；
- 力/应力：离子步编号与实际 OUTCAR 块一致，不能只验证数组长度；
- 轨迹：`total_frames` 必须描述完整输入，抽样覆盖末尾，变胞与 scale 行为有明确输出；
- 文件预览若后续实现，应保持 500 KB 截断且不切断 UTF-8 多字节字符；
- Windows 中文路径、空格、CRLF 和可能的非 UTF-8 输入需要集成夹具。

### 6.3 大文件

不能假设 OUTCAR 只需读尾部，频率、磁矩和力等块可能位于中部。需要流式扫描的命令应使用不会受单行非 UTF-8 字节直接打断的读取策略；不能读取的部分必须显式报告，不能返回貌似有效的空结果。

`convergence` 已改为 OSZICAR/OUTCAR 流式扫描，并有超过 64 MiB 的真实 CLI 回归；OUTCAR 只有不存在时才允许空力数组，存在但读取失败会向上传递错误。

## 7. 与参考实现及许可的关系

- `project/reference/VASPFlow/` 只用于查阅和生成对拍基准，不作为产品运行时依赖；
- 参考实现保持可运行有助于重建 golden，但不承担新功能维护；
- 上游完整 MIT 许可和 `Copyright (c) 2026 VASPFlow contributors` 已保存到 `project/reference/VASPFlow/LICENSE`，根 `NOTICE` 已记录来源与版权关系；
- electron-builder 随包携带 DFTwork LICENSE、NOTICE 和 VASPFlow LICENSE；NSIS 定制目录安装版的 `resources/licenses/` 已与源文件逐字核对一致，解包版与 portable 的启动和扫描也已通过。
