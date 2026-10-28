# 路线图、验收与风险

## 1. 里程碑

单人工期估计，按"能跑通真实项目"为完成标准，不含美化与文档打磨。

| 里程碑 | 内容 | 工期 | 交付物 | 状态 |
|---|---|---|---|---|
| **M0 骨架** | monorepo（pnpm workspace）、electron-vite、单实例锁、IPC ping、日志；**Electron 空壳基线** | 1–2 天 | 能启动的空窗口 + `window.dftwork.ping()` | ✅ 完成 |
| **M1 Rust 内核重写** | R0–R2：workspace、CLI 契约、解析、状态/成键判定；R3：8 个用例 + infrastructure；R4：对拍收口 | **5.5–7.5 周**（见 [rust-rewrite.md](rust-rewrite.md)） | `vaspcore.exe` 9 个子命令；对拍零差异 | 🟡 解析与查看侧完成，构建侧（build/check/discover）未做 |
| **M2 查看器** | 任务表、统计看板、收敛图、3D 结构（`SceneRenderer` + `ThreeRenderer`）；文件浏览待做 | 5–7 天 | 打开真实项目可用 | ✅ 完成（文件浏览除外） |
| **M2.5 属性面板** | SCF 电子步、每原子力、应力、振动频率、力常数、磁矩、多任务对比 | 4–5 天 | 六项属性 + 对比页 | ✅ 完成，真实数据验证 |
| **M3 AI 通道** | 能力探测、HermesAdapter、JSONL 解析、AnalysisBundle、AiPanel、会话续接、Hermes 插件（12 工具转调 core CLI） | 4–6 天 | 点「分析此任务」出结果；AI 能调 `vasp_*` 工具；无 Hermes 时降级 |
| **M4 构建与放行** | BuildPlan、diff 预览、planHash 放行、内容寻址备份、回执、回滚；备份清理策略（500 MB / 30 天 / 5 版） | 5–7 天 | 批量生成输入并可回滚；单次写入备份 < 2 MB |
| **M5 打包** | electron-builder（NSIS + portable）、首启引导、许可与 NOTICE | 2–3 天 | 干净机器可安装运行 |

合计约 **45–65 个工作日**（约 9–13 周），其中 M1 Rust 重写占 5.5–7.5 周。
**M0–M2.5 已完成**，查看侧全部可用；剩余 M3（AI）、M4（构建放行）、M5（打包）。

## 2. 验收标准（可测，不写"体验良好"这类话）

### M1

- `vaspcore.exe scan "D:/project/…" --json` 与原 JS 实现输出**逐字段一致**（任务数、状态码、路径、INCAR 摘要）。
- `cargo test` 全绿，覆盖原 77 项测试对应的行为。
- `tests/parity` 在 3 个真实项目 + 20 个合成 fixture 上零差异（或差异全部记为 `known_divergence`）。
- release 二进制 < 8 MB、冷启动 < 50 ms；`apps/desktop` 不出现任何 Rust 绑定。

### M2

- 打开一个含 50+ 任务的项目，**3 秒内**出现前 50 条，扫描不阻塞 UI。
- 选中任务到 3D 首帧 **< 1 秒**（POSCAR 500 原子以内）。
- 文件预览对 > 500KB 文件截断并明确提示。
- 中文路径、空格路径、CRLF 文件均正常。

### M3

- 点「分析此任务」，**30 秒内**给出结果（本机实测冷启动 3.1s + 推理）。
- 结论中出现至少 1 处字段引用（如 `ionic.points[-1].fmax`）；`missing` 非空时被显式列出。
- 把 Hermes 路径改错后重启：AI 按钮置灰并给出原因，**查看功能不受影响**。
- 连续两轮分析命中同一 `session_id`。
- 在 Hermes 会话里调 `vasp_scan`，返回的任务数与 UI 面板**完全一致**（验证数据同源）。

### M4

- 对 3 个任务生成计划，diff 与预期逐行一致。
- 放行后写入文件与计划内容**字节一致**（sha256 比对）。
- 计划在放行前被修改 → `planHash` 不匹配 → 拒绝执行。
- 覆盖已有文件后可从备份完整还原。

## 3. 风险

| # | 风险 | 影响 | 缓解 | 状态 |
|---|---|---|---|---|
| R1 | UI 与 AI 各写一份解析 → 结论漂移 | 高 | ADR-003：core 唯一实现 + CLI 契约测试 | 已设计消除 |
| R2 | AI 幻觉（把推测当观察） | 高 | Bundle 带 `evidence`/`missing`；prompt 强制引用字段；UI 标注"AI 结论基于软件提供的摘要"；二期 H2 工具让 AI 自己取证 | 部分缓解 |
| R3 | 绕过放行直接写 | 高 | ADR-004/008：工具面白名单 + 启动校验 profile | 已设计消除 |
| R4 | 只读工具能读项目外文件 | 中 | 待验证 `read_file` 的路径边界；若不受限，考虑给 AI 专用 cwd + 沙箱副本 | **待验证** |
| R5 | Electron 体积/内存 | 低 | 接受；提供 portable 版 | 已知 |
| R6 | Hermes 版本漂移（参数/事件变化） | 中 | 能力探测 + 契约测试（见 `hermes-contract.md §10`）；不支持则降级 | 已设计 |
| R7 | 成本：每轮约 17k 输入 tokens | 中 | UI 显示本轮 tokens；会话轮转；避免重复分析 | 已实测 |
| R8 | Windows 路径/编码/中文 | 中 | core 统一 `paths.js`；全链路 UTF-8；测试用例覆盖 | 已设计 |
| R9 | 许可合规 | 低 | 保留 MIT 与 NOTICE；依赖均为 MIT/BSD | 已设计 |
| R10 | 大项目扫描慢 | 中 | 增量事件 + 缓存 + 深度限制 | 已设计 |
| R11 | 备份/缓存堆积占满磁盘 | 中 | ADR-009：内容寻址 + 只备份覆盖项 + 排除 POTCAR + 500 MB/30 天上限 | 已设计 |
| R12 | 体积/内存超出预期 | 中 | M0 量基线；按 [ADR-010](decisions.md#adr-010-桌面外壳用-electron内核用-rust-重写) 的瘦身清单优化（外壳不换） | 待实测 |

## 4. 已知未知（需要实测后才能定稿）

1. **`read_file` 是否有 cwd 边界**——决定 R4 的严重程度与是否需要沙箱。
2. **profile 级 `platform_toolsets` 对 CLI 子进程是否生效**——决定 ADR-008 的落实方式（是否需要 `--ignore-user-config` + 专用配置目录）。
3. **多个 Hermes 子进程并发写 `state.db` 是否互斥/报错**——决定并发策略（当前设计为单并发队列）。
4. **`--run-budget` 中断后 `result` 事件的形态**——决定超时是否可识别。
5. **Hermes 插件注册 `vasp_*` 工具时，如何绑定 core CLI 的绝对路径**——决定安装器要写哪些配置。

每条都配一个最小的验证命令，在 M3 开工前跑完，把结论补进 [hermes-contract.md](hermes-contract.md) 的「实测事实」。

## 5. 明确不做（首版）

- XDATCAR 轨迹播放（留接口，见 `architecture.md §14`）
- 热力学量（ZPE / S / G(T)）——VASP 只有 Γ 点简谐近似，完整计算需 phonopy
- 逐原子磁矩（当前数据源没有 `magnetization (x)` 表，接口已留；需 `LORBIT=11`）
- 集群作业提交/监控（作业由外部工具管理，软件只读结果）
- 多用户/权限/云同步
- 自动更新（首版手动下载新版本）
- macOS / Linux 打包（代码跨平台，但只发布 Windows）

## 6. 开工顺序（M0 第一天的具体动作）

1. `cargo new --lib crates/vasp-domain`（其余三个同理）+ workspace `Cargo.toml`；前端只建 `apps/desktop` 与 `packages/shared` 两个 pnpm 包。
2. VASPFlow 克隆到 `reference/VASPFlow`（只读、不提交），固定 commit，用 `scripts/gen-golden.mjs` 生成 golden。
3. 先写 `vasp-domain` 的解析纯函数 + 夹具，`cargo test` 通过。
4. 接 `vasp-cli`，跑 `vaspcore.exe scan <真实项目> --json`，与 JS 版逐字段对拍。
5. 起 electron-vite 空窗口，spawn `vaspcore.exe` 接一条 `ping`，提交首个 commit。
