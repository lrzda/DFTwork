# DFTwork

VASP 计算任务工作台（Windows 桌面软件）。

**当前可运行**：扫描、诊断、可视化、多任务对比已全部实现并有真实数据验证；
构建输入 / 放行 / 备份回滚（M4）、AI 通道（M3）、打包（M5）尚未开始。

## 现在能做什么

| 能力 | CLI | 界面 | 实测 |
|---|---|---|---|
| 扫描项目、判定状态与任务类型 | `scan` | 任务表 + 统计看板 | 多任务 / 多目录 |
| 离子步能量 + 最大力 | `convergence` | 收敛曲线 | 数百步 |
| SCF 电子步迭代 | `scf` | 对数残差曲线 | 数百步 |
| 每原子力 + 应力张量 | `forces` | 受力排行 + 应力表 | 逐原子 + 六分量 |
| 振动频率 + 力常数矩阵 | `vibrations` | 频谱柱图 + 热图 | 模式数 + 虚频标记 |
| 总磁矩演化 | `magnetization` | 磁矩曲线 | 逐步演化 |
| 3D 结构（成键 / PBC / 镜像原子） | `scene` | three.js 看板 | 球棍 + 晶胞 |
| 多任务对比（INCAR diff + 曲线） | `compare` | 差异表 + 叠加图 | 差异项逐条列出 |

内核 `vaspcore.exe` 的每条命令都输出**恰好一行 JSON**（`{"ok":true,"data":…}`），
界面里看到的每个数字都来自同一次解析——UI 与 AI 工具共用同一个内核，避免结论漂移。

## 快速开始

```bash
# 1. 编内核
export RUSTUP_HOME="D:\Program Files\rust\rustup" CARGO_HOME="D:\Program Files\rust\cargo"
export PATH="/d/Program Files/rust/cargo/bin:$PATH"
cargo build -p vasp-cli
./target/debug/vaspcore.exe scan "D:/你的/VASP项目"

# 2. 编界面并启动
pnpm install
pnpm --filter @dftwork/desktop build
node_modules/.pnpm/electron@*/node_modules/electron/dist/electron.exe apps/desktop/out/main/index.js
```

无界面验证（自动扫描 + 截图 + 退出）：

```bash
DFTWORK_SMOKE=1 \
DFTWORK_SMOKE_ROOT="D:/你的/VASP项目" \
DFTWORK_SMOKE_DIR="$PWD" \
node_modules/.pnpm/electron@*/node_modules/electron/dist/electron.exe apps/desktop/out/main/index.js
```

可加 `DFTWORK_SMOKE_PANEL=scf|vibration|force|magnetization|compare`、
`DFTWORK_SMOKE_TASK=<相对路径>`、`DFTWORK_SMOKE_COMPARE=<rel1>,<rel2>` 指定截图状态。

## 定位

- **独立运行**：不依赖任何 AI Agent。扫描、判定、解析、可视化全部本地完成。
- **AI 可选**：用户自行安装 [Hermes Agent](https://hermes-agent.nousresearch.com/docs/) 后
  可对选中任务发起分析；未安装时软件功能完整，仅 AI 入口置灰。
- **数据同源**：UI 与 AI 工具共用 `vaspcore`（Rust 重写，行为对齐 VASPFlow），
  避免「面板说收敛、AI 说不收敛」。

## 内核设计要点

这几条都是实测撞出来的，不是设计时的猜测：

- **大文件必须流式**：单份 OUTCAR 可达几百 MB / 数百万行，既超整读上限，
  也不能只读尾部（频率块、磁矩块在中部）。`Fs::scan_lines` 用 `read_until`
  而非 `lines()`——后者遇非 UTF-8 字节直接报错。
- **成键判据改过**：原 VASPFlow 注释写 pymatgen `MinimumDistanceNN`，代码实际是
  `(vdw_i+vdw_j)/2`，在真实 多个原子体系上给出 **0 根键**（阈值 1.66–1.93 Å vs
  最近邻中位 2.80 Å）。现改为共价半径和 × 1.2 + 最小镜像距离。
- **跨界键画镜像原子**，不拉长线：一批键里 107 根（57%）会被画成横穿盒子的长线，
  最长 24.22 Å 而真实键长 3.04 Å。
- **PBC 开关决定两件事**：原子位置（折回 / 真实）与成键判据（最小镜像 / 真实距离）。
  切换时重新向内核要场景，前端不重复实现判据。
- **VASP 频率输出不是升序**：实频从高到低（13.97→0.587），虚频紧接着从低到高
  （0.044→11.70）。所以「第一个模式」既不是最低也不是最高。
- **逐原子磁矩缺失**：本机所有 `ISPIN=2` 的 OUTCAR 都没有 `magnetization (x)` 表
  （VASP 5.4.4，`NWRITE` 1 与 2 都没有）。返回 `per_atom_available=false` + 原因，
  而不是空数组（空数组会让人误以为「所有原子磁矩为 0」）。
- **热力学量不做**：Γ 点简谐 ZPE/S/G 理论上 VASP 会输出，但实测体系虚频很多使
  简谐近似无意义，完整振动自由能需 phonopy。

## 来源与许可

基于 [VASPFlow](https://github.com/21271122/VASPFlow)（MIT）的 host 数据层与前端组件
改造为独立桌面应用。保留原项目版权声明与 LICENSE；本项目同样以 MIT 发布。

原 JS 代码位于 `project/reference/VASPFlow/`，**只作行为参考与对拍基准**，不参与构建。

## 已定决策（详见 [docs/decisions.md](docs/decisions.md)）

| # | 决策 |
|---|---|
| 001 | 独立桌面软件，**不内嵌** Hermes；用户自装或不装 |
| 002 | Electron + React 18（最大化复用 VASPFlow 前端） |
| 003 | `vaspcore`（Rust）为唯一数据层，同时输出可执行文件与 CLI |
| 004 | 写操作两阶段：**AI 提案 → 用户放行 → 软件落盘** |
| 005 | 一期 AI 通道用 `hermes chat --format stream-json`；ACP 留作二期 |
| 006 | 首版以 VASPFlow 既有功能为基线；SCF / 磁矩 / 力 / 应力 / 频率已超范围实现 |
| 007 | 状态存储用 JSON 起步，避开原生模块 ABI 问题 |
| 008 | AI 工具面白名单：只读工具 + `vasp_*`，不给 `write_file` / `patch` / `terminal` |
| 009 | 备份用内容寻址 + 保留上限 |
| 010 | 外壳 **Electron** + 内核 **Rust 重写**；前端 React 不重写 |
| 011 | 渲染抽象为 `SceneRenderer` 接口，界面不依赖 three.js；首版实现 `ThreeRenderer` |

## 文档

| 文件 | 讲什么 |
|---|---|
| [docs/architecture.md](docs/architecture.md) | **架构唯一正文**：进程模型、数据流、四层与依赖规则、用例、端口、测试策略 |
| [docs/decisions.md](docs/decisions.md) | ADR 001–011 |
| [docs/rust-rewrite.md](docs/rust-rewrite.md) | Rust workspace、CLI 契约、对拍流程 |
| [docs/hermes-contract.md](docs/hermes-contract.md) | AI 契约：命令、事件协议、放行规则、实测事实 |
| [docs/roadmap.md](docs/roadmap.md) | 里程碑与验收、风险、未验证项 |
| [CONTEXT.md](CONTEXT.md) | 术语表（只放定义，不放计划） |

## 目录结构

```
DFTwork/
├── docs/                    设计文档（架构 / 决策 / 路线 / AI 契约）
├── project/                 实际工程
│   ├── apps/desktop/        Electron 主进程 + preload + 渲染进程
│   ├── crates/
│   │   ├── vasp-domain/     纯逻辑：解析、判定、几何（无 IO）
│   │   ├── vasp-application/ 用例编排 + DTO + 端口定义
│   │   ├── vasp-infrastructure/ 文件 / Hermes 子进程 / 时钟
│   │   └── vasp-cli/        vaspcore.exe
│   ├── packages/shared/     横切：DTO + IPC 契约
│   ├── reference/VASPFlow/  原 JS（只读，对拍基准）
│   └── tests/               跨包集成 + 新旧对拍
└── CONTEXT.md
```

## 当前进度

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M0 | 工程骨架、Electron 空壳、IPC | ✅ |
| M1（部分） | Rust 内核：解析 / 判定 / 几何 / 对拍 | ✅ 63 单测 + 对拍全绿 |
| M2 | 查看器：任务表、统计看板、3D 看板、收敛曲线 | ✅ |
| M2.5 | SCF / 力 / 应力 / 频率 / 力常数 / 磁矩 / 多任务对比 | ✅ 真实数据验证 |
| M4 | 构建输入、放行、备份、回滚 | ⬜ 未开始 |
| M3 | AI 通道 | ⬜ 未开始 |
| M5 | 打包（NSIS / portable） | ⬜ 未开始 |

测试：`cargo test --workspace` = 63 domain + 2 infrastructure + 1 parity，全绿。
