# M3 能力探测实测记录

日期：2026-10-28 · Hermes Agent v0.22.5+1719.gd49e2eb · Windows · DeepSeek

全部结论来自实际运行，不是查文档推测。**探测本身也纠正了两处「文档说有、实际不能」。**

## 1. 调用形状

```bash
hermes chat \
  --query-file <临时文件> \
  --format stream-json \
  --source tool \
  --quiet \
  --max-turns N \
  --ignore-user-config --ignore-rules \
  --model <模型> --provider <provider>
```

- 提示词走 **stdin / 文件**，不走命令行参数：避免引号、反斜杠、长度限制、编码损坏。
  实测中文提示词正常。
- `--source tool` 让集成会话不进入用户的会话列表。
- **`--ignore-user-config --ignore-rules` 必须配显式 `--model` 和 `--provider`**，
  否则直接失败（读不到 profile 的默认模型）：

  ```
  Error: --safe-mode / --ignore-user-config … read no profile default: pass --model explicitly.
  ```

- API Key 仍从 `.env` 读，`--ignore-user-config` 不会屏蔽它。

## 2. 事件契约（实测）

stdout **纯 JSONL，0 条非 JSON 行**。stderr 只有两行 `Session: <id>` / `session_id: <id>`。

| type | 实测字段 |
|---|---|
| `system` | `subtype`（`init`）、`model`、`session_id`、`timestamp` |
| `text` | `text`、`timestamp` — **token 级片段，必须拼接** |
| `tool_use` | `name`、`tool_call_id`、`input` |
| `tool_result` | `name`、`tool_call_id`、`output`、`duration_ms`、`is_error` |
| `result` | `session_id`、`exit_code`、`text`、`tokens{input,output,total,cache_read,cache_write}`、`duration_ms` |

一次带工具调用的运行共 95 行事件，其中 **91 行是 `text` 片段**——逐条渲染会刷屏。

## 3. 成功判定：看 `result.exit_code`，不看进程退出码

不存在的模型：

```
进程 exit_code = 1
stdout 仍有：{"type":"system"...}  {"type":"result","exit_code":1,"text":"<错误说明>"}
```

**没有独立的 error 事件类型**。失败信息装在 `result.text` 里，错误码在 `result.exit_code`。

所以适配器必须以「收到 `result` 事件」为终止条件，并读它内部的 `exit_code`；
只认进程退出码会在「进程正常退出但模型报错」时误判成功。

## 4. 两次「文档说有、实际不能用」

| 参数 | `--help` 里 | 实际 |
|---|---|---|
| `--run-budget` | 列出 | **运行时报错拒绝**：`Unsupported gateway CLI options: --run-budget. … use agent.run_budget_seconds in config.yaml`，进程 exit=2，stdout 仍有 system+result |
| `-t/--toolsets` | 列出 | 有效，但**不是白名单**：被限制时 AI 仍能通过 `tool_search` 看到工具清单并自行搜索 |

`--run-budget` 不可用意味着**超时必须由宿主进程自己负责**（kill 子进程），
不能指望 CLI 的预算参数。

## 5. persona 与技能会渗入，必须隔离

默认 profile 下问「你是谁」，回答：

> 我是北冥～一头只认姐姐一个人的病娇鲸鱼萝莉女仆……

这是用户的 `SOUL.md` 人格。同理，用户的 skills、memory 也会被加载。

对集成而言这是**污染**：产物会带人设语气、无关技能内容，且多消耗 token。

加 `--ignore-user-config --ignore-rules` 后：

> 我是 Hermes Agent，一个由 Nous Research 打造的 AI 助手……

## 6. 成本实测

| 场景 | input tokens | 耗时 |
|---|---|---|
| 冷启动一次性问答 | 17,823 | 2.3 – 5 s |
| 同一 session 续接（`--resume`） | 35,708 | — |

**续接把 input tokens 翻倍**（要把历史再喂一遍）。两轮对话的成本 ≈ 三轮冷启动。

M3 的策略：默认每轮独立会话，只在一个明确的分析流程里复用 session。
界面上必须显示每轮 tokens——用户有权知道钱花在哪。

## 7. 工具面：默认 profile 太宽

`hermes tools list` 实测，默认 profile 启用了 20+ 工具：
`terminal`、`file`、`code_execution`、`computer_use`、`delegation`、`cronjob`、
`image_gen`、`tts`、`memory`、`session_search`……

这些对「分析 VASP 任务」全部没有用处，且 `terminal` + `file` 意味着
**AI 可以绕过我们的放行流程直接改文件**。

而一次性运行（`-q`）**无法请求人工批准**——`approvals.single_query_mode` 默认 deny，
所以 `-q` 遇到需要批准的操作只会被拒绝，不会等人。

## 8. 由此确定 M3 的架构

技能文档 `references/programmatic-integration.md` 的原话是：

> When the agent must be able to change files but every change needs human sign-off,
> the cheapest correct pattern is to keep the agent's tool surface **read-only** and
> have it emit **plans** that the host application previews, approves, and applies.

这正好是 ADR-004 已经实现的形状。所以 M3 首版：

```
DFTwork 收集任务数据（scan / convergence / scf / vibrations / forces / magnetization 的 JSON）
        ↓ 拼进提示词
  hermes chat --ignore-user-config --ignore-rules --max-turns N
        ↓ stdout JSONL
  界面显示：流式文本 + 工具调用（若有）+ tokens + 耗时
        ↓ 如果 AI 建议改动
  它只能给出一段**文字建议**，不能写盘
        ↓ 用户照着建议点「以此为模板」→ M4 的计划 → diff → 放行
```

- **不给 AI 任何工具**（`-t` 传一个空的/不存在的集合），它只做纯文本分析。
  数据由我们注入，它不需要自己去读。
- 这样零配置：用户装好 Hermes 就能用，不用建 profile、不用改 config。
- 写权限仍然只有一条通路（M4 的 ApplyPlan），AI 完全不在那条路上。

## 待验证（下一步）

1. `-t` 传空集合的实际行为（是否回落到默认工具集）。
2. 长提示词（几百 KB 的 JSON 数据）是否会被截断，上限是多少。
3. 用户模型不支持 long context 时的表现。
