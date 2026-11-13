const { app, BrowserWindow } = require("electron");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const sandbox = process.env.DFTWORK_E2E_SANDBOX;
if (!sandbox) throw new Error("DFTWORK_E2E_SANDBOX is required");
const work = path.join(sandbox, "hermes-chat");
const rootA = path.join(work, "project A");
const rootB = path.join(work, "project B");
fs.mkdirSync(rootA, { recursive: true });
fs.mkdirSync(rootB, { recursive: true });
const capture = path.join(work, "capture.txt");
const mock = path.join(work, "mock hermes.cmd");
fs.writeFileSync(
  mock,
  `@echo off
setlocal
set "ARGS=%*"
if "%~1"=="--version" (
  echo Hermes Agent v0.0.0-test
  exit /b 0
)
set "QUERY="
:args
if "%~1"=="" goto run
if "%~1"=="--query-file" set "QUERY=%~2"
shift
goto args
:run
>>"${capture}" echo ---call---
>>"${capture}" echo cwd=%CD%
>>"${capture}" echo args=%ARGS%
>>"${capture}" echo prompt=
type "%QUERY%" >>"${capture}"
echo {"type":"text","text":"mock-response"}
echo {"type":"result","session_id":"session-next","exit_code":0,"text":"mock-response","tokens":{"input":1,"output":2,"total":3,"cache_read":0,"cache_write":0},"duration_ms":12}
`,
);

app.setPath("userData", path.join(sandbox, "user-data"));
app.disableHardwareAcceleration();
process.env.HERMES_BIN = mock;
process.env.DFTWORK_SMOKE_ROOT = rootA;
process.env.DFTWORK_SMOKE_PANEL = "ai";
require("../out/main/index.js");

let win;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function evaluate(expression) {
  return win.webContents.executeJavaScript(expression, true);
}
async function waitFor(expression, label, timeout = 20_000) {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    try {
      if (await evaluate(expression)) return;
    } catch {}
    await sleep(80);
  }
  const body = await evaluate("document.body.innerText").catch(() => "no renderer");
  throw new Error(`Timed out waiting for ${label}\n${body}`);
}
async function setInput(selector, value) {
  const changed = await evaluate(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) return false;
    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value").set.call(element, ${JSON.stringify(value)});
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  assert.equal(changed, true, `Input not found: ${selector}`);
  await waitFor(`document.querySelector(${JSON.stringify(selector)}).value === ${JSON.stringify(value)}`, `input value ${selector}`);
  await sleep(50);
}
async function clickButton(label) {
  const clicked = await evaluate(`(() => {
    const button = [...document.querySelectorAll("button")].find((item) =>
      item.textContent.replace(/\\s/g, "") === ${JSON.stringify(label)}
    );
    if (!button || button.disabled) return false;
    button.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    button.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
    button.click();
    return true;
  })()`);
  assert.equal(clicked, true, `Button not available: ${label}`);
}
async function send(message, expectedResponses) {
  await setInput('textarea[placeholder^="给 Hermes 发消息"]', message);
  await clickButton("发送");
  await waitFor(
    `((document.body.innerText.match(/mock-response/g) || []).length === ${expectedResponses})`,
    `Hermes response count ${expectedResponses}`,
  );
}
function calls() {
  return fs.readFileSync(capture, "utf8").split("---call---").filter((part) => part.includes("cwd="));
}

const watchdog = setTimeout(() => {
  console.error("FAIL hermes-chat watchdog");
  app.exit(2);
}, 75_000);

app.whenReady().then(async () => {
  while (BrowserWindow.getAllWindows().length === 0) await sleep(50);
  win = BrowserWindow.getAllWindows()[0];
  await waitFor('Boolean(document.querySelector(\'textarea[placeholder^="给 Hermes 发消息"]\'))', "Hermes chat panel");

  const invalid = await evaluate('window.dftwork.scan({ root: " " })');
  assert.equal(invalid.ok, false);
  assert.equal(invalid.error?.code, "VALIDATION");
  const invalidChat = await evaluate(`window.dftwork.hermesChat(${JSON.stringify({ root: rootA, message: "", model: "test-model", provider: "test-provider" })})`);
  assert.equal(invalidChat.ok, false);
  assert.equal(invalidChat.error?.code, "VALIDATION");
  console.log("PASS ipc-validation");

  const missing = await evaluate(`window.dftwork.convergence(${JSON.stringify({ taskAbsPath: path.join(work, "missing") })})`);
  assert.equal(missing.ok, false);
  assert.equal(missing.error?.code, "NOT_FOUND");
  console.log("PASS error-code");

  await send("第一条中文消息", 1);
  await send("第二条消息，续接会话", 2);
  let recorded = calls();
  assert.equal(recorded.length, 2);
  assert.ok(recorded[0].includes(`cwd=${rootA}`));
  assert.ok(recorded[1].includes(`cwd=${rootA}`));
  assert.doesNotMatch(recorded[0], /--resume/);
  assert.match(recorded[1], /--resume session-next/);
  assert.ok(recorded[0].includes("第一条中文消息"));
  assert.ok(recorded[1].includes("第二条消息，续接会话"));
  assert.ok(recorded[0].includes("--model deepseek-v4-flash"));
  assert.ok(recorded[0].includes("--provider deepseek"));
  console.log("PASS chat-cwd-resume");

  await setInput('input[placeholder^="项目根目录"]', rootB);
  await clickButton("扫描");
  await waitFor(
    `document.body.innerText.includes(${JSON.stringify(rootB)}) && !document.body.innerText.includes("mock-response")`,
    "project switch and in-memory chat reset",
  );
  await send("项目 B 的新对话", 1);
  recorded = calls();
  assert.equal(recorded.length, 3);
  assert.ok(recorded[2].includes(`cwd=${rootB}`));
  assert.doesNotMatch(recorded[2], /--resume/);
  assert.ok(recorded[2].includes("项目 B 的新对话"));
  console.log("PASS project-switch");
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => {
  clearTimeout(watchdog);
  if (win && !win.isDestroyed()) win.destroy();
  app.exit(process.exitCode ?? 0);
});
