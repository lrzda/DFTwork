# tests

测试数据的落地位置。**本仓库不携带任何 VASP 样例。**

## 目录约定

```
tests/
├── fixtures/     放真实（或脱敏的）VASP 任务目录，供手工验证与集成测试用
└── golden/       由 scripts/gen-golden.mjs 生成的旧实现基准（生成物，已 gitignore）
```

## 怎么用

1. 准备一个真实的 VASP 项目目录（含 INCAR / POSCAR / OUTCAR / OSZICAR 等）：

   ```
   D:/projects/my-vasp-project/
   ├── relax-01/     ← 含 OUTCAR，算一个任务
   ├── relax-02/
   └── static/
   ```

2. 用它生成对拍基准，然后跑测试：

   ```bash
   node scripts/gen-golden.mjs D:/projects/my-vasp-project
   cargo test -p vasp-cli --test parity
   ```

3. 手工看一眼内核输出：

   ```bash
   ./target/debug/vaspcore scan D:/projects/my-vasp-project
   ```

## 注意

- **没有 golden 时** `parity.rs` 会跳过而不是失败，所以空目录不影响 `cargo test`。
- 不要把含真实未发表数据的目录提交进仓库；需要入库的样例应手动脱敏（改元素、改坐标、截断输出）。
- `POTCAR` 受 VASP 许可保护，**任何情况下都不要提交**。
