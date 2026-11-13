# 当前状态与收敛路线图

> 同步于 2026-11-06。本文是当前计划与验收状态的唯一正文；[architecture.md](architecture.md) 说明架构，[decisions.md](decisions.md) 保留历史决策。
>
> **“已有代码”不等于“完成验收”。** 查看器主流程、P0 单次写入/崩溃恢复/回滚、桌面单实例锁与本机 NSIS / portable 发布验收已完成；备份仍不能替代独立数据备份。

## 1. 里程碑现状

| 里程碑 | 当前实现 | 验收状态与主要缺口 |
|---|---|---|
| **M0 工程骨架** | pnpm workspace、Electron + React、Rust workspace、IPC 与 CLI 均可运行 | 🟡 基础可用；桌面单实例锁已通过双进程验收；聊天历史按用户要求暂不持久化 |
| **M1 Rust 内核** | 四个 crate；扫描、状态、结构、收敛及属性解析已接入 CLI | 🟡 本批解析回归通过；对拍覆盖仍不足 |
| **M2 查看器** | 项目扫描、任务表、统计、收敛图、3D 结构与多任务对比 | 🟡 真实 Electron 手动切换项目并生成计划通过；四类过期响应已有模拟 IPC 永久回归，文件浏览、真实后端延迟与其他属性面板自动化未完成 |
| **M2.5 属性面板** | SCF、每原子力、应力、频率、力常数、总磁矩 | 🟡 面板已接入；首离子步编号已修正，完整真实 OUTCAR 验收仍需继续 |
| **M3 AI 通道** | H1：任务分析 CLI、项目 cwd 自由对话、会话续接、文本与用量显示 | 🟢 H1 自由对话及真实续接已验收；聊天仅存内存；固定任务真实分析未单独验收，H2 插件工具面未实现 |
| **M4 构建与放行** | build/apply/rollback、pending/回执找回、manifest/journal 崩溃恢复、保留策略与历史回执 UI | 🟢 P0 单次写入安全与真实 UI 验收完成；写入恢复/回滚和单实例已有永久 Electron 回归，其他 UI 自动化仍属后续工作 |
| **M5 打包与发布** | 0.2.0 release 内核、NSIS 与 portable；版本、依赖、审计和许可均已整理 | 🟢 本机隔离目录安装/启动/扫描、包内内核和许可验收通过；其他干净 Windows 机器、代码签名与远端 CI 未验收 |
| **M6 轨迹播放** | XDATCAR CLI、两遍流式扫描、均匀抽样与前端播放器 | 🟡 长轨迹、scale 和变胞提醒回归通过；变胞后段仍按首帧晶格渲染 |

原 README 的“M0–M6 全部完成”表示各方向均已有实现，**不再作为验收结论使用**。

## 2. 当前执行顺序

### P0：写入与回滚安全（本轮验收已完成）

已完成的写入边界：

1. `planHash` 绑定项目 root、条目动作、目标内容和原内容；root 或前像变化会使计划失效。
2. apply 预检完整批次，并在每次写入前再次检查目标状态；写入失败反向补偿已写条目。
3. apply 与 rollback 共用真实路径检查，覆盖 `..`、绝对路径、重复路径、符号链接和备份路径；业务计划不能写入 `.dftwork` 元数据目录。
4. `LocalFs` 使用同目录临时文件、`sync_all` 与原子替换；回执记录完整 `applied_hash`，rollback 校验后像及备份 blob，后续修改会安全拒绝。
5. `build` 原子保存 `.dftwork/plans/pending.json`；`project-state` 可在进程重启和界面重载后找回 pending 计划与最近 100 条回执，`discard-plan` 只丢弃计划，不修改业务文件。
6. 覆盖项备份位于 `.dftwork/backup/<时间戳>-<planHash短码>/`，包含 `manifest.json`、内容寻址 blobs 和写入前提交的 `journal.json`。旧 `backup/...` 回执仍能安全回滚。
7. `recover` 区分未提交与已提交中断：前者校验后像后还原覆盖项、删除本次新建项并保留已回执化的恢复记录；后者保留已提交文件和回执，只补完清理。中断后出现人工修改时拒绝自动恢复。
8. 保留策略执行每（任务、文件）5 版、项目 500 MB、最长 30 天和最近 100 条回执；失去备份的历史回执会明确显示不可回滚原因，含恢复 journal 的批次不会被清理。界面可查看占用并手动执行 `prune-backups`。

`write_safety.rs` 的 10 项原有回归继续通过；新增 `persistence.rs` **8 项集成测试**，覆盖跨进程找回、备份清单、备份 I/O 失败、首项写入/备份后/回执前后崩溃、恢复、保留上限、30 天过期和元数据防自写。领域保留策略另有 30 天与容量/journal 保护单测。

真实 Electron + Rust 已在临时项目完成普通界面验收：手动输入 root、生成计划、重载找回 pending、注入首项写入后崩溃并在 UI 恢复、重新放行、再次重载选择历史回执并回滚。已核对覆盖文件恢复原内容、本次新建文件被删除。上述路径现由 `apps/desktop/tests/write-recovery.e2e.cjs` 永久覆盖，测试项目与 Electron 用户目录由运行器隔离并自动清理。

P0 完成不代表备份可替代独立数据备份。桌面单实例锁现已在 P3 实现并通过双进程验收，但外部 CLI 或其他进程并发写同一项目仍不受该锁保护；NSIS/portable 发布验收也仍属 P3。

### P1：修复主流程与解析正确性（本批修复已完成）

已修复并回归：

1. “以此为模板”和自动构建使用当前扫描结果的 `report.root`，不再使用启动参数 `bootProject`；真实 Electron 已验证从启动项目 A 手动切换到 B 后，计划预览来自 B。
2. XDATCAR 使用两遍流式扫描：第一遍统计完整帧数，第二遍均匀保留最多 600 帧并强制保留首尾；2501 帧真实 CLI 回归通过。
3. Direct/Cartesian 坐标均正确处理正 scale；负 scale 继续显式拒绝。变胞重复头部现在会产生提示，后段仍按首帧晶格渲染，不宣称已支持动态晶胞。
4. 力/应力首个 `FREE ENERGIE` 标记对应离子步 1，后续标记才递增；单点数据仍为第 1 步。
5. OSZICAR/OUTCAR 收敛解析均改为流式扫描；超过 64 MiB 的 OUTCAR 能读取汇总力值，存在但读取失败的 OUTCAR 会显式报错，只有文件不存在才允许空力数组。
6. 扫描、结构、收敛和对比请求分别使用递增序号保护，只有最新请求能更新数据、错误和加载状态；新扫描和勾选变化会使相关旧请求失效，对比使用独立加载状态。四类乱序场景均在修改前复现失败、修改后通过。

### P2：统一 CLI、IPC 与 Hermes 契约（H1 自由对话已验收）

1. 除 help/version 外，CLI 成功和失败均使用 stdout JSON 信封，参数错误也有 `VALIDATION` 信封；stderr 只放日志。Electron 保留 `CONFLICT` / `VALIDATION` / `NOT_FOUND` 等原始错误码，并有真实 IPC 回归。
2. IPC 边界已使用 Zod 校验全部请求，包括计划、回执和对话消息；非法输入在启动内核前拒绝。
3. Hermes 探测、任务分析和项目对话统一使用 `--hermes-bin` / `HERMES_BIN` / PATH；路径含空格、未安装和 PATH 中 `.cmd` 场景已有 CLI 回归。本机真实探测 Hermes 0.22.6 与启动参数检查通过。
4. [hermes-contract.md](hermes-contract.md) 已同步 H1 命令、项目 cwd、会话续接、超时和错误语义；H2 工具面仍未实现。
5. UI 已从单一固定分析改为项目级自由对话，Hermes 以当前扫描 root 为 cwd，成功回答后续接会话，切换项目开启新会话。真实 Electron + Rust + mock Hermes 全流程通过。
6. 多 `text` 碎片拼接、坏 JSONL 拒绝、进程退出与 `result.exit_code` 不一致、Windows 进程树超时清理均有新增回归；修复前复现对应失败，修复后通过。
7. 经用户授权，真实 Electron + Rust + Hermes 0.22.6 完成两轮中文自由对话：第二轮正确返回第一轮标记，tokens 与耗时正常显示；重载后聊天清空，仅保留模型/提供方设置。聊天正文与会话 ID 暂不持久化；Hermes 自身的会话存储不由 DFTwork 控制。固定任务 `analyze` 未单独调用真实模型，H2 插件和工具面白名单未验收。

### P3：补齐工程化与发布验收（本机发布验收已完成）

桌面单实例锁已落地并通过真实双进程验收：第二实例以退出码 0 退出，首实例已有窗口恢复、显示并聚焦。P0 UI 往返、单实例、四类过期响应和 Hermes mock 对话共 4 组场景已沉淀为永久测试，由 `pnpm test:desktop` 统一构建和运行。该锁仅约束桌面应用，不保护外部 CLI 或其他进程并发写同一项目。

1. 已新增 Windows CI，运行 golden 重建校验、`cargo test --workspace`、`cargo check --workspace`、前端 typecheck/build 和真实 CLI/Electron 集成测试；按当前计划暂不提交或运行远端 CI。
2. 已加入 3 个入库合成夹具、显式必需清单和可重复生成的 golden；缺文件/读取失败会报错，重复生成无差异，CI 会检查基准漂移。
3. 当前对拍逐项比较离子步编号、能量和全部最大力值；状态、路径、应力和结构等关键字段仍需继续覆盖。
4. 扫描/结构/收敛/对比乱序及 Hermes mock 对话已有永久自动化；继续补充真实后端延迟、其他属性面板和更多真实文件场景。
5. **源码依赖整理已完成**：npm workspace、desktop、shared 与 Rust workspace 统一为 0.2.0，repository 指向 `lrzda/DFTwork`；Electron 42.11.16、electron-builder 26.18.0、electron-vite 6.0.0、Vite 8.3.5 与 Zod 4.6.7 的本地构建和 E2E 已通过。限定范围的 minimatch override 已生效，生产与全量 `pnpm audit` 均无已知漏洞。
6. **许可源码、配置与实物已核验**：`reference/VASPFlow/LICENSE` 保存上游完整 MIT 文本及版权声明，根 `NOTICE` 记录来源；安装目录的 `resources/licenses/` 包含三份许可，且与源文件逐字一致。
7. **本机发布验收已完成**：release 内核及 NSIS / portable 0.2.0 包重建成功；解包版、NSIS 定制目录安装版和 portable 均使用隔离 APPDATA 完成启动、真实扫描和截图。包内内核输出版本 0.2.0；测试安装与隔离目录已卸载清理，安装包保留在 `apps/desktop/dist/`。这不代表其他干净 Windows 机器或代码签名验收。

## 3. 已验证基线（2026-11-06）

- `vaspcore --version` 输出 `vaspcore 0.2.0`；Cargo.lock 与 pnpm-lock.yaml 已同步，frozen lockfile 安装通过。
- `cargo test --workspace --locked`：81 domain + 13 application + 2 infrastructure + 1 parity + 31 CLI 集成测试（10 写入安全 + 8 持久化/故障注入 + 2 解析 + 11 契约），**共 128 项通过**；`cargo check --workspace` 此前通过，仍有 2 个原有的未使用代码警告。
- shared、node、web 三组 TypeScript 类型检查及 Electron 42.11.16 / electron-vite 6.0.0 / Vite 8.3.5 生产构建通过。
- 升级后重新运行 `apps/desktop/tests/` 的 **4 组永久 Electron 场景**并全部通过：普通 UI 写入恢复/回滚、单实例、四类模拟 IPC 乱序，以及真实 Rust + mock Hermes 对话与项目切换。隔离目录自动清理，不调用真实 Hermes。
- `pnpm audit --prod` 与全量 `pnpm audit` 均返回 **No known vulnerabilities found**；`git diff --check` 通过。
- release 内核、`DFTwork-0.2.0-x64.exe` 与 `DFTwork-0.2.0-portable.exe` 构建成功；解包版、NSIS 安装版和 portable 均在隔离用户目录完成启动和扫描，包内内核与三份许可核验通过，未配置代码签名。
- `.github/workflows/ci.yml` 已新增 Windows CI，覆盖 golden 重建校验、Rust test/check、TypeScript typecheck、Electron 构建和上述永久 E2E；按当前计划暂不提交或运行远端 CI。
- 永久 Hermes mock 回归验证自由消息、中文输入、同项目续接、项目 A→B 的 cwd 切换与会话重置，以及 IPC Zod 校验和原始错误码保留。
- 真实 Electron + Rust + Hermes 0.22.6 完成两轮真实自由对话及续接：3985 / 4018 tokens，11.4 / 9.3 秒；第二轮返回第一轮标记 `DFT-731`。重载后该标记消失，localStorage 仅有 `dftwork.ai.model` 和 `dftwork.ai.provider`。该真实付费验收使用独立临时驱动，与永久离线 mock 回归分开记录。
- 永久竞态回归使用真实生产 React/preload 和可控模拟 IPC，扫描、结构、收敛、对比四类乱序及扫描 loading 保护均通过；修改前四场景均复现旧响应覆盖。该测试未验证真实后端延迟。
- 真实 Electron 已验证从启动项目 A 手动切换到 B 后计划预览来自 B；此前另行完成完整 UI 写入与回滚验收。
- 解析回归覆盖 2501 帧完整计数、600 帧抽样与末帧保留、Cartesian 正 scale、负 scale 拒绝、变胞提醒、首离子步编号，以及超过 64 MiB 的 OUTCAR 力值读取。
- 对拍使用 3 个入库合成 fixture，逐项比较离子步编号、能量和全部最大力值；显式清单缺失即失败，golden 连续重建哈希一致，CI 校验生成物漂移。状态、路径、应力和结构仍未覆盖。
- 本轮未运行 `cargo clippy`、`cargo audit`；`cargo fmt --all -- --check` 因工具链缺少 rustfmt 组件而未能执行。固定任务 `analyze` 未单独调用真实模型。

因此，“现有测试全绿”、P0 验收、H1 自由对话与本机发布验收均只覆盖已声明场景，**不构成全字段对拍、外部 CLI 并发写入安全、固定任务真实分析、H2 工具面、其他干净 Windows 机器或代码签名验收结论**。

## 4. 主要风险登记

| 风险 | 当前判断 | 对应工作 |
|---|---|---|
| 多文件部分写入、旧修改覆盖、root 篡改、危险回滚 | P0 已修复；10 项写入安全 + 8 个持久化/故障注入回归通过，恢复中批次受保留策略保护 | 核心 UI/故障场景已有永久回归；继续扩充真实夹具 |
| 普通 UI 构建使用错误项目根 | 已修复；项目切换生成计划及普通 UI apply→重载→rollback 均通过 | 构建计划的项目切换永久 Electron 自动化仍待补 |
| 多实例并发修改同一项目 | 桌面应用单实例锁已通过永久双进程回归；外部 CLI 或其他进程并发写同一项目仍不宣称安全 | 外部写入约束另行设计 |
| 长轨迹、scale、离子步编号和大文件解析 | 已修复并有领域/应用/CLI 回归；变胞仍按首帧晶格渲染 | 更多真实夹具与对拍 |
| CLI 错误被统一包装成 `EXTERNAL` | 已修复；stdout 信封及 NOT_FOUND/CONFLICT/VALIDATION 保留有永久 Electron 回归 | 扩充更多 IPC 契约场景 |
| 异步旧响应覆盖新任务 | 已修复；四类乱序及扫描 loading 保护已有模拟 IPC 永久回归 | 真实后端延迟与其他面板自动化 |
| Hermes 路径、错误流与超时处理 | 已覆盖空格/未安装/PATH 中 `.cmd`、坏 JSONL、退出码冲突和进程树超时；真实自由对话与续接通过 | 固定任务真实分析及 H2 工具面另行验收 |
| 依赖公告 | 生产与全量 pnpm audit 均为 0 个已知漏洞；依赖升级后的构建和四组 E2E 已通过 | 后续依赖变更继续复核；Rust 的 cargo audit 尚未运行 |
| 原项目许可与 NOTICE | 上游完整 MIT 文本、版权行和根 NOTICE 已入库；安装包内三份许可已与源文件核对一致 | 后续发布继续保留许可文件，不作超出证据的法律结论 |

## 5. 范围边界

当前仍不计划实现：

- 集群作业提交与监控；
- 多用户、权限和云同步；
- 自动更新；
- 完整热力学量与依赖 phonopy 的振动自由能；
- macOS / Linux 发布包（代码可跨平台不等于发布验收）。

XDATCAR 和 SCF 已不再是“明确不做”的首版功能，但其实现仍以本文列出的缺陷和验收条件为准。逐原子磁矩继续以数据源是否存在 `magnetization (x)` 表为前提，缺失时返回不可用原因，不伪造零值。
