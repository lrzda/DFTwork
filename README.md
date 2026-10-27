# DFTwork

VASP 计算任务工作台（Windows 桌面软件）。设计阶段，代码尚未开始。

## 定位

- **独立运行**：不依赖任何 AI Agent。扫描、状态判定、收敛曲线、3D 结构、文件浏览全部本地完成。
- **AI 可选**：用户自行安装 [Hermes Agent](https://hermes-agent.nousresearch.com/docs/) 后，可对选中任务发起分析；未安装时软件功能完整，仅 AI 入口置灰。
- **数据同源**：软件 UI 与 AI 工具共用同一个 `vaspcore`（Rust 重写，行为对齐 VASPFlow），避免"面板说收敛、AI 说不收敛"。

## 来源与许可

基于 [VASPFlow](https://github.com/21271122/VASPFlow)（MIT）的 host 数据层（`lib/host/`，约 3,900 行）与前端组件（`src/client/`，约 2,900 行）改造为独立桌面应用。
保留原项目版权声明、LICENSE 与 NOTICE；本项目同样以 MIT 发布。

## 已定决策（详见 [docs/decisions.md](docs/decisions.md)）

| # | 决策 |
|---|---|
| 001 | 独立桌面软件，**不内嵌** Hermes；用户自装或不装 |
| 002 | Electron + React 18（最大化复用 VASPFlow 前端） |
| 003 | `vaspcore`（Rust）为唯一数据层，同时输出可执行文件与 CLI |
| 004 | 写操作两阶段：**AI 提案 → 用户放行 → 软件落盘** |
| 005 | 一期 AI 通道用 `hermes chat --format stream-json`；ACP 留作二期 |
| 006 | 首版 = VASPFlow 既有功能；SCF 电子步 / XDATCAR 只留接口 |
| 007 | 状态存储用 JSON 起步，避开原生模块 ABI 问题 |
| 008 | AI 工具面白名单：只读工具 + `vasp_*`，不给 `write_file` / `patch` / `terminal` |
| 009 | 备份用内容寻址 + 保留上限（单次写入最坏 1–2 MB，稳态封顶 500 MB） |
| 010 | 外壳 **Electron** + 内核 **Rust 重写**；前端 React 不重写；原 JS 仅作对拍参考 |

## 文档

| 文件 | 讲什么 |
|---|---|
| [docs/architecture.md](docs/architecture.md) | **架构唯一正文**：进程模型、四条数据流、四层与依赖规则、14 个用例、6 个端口、组合根、测试策略、选型（配 [调用关系图](docs/architecture-calls.png) · [分层图](docs/architecture-layered.png)） |
| [docs/decisions.md](docs/decisions.md) | ADR 001–011；含体积测算、写盘生命周期与备份上限 |
| [docs/rust-rewrite.md](docs/rust-rewrite.md) | Rust workspace、CLI 契约、对拍流程、重写前必须逐条核对的判据 |
| [docs/hermes-contract.md](docs/hermes-contract.md) | AI 契约：命令、事件协议、Bundle schema、放行规则、实测事实 |
| [docs/roadmap.md](docs/roadmap.md) | 里程碑、可测验收、风险、未验证项 |
| [CONTEXT.md](CONTEXT.md) | 术语表（只放定义，不放计划） |

## 目录规划（待建）

```
DFTwork/
├── apps/desktop/            Electron 主进程 + 预加载 + 渲染进程
│   ├── src/main/            组合根：装配用例 + IPC 适配器
│   ├── src/preload/         contextBridge 暴露 window.dftwork
│   └── src/renderer/        由 VASPFlow src/client 移植（Presentation）
├── crates/
│   ├── vasp-domain/         纯逻辑：解析、判定、规则、计划（不读文件）
│   ├── vasp-application/    14 个用例（编排、读文件、DTO）
│   ├── vasp-infrastructure/ 文件 / 存储 / Hermes 子进程 / 日志
│   └── vasp-cli/            vaspcore.exe（组合根 + 子命令）
├── reference/               原 VASPFlow JS（只读，用于生成对拍基准）
├── packages/shared/         横切：DTO + zod schema + 错误码 + 路径工具
├── plugins/hermes-vaspflow/ 二期：Hermes Python 插件（12 工具转调用例）
├── tests/                   跨包集成 + 新旧对拍
└── docs/
```

## 快速状态

| 项 | 状态 |
|---|---|
| 技术路线 | ✅ Electron 外壳 + Rust 内核（ADR-010） |
| 架构正文（运行 + 代码视角） | ✅ 完成（2026-10-27） |
| 决策记录（ADR 001–011） | ✅ 完成 |
| Rust 重写方案 + 行为判据 | ✅ 完成 |
| Hermes 契约（含实测事实） | ✅ 完成 |
| 工程骨架（M0 / R0） | ⬜ 未开始 |
| vasp-domain 解析（R1） | ⬜ 未开始 |
| 前端移植 | ⬜ 未开始 |
