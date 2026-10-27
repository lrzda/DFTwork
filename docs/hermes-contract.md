# Hermes 集成契约

> 本文是软件与 Hermes 之间的**接口规范**。改动这里必须同步改实现与契约测试。
> 实测依据见本文末「实测事实」（Hermes v0.22.5，2026-10-27）。

## 0. 原则

1. **可选**：Hermes 缺席时，软件功能完好；AI 入口置灰并说明原因。
2. **只读优先**：AI 在软件里能"看"，但落盘一律由软件执行（ADR-004）。
3. **薄适配**：Adapter 只做「组装 → 启动 → 解析 → 回推」，不含任何 VASP 业务逻辑。
4. **可探测**：启动时探测能力，不假设版本。

---

## 1. 定位与探测

### 1.1 可执行文件

按顺序尝试，第一个通过 `--version` 的即采用：

1. 用户配置的绝对路径（`store.json → hermes.path`）
2. `hermes.exe`（Windows 原生入口，优先于 shim）
3. `hermes.cmd` / `hermes.bat`
4. `hermes`（PATH 中的 shebang 脚本，需 `shell: true`）

> Windows 上 Python console script 通常生成 `hermes.exe`；git-bash 里的 `which hermes` 可能指向无扩展名脚本。Electron `spawn` 不会自动解析 shebang，必须显式处理 `.exe`/`.cmd` 或 `shell: true`。

### 1.2 能力矩阵

```
probe():
  v = run("--version", timeout=5000)         → 解析 "Hermes Agent vX.Y.Z"
  h = run("chat --help", timeout=5000)       → 是否含 "--format"
  s = run("chat --help", timeout=5000)       → 是否含 "--query-file"
  p = profileExists("dftwork")                → 决定 H1 还是 H2
```

结果缓存到 `store.json → hermes.capabilities`，附 `probedAt`；版本变化时失效。

| 能力 | 缺失后果 |
|---|---|
| `--format stream-json` | 不支持流式 → 只能等最终文本（仍可用，体验降级） |
| `--query-file` | 退回 `-q <prompt>`（注意引号与长度限制） |
| `--resume` | 每轮新建会话（上下文断开） |
| profile `dftwork` | 退回 **H1 无工具模式**（只喂 Bundle，不给工具） |

### 1.3 profile 准备（H2 模式）

专用 profile `dftwork`，要求：

- `platform_toolsets.cli` 仅含：`vasp`（插件工具集）、`file`（只读子集由插件/配置约束）、`skills`
- `disabled_toolsets`：`terminal`、`browser`、`web`、`code_execution`、`image_gen`、`messaging`、`cronjob`
- 不带记忆、不带用户 SOUL（用 `--ignore-rules` 或 profile 内置说明）

启动 AI 前校验一次实际工具面（`hermes --profile dftwork tools list`）。不符合则**禁用 H2**，回退 H1，并在 UI 说明「工具面校验未通过」。

---

## 2. 调用命令

### 2.1 首轮（新建或续接命名会话）

```bash
hermes --profile dftwork chat \
  --query-file - \
  --format stream-json \
  --source tool \
  --quiet \
  --continue "vaspflow:<projectHash>" --create-if-missing \
  --run-budget 300
```

### 2.2 后续轮（已知 session_id）

```bash
hermes --profile dftwork chat \
  --query-file - \
  --format stream-json \
  --source tool \
  --quiet \
  --resume "<session_id>" \
  --run-budget 300
```

要点：

- **prompt 走 stdin**（`--query-file -`），不拼命令行：规避引号、反斜杠、长度与编码问题（实测可用，见文末 E6）。
- `--source tool` 让会话不进入用户的 Hermes 会话列表。
- `--run-budget` 设墙钟上限，防止挂死；超时由 Adapter 记 `TIMEOUT`。
- H1 无工具模式时去掉 `--profile`，并加 `--ignore-rules`（不加载用户记忆/SOUL）。
- 环境变量：`PYTHONUTF8=1`、`PYTHONIOENCODING=utf-8`（Windows 防中文乱码）。

---

## 3. stdout 事件协议

### 3.1 事件类型

| type | 关键字段 | 用途 |
|---|---|---|
| `system` (subtype=`init`) | `timestamp` | 计时起点 |
| `text` | `text` | 增量文本，**需拼接** |
| `tool_use` | `name`、`tool_call_id`、`input` | 生成工具卡片 |
| `tool_result` | `name`、`tool_call_id`、`output`、`duration_ms`、`is_error` | 回填卡片 |
| `result` | `session_id`、`exit_code`、`text`、`tokens{input,output,total,cache_read,cache_write}`、`duration_ms` | 结束、计量 |

### 3.2 解析伪代码

```ts
for await (const line of readline(stdout)) {
  if (!line.startsWith('{')) { logToStderr(line); continue; }   // 容错
  const evt = safeJsonParse(line);
  switch (evt.type) {
    case 'text':        buffer += evt.text; scheduleFlush(100 /*ms*/); break;
    case 'tool_use':    emit({ kind: 'tool', id: evt.tool_call_id, name: evt.name, input: evt.input }); break;
    case 'tool_result': emit({ kind: 'tool-result', id: evt.tool_call_id, ok: !evt.is_error, ms: evt.duration_ms, output: truncate(evt.output, 32_000) }); break;
    case 'result':      finish(evt); break;         // 保存 session_id / tokens
  }
}
```

### 3.3 已知坑

| 坑 | 处理 |
|---|---|
| `text` 是逐 token 碎片 | 累积后按 100ms 节流渲染，不要一块一条消息 |
| `Session:` / `session_id:` 在 **stderr** | 不参与解析；写入调试日志，可作为 session_id 的兜底来源 |
| 进程可能先退出后仍有缓冲 | 以 `result` 事件为准；没有 `result` 则按退出码判定 |
| `tool_result.output` 可能很大 | 截断到 32KB，完整内容落 `%APPDATA%/DFTwork/logs/tools/` |
| 同一 `name` 并行多次调用 | 用 `tool_call_id` 关联，不要按名字配对 |

---

## 4. 会话管理

- **会话键**：`vaspflow:<projectHash>`，`projectHash = sha1(realpath.toLowerCase()).slice(0, 12)`。
- **落盘**：`store.json → sessions[projectHash] = { sessionId, lastUsedAt, turns, totalTokens }`。
- **轮转**：满足任一条件即开新会话（命名 `vaspflow:<hash>:<n>`）——累计 `tokens.input > 120k`、轮次 > 20、或用户点「新对话」。
- **隔离**：不同项目不共享会话；同一项目的分析共享上下文（符合"增量处理"习惯）。

---

## 5. AnalysisBundle（软件 → AI 的事实包）

### 5.1 Schema v1

```json
{
  "schemaVersion": 1,
  "generatedAt": "2026-10-27T16:40:00+08:00",
  "project": { "root": "D:/projects/my-vasp-project", "taskCount": 42 },
  "task": {
    "relPath": "relax/step-01",
    "taskType": { "code": "relax", "label": "结构优化" },
    "status": { "code": "ERROR_DETECTED", "label": "运行出错", "reason": "OUTCAR 含致命错误", "observedAt": "..." },
    "evidence": [
      { "ruleId": "zbr", "file": "OUTCAR", "severity": "error",
        "message": "ZBRENT: fatal error in bracketing", "excerpt": "…（≤400 字符）" }
    ]
  },
  "inputs": {
    "incar": { "ENCUT": 400, "EDIFF": 1e-5, "ISIF": 2, "ISPIN": 1, "MAGMOM": [0, 0] },
    "kpoints": { "scheme": "Gamma", "grid": [1, 1, 1] },
    "potcar": { "elements": ["Si"], "orderMatchesPoscar": true },
    "poscar": { "formula": "Si", "natoms": 8, "latticeConsts": [5.43, 5.43, 5.43] }
  },
  "ionic": {
    "nSteps": 12,
    "truncated": false,
    "points": [{ "step": 1, "energy": -123.456, "fmax": 0.82 }]
  },
  "checks": { "inputCheck": { "code": "PASS", "evidence": [] } },
  "files": [{ "name": "OUTCAR", "size": 1234567, "mtime": "..." }],
  "missing": ["无 CONTCAR", "OSZICAR 仅 3 行"],
  "limits": { "maxPointsPerSeries": 200, "maxExcerptChars": 400 }
}
```

### 5.2 序列化规则

1. **只放结论性数据**，不放原始文件内容（不塞 OUTCAR）。
2. 序列点数 > 200：保留首 50 + 末 150，置 `truncated: true`。
3. 每条 `excerpt` ≤ 400 字符，超出用 `…` 截断。
4. 总大小硬上限 **12 KB**；超出时按 `files → evidence.excerpt → ionic.points` 顺序裁剪，并在 `missing` 里记 `"因体积裁剪：…"`。
5. 时间一律 ISO 8601 带时区。
6. 数值用原始精度，不做四舍五入（保留真实读数）。

---

## 6. Prompt 模板

存放于 `crates/vasp-domain/prompts/analyze.md`，随内核版本化。

```
你是 VASP 计算诊断助手，服务于 DFTwork 桌面软件。

规则：
1. 只依据 <facts> 中的事实下结论。facts 里没有的数据，明确写「缺数据：<项>」，不要推测。
2. 每条结论必须引用字段路径，例如 ionic.points[-1].fmax。
3. 禁止「活性更好/性能更优」这类空泛比较。涉及机理必须给出具体化学原因
   （键能差异、限速步骤是哪个键断裂、多出的反应步骤、配位竞争等）。
4. 可以给下一步操作建议，但只能以「建议」措辞，不得声称已执行。
5. 输出简体中文，结构固定：结论 → 证据 → 建议 → 缺数据。

<facts>
{{BUNDLE_JSON}}
</facts>

用户诉求：{{INTENT}}
```

`INTENT` 枚举：`why_not_converged`、`explain_error`、`review_inputs`、`next_steps`、`freeform`。

---

## 7. 工具映射（H2 模式）

Hermes 插件 `plugins/hermes-vaspflow/` 注册以下工具，handler 统一 shell out 到：

```bash
<coreDir>/vaspcore.exe <subcommand> --json
```

| Hermes 工具 | core 调用 | 面 | 备注 |
|---|---|---|---|
| `vasp_scan` | `scanProject(root)` | 只读 | 限制 `maxDepth`，防误扫全盘 |
| `vasp_inspect_tasks` | `inspectTaskStatus` / `TaskStore` | 只读 | |
| `vasp_convergence` | `parseConvergence(taskDir)` | 只读 | 返回点数上限 500 |
| `vasp_structure_scene` | `getStructureScene(file)` | 只读 | 输出 ≤ 200KB |
| `vasp_task_files` | `listFilesAndDirs` / `listStructureFiles` | 只读 | |
| `vasp_read_file` | `readTextPreview(file)` | 只读 | 500KB 截断 |
| `vasp_discover_inputs` | `srcInspect(root)` | 只读 | |
| `vasp_scan_templates` | `scanTemplates(root)` | 只读 | |
| `vasp_check_inputs` | `checkInputs(dirs, profile)` | 只读 | 只校验，不改文件 |
| `vasp_build_inputs` | `planOneTask`（纯）→ `ProposeBuild` | **提案** | 只返回 BuildPlan，**永不落盘** |
| `vasp_say` / `vasp_done` | — | 输出协议 | 首版不实现；用普通 text 事件替代 |

**强制约束**：插件的 `vasp_build_inputs` handler 只调用 `ProposeBuild`（内部是纯函数 `planOneTask`），不存在 apply 入口；落盘只能由软件 UI 的 `ApproveBuild` 用例触发。

---

## 8. BuildPlan 与放行

### 8.1 Schema

```json
{
  "schemaVersion": 1,
  "planHash": "sha256:…",
  "createdAt": "...",
  "projectRoot": "D:/project/…",
  "source": "ai",
  "items": [
    { "path": "relax/step-01/INCAR", "action": "overwrite", "existedBefore": true,
      "contentSha256": "…", "content": "…", "reason": "ENCUT 由 300 提到 400" }
  ],
  "warnings": ["POTCAR 元素顺序与 POSCAR 不一致"],
  "requires": ["覆盖 3 个已存在文件"]
}
```

### 8.2 规则

1. `planHash = sha256(规范化 JSON，去除 planHash 字段)`；放行时重算比对，不一致即拒绝。
2. 所有 `path` 必须是项目根内的相对路径；`..`、绝对路径、符号链接指向根外——一律拒绝。
3. 写前备份：内容寻址（`blobs/<sha256前16>`），只备份覆盖项，默认排除 POTCAR/WAVECAR；细则见 [ADR-009](decisions.md#adr-009-备份用内容寻址--保留上限不做逐文件时间戳目录)。
4. 默认不覆盖：`existedBefore: true` 的项需要用户在 diff 面板逐项勾选。
5. 落盘后写回执：`<项目根>/.dftwork/receipts/<时间戳>.json`（保留最近 100 条），含计划哈希、文件清单、备份批次、时间。

---

## 9. 错误码与降级

| code | 触发 | UI 行为 |
|---|---|---|
| `HERMES_NOT_FOUND` | 探测全部失败 | AI 区显示「未检测到 Hermes」+ 设置入口 |
| `HERMES_INCOMPATIBLE` | 无 `--format` 且版本过低 | 提示升级；提供"等最终文本"的降级模式 |
| `PROFILE_MISSING` | `dftwork` 不存在 | 回退 H1 无工具模式，提示可安装插件 |
| `TOOL_SURFACE_VIOLATION` | 校验到 terminal/browser 等 | 禁用 H2，回退 H1，给出修复指引 |
| `SPAWN_FAILED` | ENOENT/EACCES | 显示系统错误原文 |
| `TIMEOUT` | 超过 `--run-budget` | 可重试；显示已产生的部分内容 |
| `NONZERO_EXIT` | `exit_code != 0` | 展示 stderr 尾部 20 行 |
| `STREAM_PARSE` | 坏行 > 5 条 | 降级为纯文本拼接，不中断 |

所有错误只影响当前这次分析，**不影响查看器**。

---

## 10. 契约测试（CI 可跑）

| # | 用例 | 断言 |
|---|---|---|
| T1 | `hermes --version` | 能解析出 `vX.Y.Z`，5 秒内返回 |
| T2 | 固定 prompt「回答 OK」+ `--format stream-json` | 至少出现 `system`、`text`、`result`；`result.session_id` 非空 |
| T3 | 同 session 连续两次 | 第二次 `tokens.input` > 第一次（历史被加载） |
| T4 | `--query-file -` 传中文 | 正常返回，无乱码 |
| T5 | 无 profile 时 | 走 H1，且 `PROFILE_MISSING` 被正确上报 |
| T6 | 不存在的可执行路径 | `HERMES_NOT_FOUND`，不抛未捕获异常 |
| T7 | `--run-budget 1` | 在 ~1–2 秒内结束，上报 `TIMEOUT` 或 `NONZERO_EXIT` |
| T8 | 工具面校验 | `dftwork` profile 下不含 `terminal`/`browser` |

> T7 的实际表现待实测（见 `roadmap.md §4`），测完把结论写进本文末「实测事实」。

---

## 11. 二期：ACP 接入点

当需要以下能力时，把 Adapter 的 `transport` 从 `cli` 切到 `acp`：

- AI 直接执行 shell / 写文件，且需要**逐次审批弹窗**
- 会话 fork（同一任务试两种方案）
- 中途 cancel（用户点"停"立即中断）

接入面：`HermesAdapter.analyze()` 保持不变，只替换内部实现（`cli-transport` → `acp-transport`）。ACP 的 `permission` 请求映射到软件的放行对话框，语义与 BuildPlan 放行一致。

届时 ADR-004 需要更新：允许 AI 直接写，但仍须逐次放行。

## 实测事实

> Hermes v0.22.5，2026-10-27 在本机实测。

契约里凡是「Hermes 会怎样」的判断，依据都在这一节。**未在此列出的一律当未知，不要凭印象写代码。**

| # | 事实 | 怎么测的 | 对设计的影响 |
|---|---|---|---|
| E1 | `stdout` 是纯 JSONL，`Session:` 只出现在 `stderr`（此前误判过一次，已更正） | `hermes chat -q ... 2>&1` 与分流两条命令对比 | 必须分两个管道读；失败时先看是不是把 stderr 并进来了 |
| E2 | 事件序列 `system/init` → `tool_use` → `tool_result` → `text…` → `result`；首事件约 3.1 s | 一次真实调用计时 | UI 要给 3 秒以上的「分析中」反馈 |
| E3 | `-t file` **不是白名单**，`browser_*` 仍可见可调 | 用 `-t file` 跑一轮并列出工具调用 | 工具面隔离只能靠 profile（ADR-008） |
| E4 | 一次性调用（`-q`）遇危险命令默认 **deny**，不挂起等待 | 读 `tools/approval_context.py`：`_binary_approval_mode` 默认 deny，`single_query_mode` 决定是否询问 | 不能指望 Hermes 原生审批；落盘必须由软件执行（ADR-004） |
| E5 | `--resume <session_id>` 可命中同一会话 | 两次调用比对 session_id | 多轮分析可行 |
| E6 | `--query-file -` 从 stdin 读 prompt，中文无乱码，约 2.7 s 返回 | 管道传中文 prompt | 不必写临时文件，避免留垃圾 |
| E7 | `result` 事件自带 `session_id` 与 token 用量 | 看 `result` 字段 | 不用另建会话表 |

> 还有 5 个未验证项，见 [roadmap.md §4](roadmap.md)：`read_file` 的 cwd 边界、profile 级 `platform_toolsets` 对 CLI 子进程是否生效、并发写 `state.db` 的互斥性、`--run-budget` 中断后的事件形态、Hermes 插件如何绑定 `vaspcore.exe` 绝对路径。
