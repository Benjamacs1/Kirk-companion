import { _electron } from "playwright";
import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const executable = path.join(
  root,
  "dist",
  "win-unpacked",
  "Kirk Companion.exe",
);
const desktop = await _electron.launch({
  executablePath: executable,
  cwd: root,
});
try {
  const page = await desktop.firstWindow();
  await page.waitForFunction(
    () => !!document.getElementById("version")?.textContent,
  );
  const state = await page.evaluate(() => window.kirk.state());
  assert.equal(state.version, "0.1.0");
  assert.equal(state.startupSupported, true);
  assert.equal(await desktop.evaluate(({ app }) => app.isPackaged), true);
  assert.equal(await page.evaluate(() => typeof window.require), "undefined");
  assert.equal(await page.locator("#appearance").inputValue(), "ghost");
  console.log(
    "Packaged Windows app passed: ASAR assets, preload IPC, version, theme and installed-app settings.",
  );
} finally {
  await desktop.close();
}
