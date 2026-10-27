# DFTwork 术语表

本文件只放**词汇定义**，不放计划、进度或决策理由。新词先在实现里出现、再固化到这里。

## Language

**任务（Task）**：
一个包含 VASP 输出的计算目录，判据是该目录下存在 `OUTCAR` 或 `vasprun.xml`。任务由扫描器发现，带 `rel_path` 作为稳定标识。
_Avoid_：计算结果、算例、job（`job` 在本项目中专指集群作业，见下）

**项目根（Project Root）**：
用户选中的目录，扫描从它开始向下遍历。所有任务的 `rel_path` 相对于它。
_Avoid_：工作目录、根目录

**事实标准（Core）**：
Rust 内核 `vaspcore`（`crates/vasp-*`，编译为 `vaspcore.exe`）。所有"这个任务是什么状态""这条曲线是什么形状"的判断只由它产出，UI、CLI、AI 工具都消费它的输出，不得各写一份。
_Avoid_：解析层、后端

**任务状态记录（TaskStatusRecord）**：
对任务状态的一次判定结果，包含 `code`（`NO_OUTPUT_EVIDENCE` / `TASK_COMPLETED` / `ERROR_DETECTED` / `RUNNING` / `UNKNOWN`）、`reason`、`evidence[]`、`observedAt`。状态必须附证据，不允许只给结论。
_Avoid_：状态、结果

**输入检查（InputCheck）**：
对任务输入文件（POSCAR/INCAR/KPOINTS/POTCAR）的一致性校验结果，`code` 为 `NOT_REQUESTED` / `NOT_CHECKED` / `PASS` / `WARN` / `FAIL`，附 `evidence[]`。它回答"输入是否自相一致"，**不回答"能否直接提交"**。
_Avoid_：校验通过、可提交

**构建计划（Build Plan）**：
AI 或向导产出的、**尚未落盘**的一组文件变更描述（目标路径 + 完整内容 + 生成依据）。计划可预览、可 diff、可丢弃。
_Avoid_：构建结果、生成的文件

**放行（Approve）**：
用户在一次具体操作中，对**具体计划**点确认。放行是一次性的、绑定了计划哈希的；不是全局开关，不可继承给下一次。
_Avoid_：同意、允许（"允许"易与 Hermes 的 approvals 混淆）

**会话键（Session Key）**：
`vaspflow:<项目路径哈希>`。用于把同一项目的多轮 AI 分析续接到同一个 Hermes 会话。
_Avoid_：会话名、chat id

**只读面（Read Surface）**：
AI 可自由调用的工具集合：扫描、解析、读文件、校验。不产生任何写入。
_Avoid_：安全工具

**提案面（Propose Surface）**：
AI 可调用但**只能产出计划**的工具集合（如 `vasp_build_inputs`）。调用结果进入待放行队列，不落盘。
_Avoid_：写工具

**通道（Channel）**：
软件驱动 Hermes 的方式，分 H1（CLI 一次性 + 流式）、H2（Hermes 插件工具）、H3（ACP 交互）。见 `docs/hermes-contract.md`。
_Avoid_：接口、API

**作业（Job）**：
提交到计算集群的一次 VASP 运行。软件首版不管理作业，只读其结果。
_Avoid_：任务（任务指本地目录）

**用例（Use Case）**：
应用层的一个可执行操作（`OpenProject`、`ScanProject`、`ApproveBuild`…），一个用例一个 `execute()`，负责编排领域能力与基础设施、返回 DTO。
_Avoid_：命令、接口、API

**端口（Port）**：
领域层定义的 Rust trait（`HermesPort`、`ProjectStorePort`、`FileWriterPort`…），由基础设施层实现。只有"需要替换或需要 mock"的东西才端口化；文件读取与目录遍历不做端口。
_Avoid_：抽象类、适配器（适配器是实现者）

**组合根（Composition Root）**：
唯一把所有实现 `new` 出来、注入用例的地方，位于 `crates/vasp-cli/src/main.rs`。Electron 主进程不装配，只 spawn `vaspcore.exe`。其余代码不得自行构造基础设施。
_Avoid_：入口、启动文件

**TaskDetector**：
领域服务：给定「目录里的文件名列表 + 读取回调」，判定该目录是否为 VASP 任务。不自己碰文件系统。
_Avoid_：扫描器（扫描器指基础设施层的 Walker）

**DirectoryWalker**：
基础设施组件：负责目录遍历、跳过规则、并发与事件，产出目录项。不含"是不是 VASP 任务"的判断。
_Avoid_：扫描器、scanner

**PlanWriter**：
基础设施组件，实现 `FileWriterPort`。**全项目唯一落盘点**：备份 → 写临时文件 → rename → 回执。
_Avoid_：写文件函数、output builder

**对拍（Parity Test）**：
用同一输入同时跑旧实现与新实现，逐字段比对输出的测试方式。拆分 core 时的强制验收手段。
_Avoid_：回归测试（范围太泛）

**备份批次（Backup Batch）**：
一次放行对应一个备份目录 `backup/<时间戳>-<planHash 短>/`，内含 `manifest.json` 与该批次用到的 blob。清理以批次为单位。
_Avoid_：备份、快照

**blob（内容寻址对象）**：
以内容 `sha256` 前 16 位命名的备份文件；同一内容在项目里只存一份。批量构建时大量相同的 INCAR 因此只占一份空间。
_Avoid_：副本、备份文件

**待放行计划（Pending Plan）**：
`ProposeBuild` 产出、尚未落盘的 BuildPlan。全局最多保留一个（`plans/pending.json` 覆盖写），关闭软件后重开会提示继续或丢弃。
_Avoid_：草稿、临时计划
