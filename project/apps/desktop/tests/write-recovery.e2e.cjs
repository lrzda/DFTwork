const { app, BrowserWindow } = require("electron");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const sandbox = process.env.DFTWORK_E2E_SANDBOX;
if (!sandbox) throw new Error("DFTWORK_E2E_SANDBOX is required");
const root = path.join(sandbox, "project");
fs.mkdirSync(path.join(root, "source"), { recursive: true });
fs.mkdirSync(path.join(root, "target"), { recursive: true });
fs.writeFileSync(path.join(root, "source", "OUTCAR"), "energy without entropy\n");
fs.writeFileSync(path.join(root, "source", "INCAR"), "ENCUT = 400\n");
fs.writeFileSync(path.join(root, "source", "KPOINTS"), "GAMMA\n");
fs.writeFileSync(path.join(root, "target", "INCAR"), "ENCUT = 300\n");

app.setPath("userData", path.join(sandbox, "user-data"));
app.disableHardwareAcceleration();
process.env.DFTWORK_SMOKE_PANEL = "build";
process.env.HERMES_BIN = path.join(sandbox, "hermes-not-installed");

let win;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function evaluate(code) {
  return win.webContents.executeJavaScript(code, true);
}
async function waitFor(text, timeout = 20_000) {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    if (win && !win.isDestroyed()) {
      const found = await evaluate(`document.body.innerText.includes(${JSON.stringify(text)})`);
      if (found) return;
    }
    await sleep(100);
  }
  const body = win && !win.isDestroyed() ? await evaluate("document.body.innerText") : "no window";
  throw new Error(`Timed out waiting for UI text: ${text}\n${body}`);
}
async function click(text, selector = "button", exact = true) {
  const clicked = await evaluate(`(() => {
    const elements = [...document.querySelectorAll(${JSON.stringify(selector)})];
    const target = elements.find((el) => {
      const value = el.innerText.replace(/\\s+/g, "");
      return ${exact ? `value === ${JSON.stringify(text.replace(/\s+/g, ""))}` : `value.startsWith(${JSON.stringify(text.replace(/\s+/g, ""))})`};
    });
    if (!target || target.disabled) return false;
    target.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    target.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    target.click();
    return true;
  })()`);
  if (!clicked) {
    const snapshot = await evaluate(`JSON.stringify({
      body: document.body.innerText,
      controls: [...document.querySelectorAll(${JSON.stringify(selector)})].map((el) => ({
        text: el.innerText,
        disabled: el.disabled,
      })),
    })`);
    throw new Error(`Click target not found: ${selector} ${text}\n${snapshot}`);
  }
}
async function setValue(selector, value) {
  const changed = await evaluate(`(() => {
    const input = document.querySelector(${JSON.stringify(selector)});
    if (!input) return false;
    const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), "value").set;
    setter.call(input, ${JSON.stringify(value)});
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  if (!changed) throw new Error(`Input not found: ${selector}`);
}
async function scanProject() {
  await waitFor("扫描");
  await setValue("input[placeholder^='项目根目录']", root);
  await click("扫描");
  await waitFor("任务 1 个");
}
async function generatePlan() {
  await click("source", ".ant-table-row", false);
  await click("以此为模板");
  await waitFor("生成计划");
  await setValue(".ant-modal input", "target");
  await setValue(".ant-modal textarea", "ENCUT = 520");
  await click("生成计划");
  await waitFor("待放行计划已保存");
  await waitFor("放行写入");
}
async function confirmApply() {
  await click("放行写入", "button", false);
  await waitFor("确认写入？");
  await click("确认写入");
}
async function reload() {
  const loaded = new Promise((resolve) => win.webContents.once("did-finish-load", resolve));
  win.webContents.reload();
  await loaded;
}
async function selectActiveReceipt() {
  await waitFor("条回执（最多 100 条）");
  const opened = await evaluate(`(() => {
    const select = document.querySelector(".right-pane .ant-select-selector");
    if (!select) return false;
    select.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    select.click();
    return true;
  })()`);
  if (!opened) throw new Error("Receipt selector not found");
  await sleep(200);
  const selected = await evaluate(`(() => {
    const options = [...document.querySelectorAll(".ant-select-item-option")];
    const option = options.find((el) => /· 可回滚$/.test(el.innerText.trim()));
    if (!option) return false;
    option.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    option.click();
    return true;
  })()`);
  if (!selected) throw new Error("Active receipt not found");
  await waitFor("回滚这次写入");
}

require("../out/main/index.js");

app.whenReady().then(async () => {
  while (BrowserWindow.getAllWindows().length === 0) await sleep(50);
  win = BrowserWindow.getAllWindows()[0];
  while (win.webContents.isLoading()) await sleep(50);

  await scanProject();
  await generatePlan();
  const pendingPath = path.join(root, ".dftwork", "plans", "pending.json");
  assert.equal(fs.existsSync(pendingPath), true);
  const pendingHash = JSON.parse(fs.readFileSync(pendingPath, "utf8")).plan_hash;
  assert.equal(fs.readFileSync(path.join(root, "target", "INCAR"), "utf8"), "ENCUT = 300\n");

  await reload();
  await scanProject();
  await waitFor("待放行计划已保存");
  await waitFor(pendingHash);
  assert.equal(fs.readFileSync(path.join(root, "target", "INCAR"), "utf8"), "ENCUT = 300\n");
  console.log("PASS pending-reload");

  process.env.DFTWORK_TEST_FAILPOINT = "after-first-write";
  await confirmApply();
  await waitFor("检测到中断的写入");
  delete process.env.DFTWORK_TEST_FAILPOINT;
  await click("恢复原状态");
  await waitFor("历史回执：已回滚");
  assert.equal(fs.readFileSync(path.join(root, "target", "INCAR"), "utf8"), "ENCUT = 300\n");
  assert.equal(fs.existsSync(path.join(root, "target", "KPOINTS")), false);
  console.log("PASS ui-crash-recovery");

  await generatePlan();
  await confirmApply();
  await waitFor("已写入");
  assert.equal(fs.readFileSync(path.join(root, "target", "INCAR"), "utf8"), "ENCUT = 520\n");
  assert.equal(fs.readFileSync(path.join(root, "target", "KPOINTS"), "utf8"), "GAMMA\n");
  assert.equal(fs.existsSync(path.join(root, ".dftwork", "plans", "pending.json")), false);
  console.log("PASS ui-apply");

  await reload();
  await scanProject();
  await selectActiveReceipt();
  await click("回滚这次写入");
  await waitFor("回滚这次写入？");
  await click("回滚", "button");
  await waitFor("历史回执：已回滚");
  assert.equal(fs.readFileSync(path.join(root, "target", "INCAR"), "utf8"), "ENCUT = 300\n");
  assert.equal(fs.existsSync(path.join(root, "target", "KPOINTS")), false);
  console.log("PASS receipt-reload-rollback");
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => {
  delete process.env.DFTWORK_TEST_FAILPOINT;
  if (win && !win.isDestroyed()) win.destroy();
  app.exit(process.exitCode ?? 0);
});
