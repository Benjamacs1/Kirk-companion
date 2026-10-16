import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtemp, writeFile, mkdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  validateProfile,
  publicProfiles,
  launchArguments,
  launchProfile,
  discoverInstances,
} from "../src/profiles.mjs";

test("direct launch uses an argument vector; metadata never includes local paths", async () => {
  const p = validateProfile({
    name: "<b>Main</b>",
    kind: "prism",
    target: process.execPath,
    root: path.join(os.tmpdir(), "portable launcher"),
    instance: "26.2 (Kirk)",
  });
  assert.deepEqual(launchArguments(p), [
    "--dir",
    p.root,
    "--launch",
    "26.2 (Kirk)",
  ]);
  assert.deepEqual(publicProfiles([p]), [
    { id: p.id, name: "<b>Main</b>", launcher: "prism", capability: "launch" },
  ]);
  let call;
  await launchProfile(p, {
    spawnImpl: (...args) => {
      call = args;
      const child = new EventEmitter();
      child.unref = () => {};
      queueMicrotask(() => child.emit("spawn"));
      return child;
    },
  });
  assert.equal(call[0], process.execPath);
  assert.equal(call[2].shell, false);
  assert.deepEqual(call[1], ["--dir", p.root, "--launch", p.instance]);
});
test("invalid paths, instance traversal and launch flags cannot become profiles", () => {
  for (const instance of [
    "../x",
    "..",
    ".",
    "--help",
    "a/b",
    "a\\b",
    "x\u0000",
  ]) {
    assert.throws(() =>
      validateProfile({
        name: "a",
        kind: "prism",
        target: process.execPath,
        instance,
      }),
    );
  }
  for (const target of [
    "relative.exe",
    "https://example.com/a.exe",
    "kirk://launch",
  ]) {
    assert.throws(() => validateProfile({ name: "a", kind: "open", target }));
  }
  assert.throws(() =>
    validateProfile({ name: "a", kind: "shell", target: process.execPath }),
  );
});
test("shortcut fallback distinguishes open-only behavior and reports opening errors", async () => {
  const p = validateProfile({
    name: "Other launcher",
    kind: "open",
    target: process.execPath,
  });
  let opened;
  assert.equal(
    (
      await launchProfile(p, {
        openPath: async (file) => {
          opened = file;
          return "";
        },
      })
    ).capability,
    "open",
  );
  assert.equal(opened, process.execPath);
  await assert.rejects(
    launchProfile(p, { openPath: async () => "OS failed" }),
    /Could not open/,
  );
  await assert.rejects(
    launchProfile({
      ...p,
      target: path.join(os.tmpdir(), "missing-kirk-launcher-987.exe"),
    }),
    /moved or removed/,
  );
});
test("instance discovery reads instance metadata, skips unrelated directories and missing installs", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "kirk-discovery-"));
  try {
    await mkdir(path.join(root, "instances", "26.2(kirk)"), {
      recursive: true,
    });
    await mkdir(path.join(root, "instances", "not-an-instance"));
    await writeFile(
      path.join(root, "instances", "26.2(kirk)", "instance.cfg"),
      "[General]\nname=My Kirk instance\n",
    );
    assert.deepEqual(
      await discoverInstances([
        ["prism", root],
        ["multimc", path.join(root, "missing")],
      ]),
      [
        {
          kind: "prism",
          root,
          instance: "26.2(kirk)",
          name: "My Kirk instance",
        },
      ],
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
