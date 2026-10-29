# DFTwork 工程

VASP 计算任务工作台的实现。设计文档在 [`../docs`](../docs)，**本目录只放代码**。

## 结构

```
project/
├── crates/                    Rust 内核（编译产物 vaspcore.exe）
│   ├── vasp-domain/           纯逻辑：模型、解析、判定、规则、端口（零文件 IO）
│   ├── vasp-application/      14 个用例：编排、读文件、DTO、错误码
│   ├── vasp-infrastructure/   实现端口：LocalFs、HermesCli、JsonProjectStore、日志
│   └── vasp-cli/              唯一组合根 + clap 子命令 → JSON
├── apps/desktop/              Electron 外壳
│   ├── src/main/              窗口 + IPC（spawn vaspcore.exe）
│   ├── src/preload/           contextBridge → window.dftwork
│   └── src/renderer/          React 18 + antd + recharts
├── packages/shared/           DTO / IPC 契约 / 错误码（TS，与 Rust DTO 一一对应）
├── reference/VASPFlow/        原 JS 实现（MIT，只读，仅用于生成对拍基准）
├── tests/                     测试数据落地位置（当前为空，样例按需放入）
└── scripts/gen-golden.mjs     跑旧实现，产出对拍基准
```

## 依赖方向（由 Cargo 强制）

```
vasp-domain  ←  vasp-application  ←  vasp-infrastructure  ←  vasp-cli
     ↑                                                          │
     └──────────── 实现 domain 的 trait ─────────────────────────┘
```

`vasp-domain/Cargo.toml` 里不写任何内部依赖——它在物理上无法引用上层，写错就编译不过。

## 常用命令

```bash
# 内核
cargo check --workspace
cargo test  --workspace
cargo build -p vasp-cli --release          # → target/release/vaspcore.exe

# 直接试内核（把 <项目根目录> 换成任意 VASP 计算目录）
./target/debug/vaspcore scan <项目根目录>
./target/debug/vaspcore convergence <任务目录>
./target/debug/vaspcore scene <任务目录>/POSCAR

# 前端
pnpm install
pnpm dev                                    # electron-vite dev
pnpm typecheck

# 对拍（先有样例与 golden 才能比；缺 golden 时测试自动跳过）
node scripts/gen-golden.mjs <项目根目录>
cargo test -p vasp-cli --test parity
```

## 输出契约

内核**所有**子命令：

- **stdout**：恰好一行 JSON，`{"ok":true,"data":…}` 或 `{"ok":false,"error":{"code":…,"message":…}}`
- **stderr**：日志

Electron 只解析 stdout。所以内核代码里任何 `println!` 都是 bug。

## 当前进度

| 部分 | 状态 |
|---|---|
| workspace + 4 crate 骨架 | ✅ |
| domain：模型 / 路径规则 / planHash / 覆盖策略 | ✅ 含单测 |
| domain：POSCAR 解析（VASP4/5、Direct/Cartesian、选择性动力学） | ✅ |
| domain：OSZICAR 能量 + OUTCAR 最大力 | ✅ |
| domain：状态判定（4 类状态 + 证据） | ✅ 已与真实数据对拍 |
| domain：任务类型判定（INCAR → 优化/频率/AIMD/SCF/NEB） | ✅ |
| domain：成键（共价半径 ×1.2 + 最小镜像）、周期/真实双模式 | ✅ 11 项单测 |
| domain：SCF 电子步 / 磁矩 / 力 / 应力 / 频率 / 力常数 | ✅ 流式，73 项单测 |
| domain：INCAR diff（多任务对比） | ✅ 4 项单测 |
| domain：输入校验、构建计划、分析包、轨迹解析 | ✅ |
| application：Scan / Convergence / Scene / Magnetization / Scf / Forces / Vibrations / Compare | ✅ 8 个用例 |
| application：其余 6 个用例（Build / ApplyPlan / Rollback / Analyze / Trajectory 等） | ✅ |
| infrastructure：LocalFs（GBK 兜底、尾部读取、流式扫描） | ✅ |
| infrastructure：HermesCli 探测 | ✅ 只探测，不发起会话 |
| CLI：scan / convergence / scene / magnetization / scf / forces / vibrations / compare / trajectory / analyze / build / apply / rollback / hermes-probe / version | ✅ |
| Electron 主进程 + preload + IPC | ✅ 已启动验证 |
| 渲染进程：任务表、统计看板、7 个属性标签页 | ✅ |
| 3D 结构看板（three.js、PBC、成键、镜像原子） | ✅ |
| 构建 / 放行 / 备份回滚 | ✅ 计划哈希校验 + 内容寻址备份 |
| 轨迹播放（XDATCAR 逐帧） | ✅ 600 帧上限，超限均匀抽样 |
| 打包（NSIS / portable） | ✅ 已发布 v0.2.0 |

## 冒烟测试（无人值守验证界面）

```bash
cargo build -p vasp-cli
pnpm --filter @dftwork/desktop build
DFTWORK_SMOKE=1 \
DFTWORK_SMOKE_ROOT="<你的 VASP 项目根目录>" \
DFTWORK_SMOKE_DIR="$PWD" \
node_modules/.pnpm/electron@*/node_modules/electron/dist/electron.exe \
  apps/desktop/out/main/index.js
# → 生成 dftwork-smoke.png 后自动退出
```

三个环境变量：`DFTWORK_SMOKE=1` 开启、`DFTWORK_SMOKE_ROOT` 自动扫描的目录、
`DFTWORK_SMOKE_DIR` 截图落盘位置。不用这三个变量就是正常启动。

## 许可

MIT。`reference/` 与原前端组件来自 [VASPFlow](https://github.com/21271122/VASPFlow)（MIT），保留其版权声明。

## 打包成安装包

```bash
# 1. release 版内核（必须！debug 版 40+ MB，release 只有 2.9 MB）
cargo build --release -p vasp-cli

# 2. 出包
export ELECTRON_BUILDER_BINARIES_MIRROR="https://npmmirror.com/mirrors/electron-builder-binaries/"
pnpm --filter @dftwork/desktop package
```

产物在 `apps/desktop/dist/`：

| 文件 | 说明 |
|---|---|
| `DFTwork-0.1.0-x64.exe` | NSIS 安装包（可选安装目录、建桌面快捷方式、卸载保留用户数据） |
| `DFTwork-0.1.0-portable.exe` | 免安装版，双击即用 |

### 打包时的三个要点

1. **内核走 `extraResources`，不进 asar**。asar 是虚拟文件系统，里面的 exe
   没法被 `spawn`。配置在 `apps/desktop/electron-builder.yml` 的
   `extraResources: target/release/vaspcore.exe → bin/vaspcore.exe`，
   对应 `src/main/core.ts` 里 `app.isPackaged` 分支的路径。

2. **`HERMES_BIN` 必须由主进程解析**。终端里 `hermes` 能用是因为 shell 配置加了
   PATH，而 Electron 继承的是系统 PATH——直接 `spawn("hermes")` 会 ENOENT。
   打包后的机器上同样如此，所以 `main/index.ts` 启动时解析一次绝对路径。

3. **图标在 `apps/desktop/build/icon.ico`**（多尺寸 16–256）。没有它会用
   Electron 默认图标，装出来像是别人的项目。

### 当前未做

- **代码签名**：安装时 Windows SmartScreen 会提示「未知发布者」。个人使用可忽略；
  要给别人用得买证书（约 $100–300/年）。
- **自动更新**：首版手动下载新版本。
- **macOS / Linux 包**：代码跨平台，但只验证过 Windows。
