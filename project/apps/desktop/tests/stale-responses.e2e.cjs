const { app, BrowserWindow, ipcMain } = require("electron");
const assert = require("node:assert/strict");
const path = require("node:path");

const sandbox = process.env.DFTWORK_E2E_SANDBOX;
if (!sandbox) throw new Error("DFTWORK_E2E_SANDBOX is required");
app.setPath("userData", path.join(sandbox, "user-data"));
app.disableHardwareAcceleration();

const channels = {
  scan: "core:scan",
  scene: "core:scene",
  convergence: "core:convergence",
  compare: "core:compare",
};
let deferred = new Map();
let testWindow;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function task(name) {
  return {
    rel_path: name,
    status: "TASK_COMPLETED",
    status_label: "已完成",
    kind: "STATIC_SCF",
    kind_label: "静态",
    reason: "test",
    evidence: [],
    inputs: { poscar: true, incar: true, kpoints: true, potcar: true },
    inputs_complete: true,
    missing_inputs: [],
    outputs: { outcar: true, oszicar: true, vasprun: false, contcar: true, xdatcar: false },
    observed_at: null,
  };
}

function scanReport(root, names) {
  return {
    root,
    dirs_scanned: names.length + 1,
    tasks: names.map(task),
    pending_dirs: [],
    elapsed_ms: 1,
    schema_version: 1,
  };
}

function scene(source) {
  return {
    cell: [[4, 0, 0], [0, 4, 0], [0, 0, 4]],
    atoms: [{ element: "Si", position: [0, 0, 0], species_index: 0 }],
    bonds: [],
    comment: source,
    selective_dynamics: false,
    scale: 1,
    source_file: source,
  };
}

function convergence(source) {
  return {
    steps: [{ index: 1, energy_ev: -10, max_force_ev_per_a: 0.1 }],
    reached_required_accuracy: null,
    source,
    truncated: false,
  };
}

function compare(names) {
  return { tasks: names, input_diff: [], same_count: 0, series: [] };
}

function pending(channel) {
  return deferred.get(channels[channel] ?? channel) ?? [];
}

async function waitPending(channel, count) {
  const started = Date.now();
  while (Date.now() - started < 10_000) {
    if (pending(channel).length >= count) return;
    await sleep(25);
  }
  throw new Error(`Timed out waiting for ${count} ${channel} requests`);
}

async function resolve(channel, index, data) {
  const request = pending(channel)[index];
  assert.ok(request, `Missing ${channel} request ${index}`);
  request.resolve({ ok: true, data });
  await sleep(40);
}

for (const channel of Object.values(channels)) {
  ipcMain.handle(channel, (_event, request) => new Promise((resolve) => {
    const requests = deferred.get(channel) ?? [];
    requests.push({ request, resolve });
    deferred.set(channel, requests);
  }));
}
ipcMain.handle("core:version", () => ({
  ok: true,
  data: { name: "vaspcore-mock", version: "0.0.0", schema_version: 1 },
}));
ipcMain.handle("core:hermes-probe", () => ({
  ok: true,
  data: { found: false, bin: null, version: null, supports_stream_json: false, reason: "mock" },
}));
ipcMain.handle("core:project-state", () => ({
  ok: true,
  data: { pending: null, receipts: [], recovery: null, backup_bytes: 0 },
}));

async function openWindow() {
  deferred = new Map();
  const win = testWindow ?? new BrowserWindow({
    width: 1280,
    height: 800,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, "../out/preload/index.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });
  testWindow = win;
  await win.loadFile(path.join(__dirname, "../out/renderer/index.html"));
  const evaluate = (expression) => win.webContents.executeJavaScript(expression, true);
  const waitFor = async (expression, description) => {
    const started = Date.now();
    while (Date.now() - started < 10_000) {
      if (await evaluate(expression)) return;
      await sleep(40);
    }
    const body = await evaluate("document.body.innerText");
    throw new Error(`Timed out waiting for ${description}\n${body}`);
  };
  const click = async (selector, text) => {
    const clicked = await evaluate(`(() => {
      const node = [...document.querySelectorAll(${JSON.stringify(selector)})]
        .find((item) => item.textContent.replace(/\\s/g, "") === ${JSON.stringify(text)});
      if (!node) return false;
      node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
      node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true }));
      node.click();
      return true;
    })()`);
    assert.equal(clicked, true, `Click target not found: ${selector} ${text}`);
  };
  const setRoot = async (root) => {
    const focused = await evaluate(`(() => {
      const input = document.querySelector('input[placeholder^="项目根目录"]');
      if (!input) return false;
      input.focus();
      input.select();
      return true;
    })()`);
    assert.equal(focused, true, "Project root input not found");
    win.webContents.insertText(root);
    await waitFor(`document.querySelector('input[placeholder^="项目根目录"]').value === ${JSON.stringify(root)}`, "project root state");
    await sleep(50);
  };
  const startScan = async (root) => {
    const index = pending("scan").length;
    await setRoot(root);
    if (index === 0) {
      await click("button", "扫描");
    } else {
      await evaluate(`(() => {
        const input = document.querySelector('input[placeholder^="项目根目录"]');
        input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", bubbles: true }));
        input.dispatchEvent(new KeyboardEvent("keyup", { key: "Enter", code: "Enter", bubbles: true }));
      })()`);
    }
    try {
      await waitPending("scan", index + 1);
    } catch (error) {
      throw new Error(`${error.message}\n${await evaluate("document.body.innerText")}`);
    }
    return index;
  };
  const scanNow = async (root, names) => {
    const index = await startScan(root);
    await resolve("scan", index, scanReport(root, names));
    await waitFor(`document.querySelector('.ant-table-tbody')?.innerText.includes(${JSON.stringify(names[0])})`, "scan results");
  };
  const pickTask = async (name) => {
    const picked = await evaluate(`(() => {
      const row = [...document.querySelectorAll('.ant-table-tbody tr.ant-table-row')]
        .find((item) => item.innerText.includes(${JSON.stringify(name)}));
      if (!row) return false;
      row.click();
      return true;
    })()`);
    assert.equal(picked, true, `Task row not found: ${name}`);
  };
  const checkTask = async (name) => {
    const checked = await evaluate(`(() => {
      const row = [...document.querySelectorAll('.ant-table-tbody tr.ant-table-row')]
        .find((item) => item.innerText.includes(${JSON.stringify(name)}));
      const checkbox = row?.querySelector('input[type="checkbox"]');
      if (!checkbox) return false;
      checkbox.click();
      return true;
    })()`);
    assert.equal(checked, true, `Task checkbox not found: ${name}`);
  };
  const body = () => evaluate("document.body.innerText");
  const rightPanel = () => evaluate("document.querySelector('.right-pane').innerText");
  await waitFor("Boolean(window.dftwork) && document.body.innerText.includes('扫描')", "production renderer and preload");
  return { win, evaluate, waitFor, click, startScan, scanNow, pickTask, checkTask, body, rightPanel };
}

async function scanRace() {
  const ui = await openWindow();
  try {
    const first = await ui.startScan("/project-a");
    const second = await ui.startScan("/project-b");
    await resolve("scan", second, scanReport("/project-b", ["task-b"]));
    await ui.waitFor("document.querySelector('.ant-table-tbody')?.innerText.includes('task-b')", "newer scan");
    await resolve("scan", first, scanReport("/project-a", ["task-a"]));
    assert.match(await ui.body(), /task-b/);
    assert.doesNotMatch(await ui.body(), /task-a/);

    const third = await ui.startScan("/project-c");
    const fourth = await ui.startScan("/project-d");
    await resolve("scan", third, scanReport("/project-c", ["task-c"]));
    const loading = await ui.evaluate("document.querySelector('input[placeholder^=\"项目根目录\"]').closest('.ant-space-compact').querySelector('button').classList.contains('ant-btn-loading')");
    assert.equal(loading, true, "Stale scan cleared the current loading state");
    assert.match(await ui.body(), /task-b/);
    assert.doesNotMatch(await ui.body(), /task-c/);
    await resolve("scan", fourth, scanReport("/project-d", ["task-d"]));
    await ui.waitFor("document.querySelector('.ant-table-tbody')?.innerText.includes('task-d')", "latest scan");
  } finally {
    deferred.clear();
  }
}

async function sceneRace() {
  const ui = await openWindow();
  try {
    await ui.scanNow("/project", ["one", "two"]);
    await ui.pickTask("one");
    await waitPending("scene", 1);
    await ui.pickTask("two");
    await waitPending("scene", 2);
    await resolve("scene", 1, scene("SCENE TWO"));
    await ui.waitFor("document.querySelector('.right-pane').innerText.includes('SCENE TWO')", "newer scene");
    await resolve("scene", 0, scene("SCENE ONE"));
    assert.match(await ui.rightPanel(), /SCENE TWO/);
    assert.doesNotMatch(await ui.rightPanel(), /SCENE ONE/);
  } finally {
    deferred.clear();
  }
}

async function convergenceRace() {
  const ui = await openWindow();
  try {
    await ui.scanNow("/project", ["one", "two"]);
    await ui.pickTask("one");
    await waitPending("convergence", 1);
    await ui.pickTask("two");
    await waitPending("convergence", 2);
    await resolve("convergence", 1, convergence("SERIES TWO"));
    await ui.click(".ant-tabs-tab", "收敛");
    await ui.waitFor("document.querySelector('.right-pane').innerText.includes('SERIES TWO')", "newer convergence series");
    await resolve("convergence", 0, convergence("SERIES ONE"));
    assert.match(await ui.rightPanel(), /SERIES TWO/);
    assert.doesNotMatch(await ui.rightPanel(), /SERIES ONE/);
  } finally {
    deferred.clear();
  }
}

async function compareRace() {
  const ui = await openWindow();
  try {
    await ui.scanNow("/project", ["one", "two", "three"]);
    await ui.checkTask("one");
    await ui.checkTask("two");
    await waitPending("compare", 1);
    await ui.checkTask("three");
    await waitPending("compare", 2);
    await resolve("compare", 1, compare(["one", "two", "three"]));
    await ui.waitFor("document.querySelector('.right-pane').innerText.includes('three')", "newer comparison");
    await resolve("compare", 0, compare(["one", "two"]));
    assert.match(await ui.rightPanel(), /three/);
    assert.match(await ui.rightPanel(), /相同参数 0 项/);
  } finally {
    deferred.clear();
  }
}

const watchdog = setTimeout(() => {
  console.error("FAIL stale-response watchdog");
  app.exit(2);
}, 75_000);

app.whenReady().then(async () => {
  for (const [name, scenario] of Object.entries({ scan: scanRace, scene: sceneRace, convergence: convergenceRace, compare: compareRace })) {
    await scenario();
    console.log(`PASS stale-${name}`);
  }
}).catch((error) => {
  console.error(error);
  process.exitCode = 1;
}).finally(() => {
  clearTimeout(watchdog);
  for (const win of BrowserWindow.getAllWindows()) win.destroy();
  app.exit(process.exitCode ?? 0);
});
