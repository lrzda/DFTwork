# Hermes 集成契约

> 同步于 2026-11-04。本文区分 **当前 H1 文本分析与项目对话** 和 **后续 H2/H3 设计**。
> 历史实测依据见文末及 [m3-probe-findings.md](m3-probe-findings.md)。契约变更必须同步实现和测试。

## 0. 原则与当前范围

1. **可选**：Hermes 缺席不应影响扫描、结构和属性查看。
2. **当前无文件/终端工具面**：任务分析 CLI 注入任务事实；项目对话 UI 发送用户原始消息。二者都不直接写用户文件。
3. **薄适配**：适配器只负责组装、启动、解析和回传，不另写 VASP 领域判断。
4. **宿主负责超时**：已实测 `--run-budget` 不能用于当前集成，Windows 下由宿主通过 `taskkill /T /F` 结束 Hermes 进程树，不能只结束 `.cmd` 外层。
5. **H2/H3 不计入当前完成度**：插件工具、专用 profile 和 ACP 仍是后续设计。H1 自由对话与会话续接已通过真实 Electron + 真实模型两轮验收。

本轮经用户明确授权执行了两轮真实 Hermes 自由对话；固定任务 `analyze` 未单独发起真实付费请求。其余结论来自 CLI、真实可执行文件探测和真实 Electron 的离线 mock 验证。

## 1. 可执行文件与环境

### 1.1 当前路径解析

Electron Main 启动时：

1. 尝试 `%LOCALAPPDATA%/hermes/hermes-agent/.hermes/bin/hermes.exe`；
2. 尝试同目录 `hermes.cmd`；
3. 都找不到则退回 `hermes`，由 PATH 解析；
4. 若未显式设置，则把结果写入 `HERMES_BIN`，供 Rust 子进程继承。

`hermes-probe`、`analyze` 和 `hermes-chat` 统一按 `--hermes-bin` → `HERMES_BIN` → PATH 中的 `hermes` 解析。Rust 对裸命令名会搜索 PATH，并在 Windows 补全 `.exe` / `.cmd` / `.bat`；路径含空格、PATH 中的 `.cmd` 和不存在的可执行文件均有 CLI 回归。未安装只降级 AI，不影响查看器。

2026-11-04 已通过真实 `vaspcore hermes-probe` 探测到 Hermes `0.22.6+481.gc314285`，并用真实 `chat --help` 核对当前启动参数。适配器的 `supports_stream_json` 仍是固定布尔值，不代表每次启动都完成能力校验。

### 1.2 尚未实现的探测设计

以下能力矩阵属于目标，不是当前功能声明：版本/`--help` 能力缓存、`probedAt`、profile 检测、H1/H2 自动选择和工具面启动校验。当前不能用文档中的矩阵宣称这些能力已落地。

Windows 下需要区分 `.exe`、`.cmd` 与无扩展名 shebang 脚本；Electron/Rust 直接 spawn 时不能假设 Git Bash 的 PATH 和脚本解析行为会自动生效。

## 2. 当前 H1 调用

任务分析和自由对话最终都调用：

```bash
hermes chat \
  --query-file <临时文件> \
  --format stream-json \
  --source tool \
  --quiet \
  --max-turns N \
  --ignore-user-config --ignore-rules \
  -t todo \
  --model <模型> --provider <provider> \
  [--resume <session_id>]
```

DFTwork 对话入口为 `vaspcore hermes-chat <root> <消息文件>`。Runner 将规范化后的当前扫描 root 设置为 Hermes 子进程 cwd；首次成功回答返回 `session_id`，后续消息用 `--resume` 续接。切换到不同项目 root 时，UI 清空消息和会话 ID，新消息不再携带旧 `--resume`。

聊天正文与 `session_id` 只保存在 `AiPanel` 的 React 状态中；新对话、切换项目、页面重载和关闭窗口会清空。localStorage 只保存模型名与提供方设置，暂不实现 DFTwork 聊天历史持久化。Hermes 自己的会话存储、续接状态和服务端日志不属于 DFTwork 的控制范围，`--source tool` 只负责会话来源过滤，不表示 Hermes 端零存储。

要点：

- 用户消息由 Electron 写入随机命名的临时 UTF-8 文件，调用结束后删除；内核传递消息时也不拼进命令行。临时文件用于单次传参，不是聊天历史；进程异常退出可能留下临时文件；
- `--ignore-user-config --ignore-rules` 用于隔离用户人格、记忆和技能；模型和提供方在 UI 与 CLI 都必须显式给出；
- `--source tool` 使集成会话不进入用户的普通 Hermes 会话列表；
- 对话请求 `-t todo`，DFTwork 不提供文件写入或终端工具；cwd 不等于 Hermes 自动看到了项目文件内容。2026-11-04 实测 `tools list` 即使指定 `-t todo` 仍列出完整静态清单，因此该清单不能作为实际运行工具面的验收证据；
- 不使用 `--run-budget`；内核默认超时 180 秒，Electron 外层调用超时 300 秒；Windows 超时和坏流清理会结束整个子进程树；
- API Key 由用户自己的 Hermes 配置提供，DFTwork 不持有或分发 Key。

## 3. stdout JSONL 事件协议

| type | 关键字段 | 用途 |
|---|---|---|
| `system`（`subtype=init`） | `session_id`、`model`、`timestamp` | 初始化信息 |
| `text` | `text`、`timestamp` | token 级文本片段，必须拼接 |
| `tool_use` | `name`、`tool_call_id`、`input` | H2 才需要；当前 H1 不主动给工具 |
| `tool_result` | `name`、`tool_call_id`、`output`、`duration_ms`、`is_error` | H2 工具结果关联 |
| `result` | `session_id`、`exit_code`、`text`、`tokens`、`duration_ms` | 终止、错误和用量判定 |

解析要求：

1. stdout 与 stderr 分开处理；历史实测 stdout 为 JSONL，`Session:` 等日志在 stderr。当前 Runner 丢弃 stderr，不把其混入 JSONL。
2. `text` 是大量碎片，必须拼接成一条回答；当前 UI 在命令完成后一次性追加回答，尚未实现 token 流式渲染。
3. 同时检查 `result.exit_code` 和进程退出状态；模型失败不能因进程正常退出而误报成功，进程失败也不能被 `result.exit_code=0` 掩盖。
4. 工具结果若在未来启用，应按 `tool_call_id` 关联，而不是按工具名称配对。
5. 没有 `result`、进程启动失败、超时和非零退出均以失败呈现；非空坏 JSONL 行返回 `EXTERNAL`，不能忽略后伪装成成功。

## 4. 任务事实与 Prompt

`analyze` 保留“DFTwork 收集事实，Hermes 只解释事实”的固定分析方向：

```text
scan / convergence / scf / vibrations / forces / magnetization 等 JSON
  → 组装任务事实与用户 intent
  → hermes chat
  → 结论、证据、建议、缺数据
```

项目对话 `hermes-chat` 不套用上述固定问题，也不自动附加任务事实，而是原样传递用户消息。由于当前没有文件读取工具，Hermes 的 cwd 虽为项目 root，但不会因此自动看到 OUTCAR、INCAR 等文件内容；需要项目数据时必须由后续受控上下文或 H2 工具显式提供。

固定分析的 prompt 应继续遵守：

- 只依据提供的事实，不编造缺失数据；
- 结论引用具体字段或输入证据；
- 建议不得声称已经执行；
- 输出简体中文，并区分观察、推测与建议。

早期 `AnalysisBundle v1` 的 12 KB 上限、首 50 + 末 150 点、excerpt ≤ 400 字符、`missing`/`limits` 字段和会话轮转表属于目标契约，**不能在没有序列化与契约测试佐证时视为全部已实现**。补齐时应为大小、裁剪和缺数据行为增加固定夹具。

## 5. H2 工具面：目标设计

H2 计划使用专用 `dftwork` profile 和 Hermes 插件，把只读/提案工具转调同一个 `vaspcore.exe`：

| 工具类别 | 例子 | 约束 |
|---|---|---|
| 只读 | `vasp_scan`、`vasp_convergence`、`vasp_structure_scene`、`vasp_read_file` | 限制 root、深度和输出大小 |
| 输入检查/发现 | `vasp_check_inputs`、`vasp_discover_inputs`、`vasp_scan_templates` | 对应 core 能力实现后再开放 |
| 提案 | `vasp_build_inputs` | 只返回 BuildPlan，不提供 apply 入口 |
| 禁止 | `terminal`、`write_file`、`patch`、浏览器/代码执行等默认工具 | 通过实际工具面校验，不靠 prompt 自觉 |

`-t/--toolsets` 已实测不是可靠白名单。H2 启用前必须验证专用 profile 对 CLI 子进程确实生效，并用能反映最终运行配置的接口检查工具面；不满足时只能回退 H1。当前版本的 `tools list` 会显示静态完整清单，不能单凭它验证 `-t` 已限制实际运行工具。

`read_file` 的 cwd 边界、并发写 Hermes `state.db`、插件如何绑定 core 绝对路径等仍需最小实测，不应凭印象实现。

## 6. BuildPlan 与 AI 的边界

目标原则不变：AI 可以生成变更提案，但落盘必须经过软件 UI 的 diff 和用户放行，AI 工具面不得直接暴露 apply。

P0 已落地 root 与前像绑定、全量预检、原子写、回滚后像校验、pending/回执找回、恢复 journal、备份保留和真实 UI apply→rollback 验收。软件侧 `build` 会保存 pending 计划，但仍不修改业务文件；apply 只允许由用户在软件界面放行。因而：

- H1 仍不让 AI 调用构建或写文件，自由对话也不会自动附带项目文件内容；
- P0 完成只是 H2 提案面的前置条件，不代表 Hermes 插件、profile 工具面白名单或启动校验已经实现；
- ADR-009 的 pending、回执和备份保留已经在软件主流程实现，但不是 Hermes 直接可调用的工具；
- AI 不得直接调用 apply；H2 若开放 `vasp_build_inputs`，仍须只产出提案并交给软件 UI 放行。

详细写入验收见 [roadmap.md](roadmap.md) P0。

## 7. 错误与降级

目标错误分类：

| code | 含义 | 期望行为 |
|---|---|---|
| `HERMES_NOT_FOUND` | 可执行文件不可用 | 仅禁用 AI，说明路径原因 |
| `HERMES_INCOMPATIBLE` | 必需参数或协议不支持 | 回退或提示升级 |
| `PROFILE_MISSING` | H2 profile 不存在 | 回退 H1 |
| `TOOL_SURFACE_VIOLATION` | 实际工具面越界 | 禁止 H2 |
| `SPAWN_FAILED` | 无法启动进程 | 显示系统错误 |
| `TIMEOUT` | 宿主超时 | 终止当前分析，可重试 |
| `NONZERO_EXIT` | 进程或 result 报告失败 | 保留错误文本与日志 |
| `STREAM_PARSE` | JSONL 解析失败 | 记录坏行并按契约降级 |

通用 CLI 的成功与失败信封已统一到 stdout，Electron 保留原始应用错误码。上表中的 Hermes 专用错误码仍是目标：当前探测缺席以 `found=false` 返回，启动失败使用 `EXTERNAL`，超时和模型错误由 `AnalysisReport` 的 `timed_out` / `exit_code` 表达。

## 8. 契约测试

本轮离线验证已覆盖：

1. 自定义 `HERMES_BIN`、路径含空格、PATH 中 `.cmd` 回退和未安装降级；
2. 中文原始消息、项目 cwd、`session_id` 回传与下一轮 `--resume`；
3. 真实 Electron 对话框连续发送、项目 A→B 后 cwd 更新且旧会话不续接；
4. IPC Zod 非法输入拒绝，以及 CLI `CONFLICT` / `VALIDATION` / `NOT_FOUND` 错误码保留；
5. mock `result` 的 tokens、耗时和会话字段展示；
6. 多 `text` 碎片拼接、坏 JSONL 拒绝、进程退出与 `result.exit_code` 不一致，以及 Windows 进程树超时清理。11 项 CLI 契约测试全部通过，修复前已复现对应失败。

上述第 2–5 项的真实 Electron + Rust + mock Hermes 路径由 `apps/desktop/tests/hermes-chat.e2e.cjs` 永久覆盖，并纳入 `pnpm test:desktop`；不会调用真实模型。

2026-11-04 经用户授权，使用真实 Electron 生产主入口、React 页面、Rust 内核和 Hermes 0.22.6 执行两轮最小中文对话，cwd 显示为临时扫描项目 root。第一轮返回 `DFT-731`，3985 tokens / 11.4 秒；第二轮通过 `--resume` 续接并正确返回该标记，4018 tokens / 9.3 秒。页面重载后聊天标记消失，localStorage 只有模型名和提供方两个设置。该真实付费验收的临时驱动已删除，与上述永久离线 mock 回归分开记录。

另用不存在的 provider 做了真实可执行文件的失败协议检查，未请求有效模型；Hermes 0.22.6 返回 `result.exit_code=1`，但文本是泛化的中断提示。DFTwork 保留非零失败状态，不能把这种结果当作可用对话。

固定任务 `analyze` 的真实付费调用未单独执行。H2 仍需增加 profile 工具面、只读 root 边界、提案不修改业务文件和 core 绝对路径测试；静态 `tools list` 不能替代实际工具面验收。

## 9. H3 / ACP 后续接入点

只有在确实需要 AI 直接执行危险操作并逐次请求权限、会话 fork 或即时 cancel 时，才评估 ACP。适配器业务接口可以保持稳定，但任何“允许 AI 直接写”的变化都必须新增 ADR，并继续满足软件侧计划、放行和写入安全约束。

## 10. 历史实测事实

> Hermes v0.22.5 于 2026-10-27 实测；补充记录见 [m3-probe-findings.md](m3-probe-findings.md)（2026-10-28）。这些是有日期的证据，不表示本轮重新运行。

| # | 事实 | 对设计的影响 |
|---|---|---|
| E1 | stdout 是纯 JSONL，`Session:` 在 stderr | 分管读取，失败时检查是否错误合流 |
| E2 | 可见 `system/init → tool_use/tool_result → text… → result`；当时首事件约 3.1 秒 | UI 需要明确的分析中状态 |
| E3 | `-t file` 不是白名单，仍可能看到其他工具 | H2 必须验证实际工具面 |
| E4 | 一次性调用遇危险命令默认 deny，不等待人工批准 | 不依赖 Hermes 原生审批实现软件放行 |
| E5 | `--resume <session_id>` 当时可命中同一会话 | 续接技术上可行，但当前主流程未完成 |
| E6 | `--query-file -` 当时可从 stdin 读取中文 prompt | 避免命令行拼接与临时 prompt 泄漏 |
| E7 | `result` 带 `session_id` 和 tokens | 用量与错误以 result 为依据 |
| E8 | `--run-budget` 虽在 help 中，实际运行被拒绝 | 宿主必须负责超时 |
| E9 | `--ignore-user-config --ignore-rules` 必须显式给模型和 provider | 启动参数需成组校验 |
| E10 | 默认 profile 会加载用户 SOUL、记忆、技能和宽工具面 | H1 默认隔离用户配置，H2 另行收敛 |
