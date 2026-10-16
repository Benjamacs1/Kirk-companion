import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { stat, readdir, readFile } from "node:fs/promises";
import path from "node:path";
import os from "node:os";

export const ID =
  /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i;
const KINDS = new Set(["prism", "multimc", "shortcut", "open"]);
export function validateProfile(input, id = randomUUID()) {
  if (!input || !KINDS.has(input.kind))
    throw new Error("Choose a launcher type.");
  const name = typeof input.name === "string" ? input.name.trim() : "";
  if (!name || name.length > 80)
    throw new Error("Give this profile a name (up to 80 characters).");
  if (!ID.test(id)) throw new Error("Invalid profile ID.");
  const target = input.target;
  if (
    typeof target !== "string" ||
    !path.isAbsolute(target) ||
    target.length > 4096 ||
    /[\x00-\x1f]/.test(target)
  ) {
    throw new Error("Choose a local launcher or shortcut.");
  }
  const profile = { id, name, kind: input.kind, target };
  if (input.kind === "prism" || input.kind === "multimc") {
    const instance =
      typeof input.instance === "string" ? input.instance.trim() : "";
    if (
      !instance ||
      instance.length > 200 ||
      /[\x00-\x1f/\\]/.test(instance) ||
      instance.startsWith("-") ||
      instance === "." ||
      instance === ".."
    ) {
      throw new Error("Enter the instance folder name, without a path.");
    }
    profile.instance = instance;
    const root = input.root || "";
    if (
      typeof root !== "string" ||
      (root &&
        (!path.isAbsolute(root) ||
          /[\x00-\x1f]/.test(root) ||
          root.length > 4096))
    ) {
      throw new Error("Choose an absolute launcher data folder.");
    }
    profile.root = root;
    // macOS apps are bundles; execute their actual launcher binary for CLI arguments.
    if (process.platform === "darwin" && target.endsWith(".app"))
      throw new Error(
        "Choose the launcher executable inside the app bundle for direct launch.",
      );
  }
  return profile;
}

export function publicProfiles(profiles) {
  return profiles.map(({ id, name, kind }) => ({
    id,
    name,
    launcher: kind,
    capability: kind === "open" ? "open" : "launch",
  }));
}

export function launchArguments(profile) {
  if (profile.kind !== "prism" && profile.kind !== "multimc")
    throw new Error("This profile uses a shortcut.");
  return [
    ...(profile.root ? ["--dir", profile.root] : []),
    "--launch",
    profile.instance,
  ];
}

export async function launchProfile(
  profile,
  { openPath, spawnImpl = spawn } = {},
) {
  await stat(profile.target).catch(() => {
    throw new Error("The launcher was moved or removed. Choose it again.");
  });
  if (profile.kind === "prism" || profile.kind === "multimc") {
    // An argument vector is passed directly to the executable; never to a shell.
    await new Promise((resolve, reject) => {
      const child = spawnImpl(profile.target, launchArguments(profile), {
        detached: true,
        stdio: "ignore",
        shell: false,
        windowsHide: true,
      });
      child.once("error", () =>
        reject(
          new Error(
            "Could not start the launcher. Check the selected executable.",
          ),
        ),
      );
      child.once("spawn", () => {
        child.unref();
        resolve();
      });
    });
  } else {
    const error = await openPath(profile.target);
    if (error)
      throw new Error("Could not open the selected launcher or shortcut.");
  }
  return {
    status: "dispatched",
    capability: profile.kind === "open" ? "open" : "launch",
    profile: profile.id,
  };
}

export function launcherRoots(
  platform = process.platform,
  home = os.homedir(),
  env = process.env,
) {
  if (platform === "win32") {
    const roaming = env.APPDATA || path.join(home, "AppData", "Roaming");
    return [
      ["prism", path.join(roaming, "PrismLauncher")],
      ["multimc", path.join(roaming, "MultiMC")],
    ];
  }
  if (platform === "darwin")
    return ["PrismLauncher", "MultiMC"].map((n) => [
      n === "MultiMC" ? "multimc" : "prism",
      path.join(home, "Library", "Application Support", n),
    ]);
  const data = env.XDG_DATA_HOME || path.join(home, ".local", "share");
  return [
    ["prism", path.join(data, "PrismLauncher")],
    ["multimc", path.join(data, "multimc")],
    [
      "prism",
      path.join(
        home,
        ".var",
        "app",
        "org.prismlauncher.PrismLauncher",
        "data",
        "PrismLauncher",
      ),
    ],
  ];
}

export async function discoverInstances(roots = launcherRoots()) {
  const found = [];
  for (const [kind, root] of roots) {
    const folder = path.join(root, "instances");
    const entries = await readdir(folder, { withFileTypes: true }).catch(
      () => [],
    );
    for (const entry of entries.slice(0, 100)) {
      if (!entry.isDirectory()) continue;
      const file = path.join(folder, entry.name, "instance.cfg");
      const text = await readFile(file, "utf8").catch(() => "");
      if (!text) continue;
      const name = /^name=(.*)$/m.exec(text)?.[1]?.trim() || entry.name;
      found.push({ kind, root, instance: entry.name, name: name.slice(0, 80) });
    }
  }
  return found;
}
