# DFTwork

> A Windows desktop workbench for inspecting, diagnosing and visualizing VASP calculations.

VASP 计算任务工作台（Windows 桌面软件）。扫描项目目录，识别任务状态，并展示结构、能量、力、频率、磁矩与轨迹。

## 当前状态

**查看器主流程、P0 写入安全、H1 自由对话与本机发布包验收已完成。** pending 计划、回执、备份 manifest 和崩溃恢复日志会持久化到项目 `.dftwork/`，普通 UI 的 apply→重载找回→rollback 已实测通过。Hermes 自由对话已完成真实两轮续接验证；DFTwork 聊天仅存内存，重载后清空。桌面单实例锁已通过真实双进程验收，但不覆盖外部 CLI 并发写入。备份受 5 版 / 500 MB / 30 天上限约束，仍不能作为重要数据的唯一副本。

当前源码、构建和测试状态以 [docs/roadmap.md](docs/roadmap.md) 为准；目标架构与已实现能力的区别见 [docs/architecture.md](docs/architecture.md)。

## 发布包与源码构建

发布页：[Releases](https://github.com/lrzda/DFTwork/releases/latest)。当前 0.2.0 NSIS / portable 包已在本机隔离目录重建、安装或启动，完成真实扫描并核对包内内核与许可文件；测试安装和隔离用户目录已清理。本机验收不代表其他干净 Windows 机器或代码签名验收。

从源码运行需要 Node.js、pnpm 和 Rust；运行查看器不需要 Python、VASP 或 Hermes。Hermes 只用于可选 AI 分析。

```bash
cd project

# 1. 编内核
cargo build -p vasp-cli
./target/debug/vaspcore.exe scan "D:/你的/VASP项目"

# 2. 编界面并启动
pnpm install
pnpm --filter @dftwork/desktop build
node_modules/.pnpm/electron@*/node_modules/electron/dist/electron.exe apps/desktop/out/main/index.js
```

如果 Rust 安装在非默认位置，需先设置本机的 `RUSTUP_HOME`、`CARGO_HOME` 和 `PATH`。

无界面验证（自动扫描 + 截图 + 退出）：

```bash
DFTWORK_SMOKE=1 \
DFTWORK_SMOKE_ROOT="D:/你的/VASP项目" \
DFTWORK_SMOKE_DIR="$PWD" \
node_modules/.pnpm/electron@*/node_modules/electron/dist/electron.exe apps/desktop/out/main/index.js
```

可加 `DFTWORK_SMOKE_PANEL=scf|vibration|force|magnetization|compare`、`DFTWORK_SMOKE_TASK=<相对路径>`、`DFTWORK_SMOKE_COMPARE=<rel1>,<rel2>` 指定截图状态。

## 自动化验证

先完成上述 debug 内核构建和 `pnpm install`，再运行：

```bash
pnpm test:desktop
```

该命令构建生产 Electron 页面，并运行 4 组永久回归：写入恢复/回滚、桌面单实例、四类过期响应保护，以及 Hermes 对话与项目切换。写入和 Hermes 场景使用真实 Rust 内核；竞态场景使用真实生产 React/preload 与可控模拟 IPC，不代表真实后端延迟测试。测试项目与 Electron 用户目录均隔离在临时目录，结束后自动清理，不调用真实 Hermes。

`.github/workflows/ci.yml` 已配置 Windows CI，执行 golden 重建校验、Rust 全量测试/检查、TypeScript typecheck、Electron 构建与上述端到端测试。CI 远端首次运行尚未验收。

## 当前已有能力

| 能力 | CLI | 界面 | 当前限制 |
|---|---|---|---|
| 扫描项目、判定状态与任务类型 | `scan` | 任务表 + 统计看板 | 一次性扫描，尚无增量事件流 |
| 离子步能量 + 最大力 | `convergence` | 收敛曲线 | OSZICAR/OUTCAR 流式读取；OUTCAR 读取失败会报错 |
| SCF 电子步迭代 | `scf` | 对数残差曲线 | 已通过现有测试与构建 |
| 每原子力 + 应力张量 | `forces` | 受力排行 + 应力表 | 首离子步编号已修正 |
| 振动频率 + 力常数矩阵 | `vibrations` | 频谱柱图 + 热图 | 以现有 OUTCAR 数据为限 |
| 总磁矩演化 | `magnetization` | 磁矩曲线 | 逐原子磁矩取决于 OUTCAR 是否有对应表 |
| 3D 结构（成键 / PBC / 镜像原子） | `scene` | three.js 看板 | 渲染器注册表尚未实现 |
| 多任务对比（INCAR diff + 曲线） | `compare` | 差异表 + 叠加图 | 扫描/结构/收敛/对比均有模拟 IPC 永久乱序回归，未覆盖真实后端延迟 |
| 轨迹播放（XDATCAR） | `trajectory` | 逐帧播放器 | 两遍流式抽样并保留末帧；变胞后段仍按首帧晶格渲染 |
| 构建、apply、rollback | `build` / `apply` / `rollback` / `project-state` / `recover` | diff + 放行 + 历史回执 + 崩溃恢复 | P0 持久化、保留策略、故障注入及真实 UI 往返验收通过 |
| Hermes 任务分析与自由对话 | `analyze` / `hermes-chat` / `hermes-probe` | 项目级对话框 | 以当前扫描 root 为 cwd，支持会话续接；聊天仅存内存，H2 插件工具面未实现 |

除 help/version 外，CLI 成功和失败都在 stdout 输出一行 JSON 信封，stderr 只放日志；Electron 保留 `CONFLICT` / `VALIDATION` / `NOT_FOUND` 等原始错误码。IPC 边界使用 Zod 校验请求，非法输入不会启动内核。

## 定位

- **独立运行**：扫描、判定、解析和可视化均在本地完成，不依赖 AI Agent。
- **AI 可选**：用户自行安装 [Hermes Agent](https://hermes-agent.nousresearch.com/docs/) 后可发起文本分析；未安装时不影响查看器。
- **数据同源**：UI 与 AI 使用 `vaspcore` 的 Rust 解析结果，避免分别实现领域判断。
- **两阶段写入**：AI 只能给建议；软件构建经过 pending 计划、diff、用户放行、写前备份、回执和可验证回滚。P0 恢复与保留策略已实现，但不支持多实例并发写同一项目。

## 内核实现要点与已知边界

- **大文件不能只读尾部**：频率、磁矩等数据可能位于 OUTCAR 中部。`convergence` 的 OSZICAR/OUTCAR 已改为流式读取，OUTCAR 存在但读取失败会显式报错。
- **成键判据**：当前采用共价半径和 × 1.2 + 最小镜像距离；PBC 开关同时影响原子位置与成键判断。
- **跨界键画镜像原子**：避免把周期边界的键画成横穿晶胞的长线。
- **VASP 频率输出不是统一升序**：实频与虚频的排列需要分别处理。
- **逐原子磁矩缺失时返回原因**：`per_atom_available=false` 不等于所有原子磁矩为零。
- **轨迹读 XDATCAR**：两遍流式扫描只保留最多 600 个样本，`total_frames` 统计完整输入并保留末帧。播放器复用首帧场景，只更新位置；变胞后段仍按首帧晶格渲染。
- **热力学量不在当前范围**：完整振动自由能需要 phonopy 等额外工具与模型。

## 来源与许可

基于 [VASPFlow](https://github.com/21271122/VASPFlow)（MIT）的 host 数据层与前端组件改造。原 JS 代码位于 `project/reference/VASPFlow/`，只作行为参考与对拍来源，不参与构建。

上游完整 MIT 许可及 `Copyright (c) 2026 VASPFlow contributors` 已保存在 `project/reference/VASPFlow/LICENSE`，根目录 `NOTICE` 记录来源与版权关系。DFTwork LICENSE、NOTICE 和 VASPFlow LICENSE 已进入 `resources/licenses/`，并已在解包目录和 NSIS 安装目录与源文件核对一致。

## 文档

| 文件 | 讲什么 |
|---|---|
| [docs/roadmap.md](docs/roadmap.md) | **当前状态与计划唯一正文**：P0–P3、验收条件、已验证基线 |
| [docs/architecture.md](docs/architecture.md) | 当前进程/分层/数据流，以及与目标安全边界的差距 |
| [docs/decisions.md](docs/decisions.md) | ADR 001–011；accepted 表示设计决策，不表示实现验收 |
| [docs/rust-rewrite.md](docs/rust-rewrite.md) | Rust workspace、CLI、对拍现状与剩余行为规格 |
| [docs/hermes-contract.md](docs/hermes-contract.md) | 当前 H1 契约与 H2 目标设计 |
| [docs/m3-probe-findings.md](docs/m3-probe-findings.md) | Hermes 历史实测记录 |
| [CONTEXT.md](CONTEXT.md) | 术语定义，不承担进度说明 |

## 目录结构

```text
DFTwork/
├── docs/
├── project/
│   ├── apps/desktop/          Electron 主进程 + preload + 渲染进程
│   ├── crates/
│   │   ├── vasp-domain/       模型、解析、判定、几何与策略
│   │   ├── vasp-application/  用例编排 + DTO + Fs 端口
│   │   ├── vasp-infrastructure/ 文件 / Hermes 子进程 / 时钟 / 日志
│   │   └── vasp-cli/          vaspcore.exe
│   ├── packages/shared/       TypeScript DTO + IPC 常量
│   ├── reference/VASPFlow/    原 JS（只读参考）
│   └── tests/                 跨包集成与对拍
└── CONTEXT.md
```

## 验证基线

2026-11-06 依赖、版本、许可与本机发布验收后：

- npm workspace、desktop、shared 和 Rust workspace 版本统一为 **0.2.0**，`vaspcore --version` 输出 `vaspcore 0.2.0`；repository 已统一指向 `lrzda/DFTwork`；
- Electron 固定为 42.11.16，electron-builder / electron-vite / Vite / Zod 已升级，旧 minimatch 链按版本范围修复；生产与全量 `pnpm audit` 均为 **No known vulnerabilities found**；
- `cargo test --workspace --locked`：81 domain + 13 application + 2 infrastructure + 1 parity + 31 CLI 集成测试，共 **128 项通过**；仍有 2 个原有的未使用代码警告；
- shared、node、web 三组 TypeScript typecheck 和 Electron 42.11.16 / electron-vite 6.0.0 / Vite 8.3.5 生产构建通过；
- 升级后重新运行 `apps/desktop/tests/` 的 **4 组永久 Electron 场景**并全部通过：写入恢复/回滚、单实例、四类模拟 IPC 乱序，以及真实 Rust + mock Hermes 对话与项目切换；测试目录由运行器自动清理；
- release 内核和 0.2.0 NSIS / portable 构建成功；解包版、定制目录安装版和 portable 均在隔离用户目录完成启动与扫描，包内内核和三份许可核验通过；测试安装与临时目录已清理，发布产物保留；
- Windows CI 配置已新增，覆盖 golden 重建校验、Rust test/check、TypeScript typecheck、Electron 生产构建和上述永久 E2E；按当前计划暂不提交或运行远端 CI；
- 永久 Hermes mock 回归验证自由消息、中文输入、会话续接、项目 A→B 的 cwd 切换与会话重置，以及 IPC 入参校验和错误码保留；另有 PATH 中 `.cmd`、文本碎片、坏 JSONL、进程退出不一致和进程树超时回归；
- 真实 Electron + Rust + Hermes 0.22.6 两轮中文自由对话与续接通过，第二轮正确返回第一轮标记，显示 tokens 和耗时；页面重载后聊天清空，仅保留模型/提供方设置。固定任务 `analyze` 未单独调用真实模型，H2 工具面未实现；
- 永久竞态回归使用真实生产 React/preload 和可控模拟 IPC，覆盖扫描、结构、收敛、对比乱序响应及扫描 loading 保护；修改前四场景均失败，修改后均通过，但不代表真实后端延迟测试；
- 3 个入库合成 fixture 的对拍通过，比较离子步编号、能量和全部最大力值；必需输入缺失会失败，golden 确定性重建无差异并纳入 CI 漂移检查；状态、路径、应力和结构尚未对拍；
- 未在其他干净 Windows 机器验收或配置代码签名，未执行真实付费固定任务分析；未运行 `cargo clippy`、`cargo audit`，格式检查因缺少 rustfmt 组件未能执行。

详细缺口和下一步验收顺序见 [docs/roadmap.md](docs/roadmap.md)。
