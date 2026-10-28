# 决策记录（ADR）

格式：一条决策一节。状态为 `accepted` 的条目，改动前必须先写新 ADR 并标注它被取代。

---

## ADR-001 独立桌面软件，不内嵌 Hermes

**状态**：accepted ｜ 2026-10-27 ｜ 决策人：用户

**背景**：VASPFlow 现为 DSH 插件形态，受宿主版本与插件 SDK 约束；用户希望"离开 agent 也能正常使用"，且不想替使用者分发 Hermes。

**决策**：做成独立 Windows 桌面软件，**不打包、不内嵌** Hermes。AI 能力是可选增强：用户自装 Hermes 则可用，不装则仅 AI 入口置灰。软件不持有任何模型 API Key。

**后果**：

- 软件可完全离线工作，扫描/收敛/3D/文件浏览零依赖。
- 若要给组内他人使用，"对方需自装 Hermes"是已知门槛；文档需写明。
- 未来若改为内嵌，只需新增一个 `HermesRuntime` 提供者，Adapter 接口不变。

**被否方案**：内嵌 Python 运行时 + hermes-agent（体积数百 MB，版本升级要随软件发版）；只做 Hermes 插件（受宿主 SDK 约束，且离开 Hermes 不可用）。

---

## ADR-002 桌面壳用 Electron

**状态**：accepted ｜ 2026-10-27

**背景**：`lib/host/` 是纯 ESM JavaScript；`src/client/` 是 React 18 + antd + three + recharts。

**决策**：Electron 30+，主进程 Node 20 ESM，electron-vite 构建，electron-builder 打包（NSIS + portable）。

**后果**：

- core 可在主进程**直接 import**，不需要 sidecar、不需要重写。
- 前端 4 个核心组件可直接搬，只需替换 `api.ts` 这一层（282 行）。
- 代价：安装包约 120–160 MB、空载内存约 150–250 MB。对科研桌面软件可接受（VESTA 亦为重型 GUI）。

**被否方案**：Tauri 2（省 130MB，但要么用 Rust 重写 2,000+ 行解析，要么打包 Node sidecar，成本 2–4 周且引入双运行时）；本地 Web + 浏览器（用户明确要"软件"）；PySide6（前端全部重写，three.js 生态断开）。

---

## ADR-003 `vaspcore` 是唯一数据层，并输出 CLI（Rust 实现）

**状态**：accepted ｜ 2026-10-27

**背景**：VASPFlow 的设计原则是"面板与 Agent 共用同一数据层"。独立软件后，若 Hermes 侧用 Python 再写一遍解析，两边必然漂移。

**决策**：

1. 把 VASPFlow 的 host 层按行为重写为 Rust workspace（`vasp-domain` / `vasp-application` / `vasp-infrastructure` / `vasp-cli`）。原 JS 移入 `reference/` 仅作对拍基准。
2. 同时输出 CLI `vaspcore.exe`，所有子命令支持 `--json`。
3. 软件（spawn `vaspcore.exe`）、Hermes 插件（直接调同一二进制）、测试（CLI + 对拍）三方共用。

**后果**：

- 单一事实来源成立；AI 与 UI 的结论不会互相矛盾。
- CLI 冷启动约 100–300ms，可接受；也可 esbuild 打成零依赖单文件，Hermes 侧无需 npm 安装。
- core 不得 import Electron / DSH / React，保持纯 Node。

**被否方案**：软件用 JS、Hermes 用 Python 各写一份（漂移不可避免）；软件暴露本地 HTTP 给 Hermes 调（要求软件常驻，且引入端口与鉴权问题）。

---

## ADR-004 写操作两阶段：AI 提案 → 用户放行 → 软件落盘

**状态**：accepted ｜ 2026-10-27 ｜ 决策人：用户（要求"AI 可读可写，但每次修改必须用户放行"）

**背景**：Hermes 的一次性调用（`-q`）在遇到危险命令时默认 **deny**（见 [hermes-contract.md §10](hermes-contract.md#实测事实) E4），没有交互放行通道；若改走 ACP 才能拿到 permission request，但首版成本高。同时用户要求 AI 具备实际写入能力。

**决策**：

1. AI 拥有**只读面**（扫描、解析、读文件、校验）与**提案面**（`vasp_build_inputs` 等，只产出计划）。
2. 任何落盘由软件 `BuildQueue` 执行：计划 → diff 预览 → 用户放行（绑定 `planHash`）→ 写前备份 → 落盘 → 回执。
3. AI 工具面**不提供** `write_file` / `patch` / `terminal`，从根上阻断"绕开放行直接写"。
4. 放行是一次性、绑定具体计划的，不是全局开关。

**后果**：

- 满足"权限大但每次放行"，且不依赖 Hermes 版本或审批实现。
- 用户体验优于终端审批弹窗：可看 diff、可批量、可回滚。
- 若未来要允许 AI 直接运行 shell（例如提交集群作业），必须新增 ADR 并引入 ACP 的 permission 通道，不得静默放开。

**被否方案**：靠 Hermes 的 `approvals` 机制（CLI 下默认 deny，无法交互）；全局 `--yolo`（违背"每次放行"）；只允许只读（不满足用户对写入能力的要求）。

---

## ADR-005 一期 AI 通道用 CLI 流式；ACP 留作二期

**状态**：accepted ｜ 2026-10-27

**背景**：见 [hermes-contract.md §10](hermes-contract.md#实测事实) E1/E2/E5。

**决策**：

- 一期：`hermes chat --query-file - --format stream-json --source tool --quiet`，按行解析 JSONL，`--resume <session_id>` 续接。
- 二期（按需）：`hermes acp`，用于需要 permission 请求、cancel、fork 的场景。
- **不采用** TUI gateway（内部协议，版本漂移风险大）与 API server（需常驻 gateway，审批能力弱）。

**后果**：

- 冷启动约 3 秒，每轮输入 token 约 17k（见 E1），需要在 UI 上给"分析中"的即时反馈，并避免无意义重跑。
- `stdout` 干净、`stderr` 有日志，管道要分开处理。
- 结束事件 `type:"result"` 自带 `session_id` 与 token 用量，无需另解析。

---

## ADR-006 首版范围 = VASPFlow 既有功能

**状态**：accepted ｜ 2026-10-27 ｜ 决策人：用户

**决策**：首版实现 VASPFlow 已有能力：

- 面板：项目打开、任务扫描/状态、收敛图、3D 结构、文件浏览与预览（500KB 截断）、一键分析
- 12 个 `vasp_*` 工具（含 `build_inputs` / `check_inputs`）
- 3 个 skills 与 persona（二期随 Hermes 插件发布）

**当时计划不做**：SCF 电子步序列、XDATCAR 轨迹播放。

**后续变更**：SCF 电子步已在 M2.5 实现（OSZICAR 流式解析 + 对数残差曲线）。
XDATCAR 仍不做，接口保留。

**后果**：首版工期可控；后续加功能不需要改 IPC 契约（只增字段）。

---

## ADR-007 状态存储用 JSON，不用 SQLite

**状态**：accepted ｜ 2026-10-27

**背景**：需要存最近项目、面板尺寸、会话映射、构建回执，数据量小（< 1MB）。

**决策**：`%APPDATA%/DFTwork/store.json`，写入走"临时文件 + rename"原子替换；项目级视图状态存 `<项目根>/.dftwork/view.json`。

**后果**：无原生模块，安装包与 CI 简单；不支持并发多实例写入（用单实例锁规避）。任务索引若将来超过万级，再评估 SQLite。

**被否方案**：better-sqlite3（Electron ABI 需 electron-rebuild，跨版本易碎）；lowdb（多一层依赖，收益不大）。

---

## ADR-008 AI 工具面白名单：只读 + `vasp_*`

**状态**：accepted ｜ 2026-10-27

**背景**：实测 `-t file` 并不能限制工具面（`browser_*` 仍可见，见 §10 E3）。若放任默认工具集，AI 具备 `terminal` / `write_file`，与 ADR-004 冲突。

**决策**：

1. 为软件建立专用 Hermes profile（如 `dftwork`），在其中设置 `platform_toolsets.cli` 与 `disabled_toolsets`，只保留：`file` 的只读子集（`read_file`/`search_files`）、自定义 `vasp_*` 工具、`skills`。
2. 显式禁用：`terminal`、`browser`、`web`、`image_gen`、`code_execution`、`messaging`、`cronjob`。
3. 启动时用 `hermes tools list` 或 profile 配置校验实际工具面；不匹配则拒绝启动 AI 通道并提示修复。

**后果**：审批模型不依赖 agent 的自觉；即使用户误配，最大风险也限于"读到不该读的文件"（见风险 R4）。

**被否方案**：靠 `-t`（实测无效）；靠系统提示约束（不可强制）。

---

## ADR-009 备份用内容寻址 + 保留上限，不做逐文件时间戳目录

**状态**：accepted ｜ 2026-10-27

**背景**：ADR-004 规定落盘由软件执行。若按最朴素实现（每个覆盖文件一个时间戳目录、全量复制、不清理），高频构建会把项目盘堆到 GB 级——这正是需要提前排除的风险。

**决策**：

1. 一次放行 = **一个批次目录**：`backup/<时间戳>-<planHash 短>/`，内含 `manifest.json` + `blobs/`。
2. blob 以内容 `sha256` 前 16 位命名，**同内容只存一份**。
3. 只备份 `action = overwrite` 的文件；**新建文件不备份**。
4. 默认排除 `POTCAR` / `WAVECAR` / `CHGCAR` / `*.tmp`。
5. 保留策略：每（任务, 文件）5 版、项目上限 500 MB、最长 30 天；清理以批次为单位。
6. 计划最多保留 1 个（`plans/pending.json` 覆盖写）；回执保留最近 100 条。

**后果**：

- 单次写入最坏 1–2 MB，稳态受 500 MB 封顶。重度使用（20 次/天）30 天约 180 MB；脚本反复构建也顶到 500 MB 为止。对照单个 OUTCAR 常 10–100 MB——备份从来不是主要磁盘占用。
- **三类文件分清**：备份（`.dftwork/backup/`，可删，丢了只影响回滚）／状态（`store.json`、`view.json`，可重生成）／临时（`*.tmp`，写完即删）。
- **只读操作不产生任何文件**：扫描、查看收敛、看结构、AI 分析全部零落盘；`pending.json` 是覆盖写，不累积。
- **反面清单（必须避免）**：整目录复制、备份 POTCAR、按时间戳存全量副本、每次操作落一堆独立日志。
- 用户可见：设置页显示当前占用 + 「清理备份」按钮；回执里列出本次备份路径。

**被否方案**：每次整目录复制（体积失控）；备份 POTCAR（单次几百 MB）；不设上限（一年 3–5 GB）；计划与日志每次落独立文件（一次操作多 3–5 个文件）。

---

## ADR-010 桌面外壳用 Electron；内核用 Rust 重写

**状态**：accepted ｜ 2026-10-27 ｜ 决策人：用户

**背景**：用户先确定外壳用 Electron，随后在比较「JS 沿用 / Rust / C++ / Go / C# / Zig / Python」后，决定**内核用 Rust 重写一版新的**，原有 JS 代码只作参考。

**决策**：

1. **外壳 Electron**（已定）；前端 React **不重写**（three.js 与 recharts 在原生 UI 无等价物）。
2. **内核用 Rust 重写**：新建 workspace，不复用 JS 代码；原 JS 移入 `reference/`，只读、可运行，用于生成对拍基准。
3. 集成方式：**独立二进制 `vaspcore.exe` + JSON CLI**，Electron 主进程 spawn；首版不做 napi 原生模块（避免 Node ABI 绑定与崩溃带崩主进程）。
4. 分层不变：`vasp-domain`（纯逻辑）← `vasp-application`（用例）← `vasp-infrastructure`（实现 trait）；`vasp-cli` 是唯一组合根。依赖方向由 Cargo 强制。
5. 行为不变靠**对拍**：原 JS 生成 golden JSON，Rust 逐字段比对；差异必须清零或记录为 `known_divergence`。
6. M0 量 Electron 基线；M1 由「搬 JS 模块」改为「Rust 重写 + 对拍」，工期 5.5–7.5 周。

**后果**：

- 安装包主体仍是 Electron 运行时（~85–110 MB），Rust 内核只增加 3–6 MB；换来解析快 3–8×、无 GC、内核与 UI 完全解耦。
- Hermes 插件直接调用 `vaspcore.exe`，不再需要 Node。
- 工期比「JS 沿用」多约 5 周，主要花在重写与对拍。

**依据（当时测算）**：

| 组成 | 体积 | 占比 |
|---|---|---|
| Chromium + V8 + Node | 110–130 MB | ~85% |
| antd + three + recharts | 3–5 MB | ~3% |
| 业务代码（压缩后） | 0.3–0.6 MB | <1% |
| 图标字体等 | 5–10 MB | ~5% |

**体积由运行时决定，换语言无效**——业务代码全删到零仍是 120 MB。四条路线当时比较过：Electron + JS 85–110 MB（+0）；Tauri + Node sidecar 30–40 MB（+1–2 周）；Tauri + Rust 8–14 MB（+6–10 周）；原生 UI 8–15 MB（+3–4 月）。性能瓶颈在磁盘 IO、GPU 与浏览器渲染，换语言对扫描/3D/交互只有 ±10%，唯 GB 级 XDATCAR 值得单独优化。瘦身靠 locale 裁剪、去 ffmpeg、asar、动态 import，可压到约 90 MB。

**被否方案**：JS 沿用（用户否决，要求 Rust 重写）；napi-rs 首版（ABI 与崩溃隔离问题）；C++（维护成本）、Go（进程往返 + 体积）、C#（跨平台与体积）、Zig（生态）、Python（分发与启动）作内核。

**详见**：[rust-rewrite.md](rust-rewrite.md)

---

## ADR-011 渲染抽象成 SceneRenderer 接口，界面不直接依赖 three.js

**状态**：accepted ｜ 2026-10-27 ｜ 决策人：用户（要求画质可控并预留自定义渲染）

**决策**：

1. 定义 `SceneRenderer` 接口：`mount / load / setStyle / setEffect / highlight / playTrajectory / resize / screenshot / dispose`。
2. 首版实现 `ThreeRenderer`（three.js 0.160）；界面组件只依赖接口，不直接 `new THREE.Scene`。
3. 渲染器注册表 + 配置项：`renderer.impl / style / effect / quality`。
4. 未来可加 `WebGpuRenderer`、`RustWgpuRenderer`、`NullRenderer`（单测），上层不改。
5. 画质基线：MSAA、sRGB 输出、`devicePixelRatio ≤ 2`、原子用 `InstancedMesh`、球体分段按原子数自适应、选中用 `OutlinePass`。

**后果**：画质可独立迭代；换壳（Electron ↔ Tauri）不影响渲染层；等值面、轨迹动画、自定义着色器都只是新增实现。详见 [architecture.md §5.4](architecture.md#54-presentation)。

**被否方案**：界面直接操作 three.js（换渲染器要改所有组件）；只做固定画质（无法扩展等值面/着色器）；用原生 UI 画 3D（等价物从零）。
