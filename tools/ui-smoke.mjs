import { _electron } from "playwright";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { WebSocketServer } from "ws";
import { mkdtemp, mkdir, writeFile, readFile, rm } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tmp = await mkdtemp(path.join(tmpdir(), "kirk-companion-ui-"));
const artifacts = path.join(root, "artifacts");
await mkdir(artifacts, { recursive: true });
const captured = path.join(tmp, "launch-args.txt");
let target;
if (process.platform === "win32") {
  const source = path.join(tmp, "launcher.cs");
  target = path.join(tmp, "Test Launcher.exe");
  await writeFile(
    source,
    'using System.IO; class Launcher { static void Main(string[] args) { File.WriteAllLines(@"' +
      captured.replaceAll('"', '""') +
      '", args); } }',
  );
  const compiler = path.join(
    process.env.WINDIR || "C:\\Windows",
    "Microsoft.NET",
    "Framework64",
    "v4.0.30319",
    "csc.exe",
  );
  execFileSync(compiler, [
    "/nologo",
    "/target:winexe",
    "/out:" + target,
    source,
  ]);
} else {
  target = path.join(tmp, "launcher");
  await writeFile(
    target,
    '#!/bin/sh\nprintf "%s\\n" "$@" > ' +
      "'" +
      captured.replaceAll("'", "'\\''") +
      "'\n",
    { mode: 0o700 },
  );
}
const token = "T".repeat(43);
let peer,
  hello,
  revoked = false;
const server = createServer(async (req, res) => {
  if (req.url === "/companion/claim") {
    let text = "";
    for await (const part of req) text += part;
    const body = JSON.parse(text);
    assert.equal(body.code, "ACDEFHJK");
    assert.equal(body.name, "Test gaming PC");
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify({ token }));
  } else if (req.url === "/companion/revoke") {
    revoked = req.headers.authorization === "Bearer " + token;
    res.end("{}");
  } else {
    res.statusCode = 404;
    res.end("{}");
  }
});
const sockets = new WebSocketServer({ server });
sockets.on("connection", (ws) => {
  peer = ws;
  ws.on("message", (raw) => {
    const msg = JSON.parse(String(raw));
    if (msg.type === "hello") hello = msg;
  });
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const backend = `http://127.0.0.1:${server.address().port}`;
let app;
const until = async (fn) => {
  const deadline = Date.now() + 5000;
  while (!(await fn())) {
    if (Date.now() > deadline) throw new Error("Timed out.");
    await new Promise((r) => setTimeout(r, 20));
  }
};
try {
  app = await _electron.launch({
    args: [root],
    cwd: root,
    env: {
      ...process.env,
      KIRK_COMPANION_TEST: "1",
      KIRK_COMPANION_DATA: path.join(tmp, "data"),
    },
    timeout: 30000,
  });
  const page = await app.firstWindow();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.locator("#device-name").waitFor();
  await page.waitForFunction(() => !!document.getElementById("backend").value);
  assert.equal(await page.evaluate(() => typeof window.require), "undefined");
  await page.locator("#device-name").fill("Gaming PC");
  await page.screenshot({
    path: path.join(artifacts, "setup-empty.png"),
    fullPage: true,
  });
  await page
    .getByRole("link", { name: "Launch profiles", exact: true })
    .click();
  assert.equal(await page.evaluate(() => location.hash), "#profiles-card");
  await page.getByRole("button", { name: "+ Add profile" }).click();
  await page.locator("#profile-name").fill("Kirk test instance");
  await app.evaluate(({ dialog }, file) => {
    dialog.showOpenDialog = async () => ({
      canceled: false,
      filePaths: [file],
    });
  }, target);
  await page.locator("#choose-target").click();
  await page.locator("#instance").fill("26.2 (Kirk test)");
  await page.getByRole("button", { name: "Save profile", exact: true }).click();
  await page
    .getByRole("button", {
      name: "Test launch Kirk test instance",
      exact: true,
    })
    .waitFor();
  await page.locator("#device-name").fill("Test gaming PC");
  await page.locator("#code").fill("ACDE FHJK");
  await page.locator("#advanced").evaluate((el) => {
    el.open = true;
  });
  await page.locator("#backend").fill(backend);
  await page
    .getByRole("button", { name: "Pair computer", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      document.getElementById("status").textContent === "Ready for the website",
  );
  await until(() => hello?.profiles?.length === 1);
  assert.ok(!JSON.stringify(hello).includes(target));
  const reply = new Promise((resolve) => {
    const listener = (raw) => {
      const msg = JSON.parse(String(raw));
      if (msg.id) {
        peer.off("message", listener);
        resolve(msg);
      }
    };
    peer.on("message", listener);
  });
  peer.send(
    JSON.stringify({
      id: "smoke_launch_1234567890",
      op: "launch_game",
      profile: hello.profiles[0].id,
      expires_at: Date.now() + 10000,
    }),
  );
  assert.equal((await reply).ok, true);
  await until(async () => {
    try {
      return (await readFile(captured, "utf8")).includes("26.2 (Kirk test)");
    } catch {
      return false;
    }
  });
  assert.deepEqual((await readFile(captured, "utf8")).trim().split(/\r?\n/), [
    "--launch",
    "26.2 (Kirk test)",
  ]);
  const disk = JSON.parse(
    await readFile(path.join(tmp, "data", "settings.json"), "utf8"),
  );
  assert.ok(!JSON.stringify(disk).includes(token));
  await page.evaluate(() => {
    document.documentElement.style.scrollBehavior = "auto";
    window.scrollTo(0, 0);
  });
  await page.screenshot({
    path: path.join(artifacts, "setup-connected.png"),
    fullPage: true,
  });
  await page.locator("#appearance").selectOption("kirk");
  assert.equal(
    await page.evaluate(() =>
      getComputedStyle(document.documentElement)
        .getPropertyValue("--accent")
        .trim(),
    ),
    "#0a84ff",
  );
  await app.evaluate(({ BrowserWindow }) =>
    BrowserWindow.getAllWindows()[0].setSize(700, 850),
  );
  assert.equal(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
    true,
  );
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({
    path: path.join(artifacts, "setup-narrow.png"),
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Unpair computer", exact: true })
    .click();
  await page.waitForFunction(
    () => document.getElementById("status").textContent === "Not paired",
  );
  assert.equal(revoked, true);
  await page
    .getByRole("button", { name: "Remove Kirk test instance", exact: true })
    .click();
  await page.waitForFunction(
    () => document.querySelectorAll(".profile").length === 0,
  );
  assert.deepEqual(errors, []);
  console.log(
    "Desktop smoke passed: native file selection, saved profile, pairing, real process launch, credential storage, both themes, narrow layout, unpair and removal.",
  );
} finally {
  if (app) await app.close();
  for (const ws of sockets.clients) ws.terminate();
  await new Promise((r) => sockets.close(r));
  await new Promise((r) => server.close(r));
  await rm(tmp, { recursive: true, force: true });
}
