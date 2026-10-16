import {
  app,
  BrowserWindow,
  ipcMain,
  dialog,
  shell,
  Tray,
  Menu,
  nativeImage,
  safeStorage,
} from "electron";
import { readFile, writeFile, mkdir, rename } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { Companion } from "./service.mjs";
import { discoverInstances, launchProfile, ID } from "./profiles.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = pathToFileURL(path.join(here, "ui", "index.html")).href;
let window,
  tray,
  companion,
  quitting = false,
  storageWarning = "",
  initialWarning = "";
const smoke = process.env.KIRK_COMPANION_TEST === "1" && !app.isPackaged;
if (smoke && process.env.KIRK_COMPANION_DATA)
  app.setPath("userData", process.env.KIRK_COMPANION_DATA);
const configFile = path.join(app.getPath("userData"), "settings.json");
// Serialize disk writes so a later change can never be overwritten by an earlier save.
let saving = Promise.resolve();
function canEncrypt() {
  return (
    safeStorage.isEncryptionAvailable() &&
    (process.platform !== "linux" ||
      safeStorage.getSelectedStorageBackend() !== "basic_text")
  );
}
async function load() {
  let data;
  try {
    data = JSON.parse(await readFile(configFile, "utf8"));
  } catch (error) {
    if (error.code !== "ENOENT")
      initialWarning =
        "Saved settings could not be read. Your original file has been kept; save a profile to start fresh.";
    return {};
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    initialWarning =
      "Saved settings could not be read. Pair this computer again.";
    return {};
  }
  const { encryptedToken, token: _discardPlaintext, ...config } = data;
  if (encryptedToken) {
    try {
      config.token = safeStorage.decryptString(
        Buffer.from(encryptedToken, "base64"),
      );
    } catch {
      initialWarning =
        "Your saved pairing could not be unlocked. Pair this computer again.";
    }
  }
  return config;
}
function save(config) {
  const snapshot = structuredClone(config);
  saving = saving
    .catch(() => {})
    .then(async () => {
      const { token, ...disk } = snapshot;
      if (token && canEncrypt())
        disk.encryptedToken = safeStorage
          .encryptString(token)
          .toString("base64");
      storageWarning =
        token && !canEncrypt()
          ? "No system credential store is available. Pairing lasts until you quit this app."
          : "";
      await mkdir(path.dirname(configFile), { recursive: true, mode: 0o700 });
      await writeFile(
        configFile + ".tmp",
        JSON.stringify(disk, null, 2) + "\n",
        { mode: 0o600 },
      );
      await rename(configFile + ".tmp", configFile);
    });
  return saving;
}
function snapshot() {
  return {
    ...companion.snapshot(),
    version: app.getVersion(),
    platform: process.platform,
    storageWarning: storageWarning || initialWarning,
    startupSupported:
      app.isPackaged && ["win32", "darwin"].includes(process.platform),
    startup:
      app.isPackaged && ["win32", "darwin"].includes(process.platform)
        ? app.getLoginItemSettings({
            path: process.execPath,
            args: ["--background"],
          }).openAtLogin
        : false,
  };
}
function show() {
  if (!window || window.isDestroyed()) {
    window = new BrowserWindow({
      width: 1080,
      height: 850,
      minWidth: 680,
      minHeight: 560,
      title: "Kirk Companion",
      backgroundColor: "#1b1d20",
      show: false,
      titleBarStyle: "hidden",
      icon: path.join(here, "ui", "icon.png"),
      ...(process.platform === "darwin"
        ? { trafficLightPosition: { x: 14, y: 16 } }
        : {
            titleBarOverlay: {
              color: "#121315",
              symbolColor: "#c4c5c6",
              height: 48,
            },
          }),
      webPreferences: {
        preload: path.join(here, "preload.cjs"),
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
      },
    });
    window.setMenuBarVisibility(false);
    window.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
    window.webContents.on("will-navigate", (event) => event.preventDefault());
    window.loadFile(path.join(here, "ui", "index.html"));
    window.once("ready-to-show", () => window.show());
    window.on("close", (event) => {
      if (!quitting && tray) {
        event.preventDefault();
        window.hide();
      }
    });
  }
  window.show();
  if (window.isMinimized()) window.restore();
  window.focus();
}
// Deep links only bring the local setup window forward. Remote launching always uses the paired channel.
// No URL, path, command, pairing token, or profile is ever executed from a kirk:// argument.
const locked = app.requestSingleInstanceLock();
if (!locked) app.quit();
else {
  app.on("second-instance", (_event, argv) => {
    if (!argv.includes("--background")) show();
  });
  app.on("open-url", (event) => {
    event.preventDefault();
    if (app.isReady()) show();
  });
  app.on("activate", () => {
    if (companion) show();
  });
  app.on("before-quit", () => {
    quitting = true;
    companion?.stop();
  });
  app.on("window-all-closed", () => {
    if (!tray) app.quit();
  });
  app
    .whenReady()
    .then(async () => {
      let initial = await load();
      try {
        companion = new Companion({
          initial,
          save,
          launch: (p) =>
            launchProfile(p, { openPath: (target) => shell.openPath(target) }),
        });
      } catch {
        initialWarning =
          "The saved server URL is invalid. Pair this computer again.";
        companion = new Companion({
          initial: { profiles: initial.profiles },
          save,
          launch: (p) =>
            launchProfile(p, { openPath: (target) => shell.openPath(target) }),
        });
      }
      companion.on("state", () => {
        if (window && !window.isDestroyed())
          window.webContents.send("companion:state", snapshot());
      });
      const invoke = (name, fn) =>
        ipcMain.handle("companion:" + name, async (event, ...args) => {
          if (
            !window ||
            event.sender !== window.webContents ||
            event.senderFrame !== window.webContents.mainFrame ||
            event.senderFrame.url.split("#", 1)[0] !== ui
          )
            throw new Error("Untrusted caller.");
          try {
            return { ok: true, data: await fn(...args) };
          } catch (error) {
            return {
              ok: false,
              error: error.message || "Something went wrong. Try again.",
            };
          }
        });
      invoke("state", snapshot);
      invoke("pair", (input) => companion.pair(input));
      invoke("unpair", () => companion.unpair());
      // File choices are made in native dialogs. Renderer input cannot substitute a different executable.
      let selectedTarget = null,
        selectedRoot = null;
      invoke("choose", async (kind) => {
        const folder = kind === "root";
        const result = await dialog.showOpenDialog(window, {
          title: folder
            ? "Choose launcher data folder"
            : "Choose launcher or instance shortcut",
          properties: folder ? ["openDirectory"] : ["openFile"],
          ...(process.platform === "win32" && !folder
            ? {
                filters: [
                  {
                    name: "Launchers and shortcuts",
                    extensions: ["exe", "lnk"],
                  },
                ],
              }
            : {}),
        });
        if (result.canceled) return null;
        const chosen = result.filePaths[0];
        if (folder) selectedRoot = chosen;
        else selectedTarget = chosen;
        return chosen;
      });
      invoke("discover", discoverInstances);
      invoke("add", async (input) => {
        if (input.target !== selectedTarget)
          throw new Error(
            "Use Browse to choose the launcher or shortcut first.",
          );
        if (input.root && input.root !== selectedRoot) {
          const discovered = await discoverInstances();
          if (
            !discovered.some(
              (p) =>
                p.root === input.root &&
                p.instance === input.instance &&
                p.kind === input.kind,
            )
          )
            throw new Error("Use Browse to choose the launcher data folder.");
        }
        const result = await companion.addProfile(input);
        selectedTarget = null;
        selectedRoot = null;
        return result;
      });
      invoke("remove", (id) => {
        if (!ID.test(id)) throw new Error("Unknown profile.");
        return companion.removeProfile(id);
      });
      invoke("test", (id) => companion.runProfile(id));
      invoke("website", () =>
        shell.openExternal("https://charlie-kirk.uk/app"),
      );
      invoke("startup", (enabled) => {
        if (!app.isPackaged || !["win32", "darwin"].includes(process.platform))
          throw new Error(
            "Start on login is available in the installed Windows and macOS apps.",
          );
        app.setLoginItemSettings({
          openAtLogin: enabled === true,
          path: process.execPath,
          args: ["--background"],
        });
        return snapshot();
      });
      const icon = nativeImage
        .createFromPath(path.join(here, "ui", "icon.png"))
        .resize({ width: 20, height: 20 });
      tray = new Tray(icon);
      tray.setToolTip("Kirk Companion");
      tray.setContextMenu(
        Menu.buildFromTemplate([
          { label: "Open Kirk Companion", click: show },
          {
            label: "Open website",
            click: () => shell.openExternal("https://charlie-kirk.uk/app"),
          },
          { type: "separator" },
          { label: "Quit", click: () => app.quit() },
        ]),
      );
      tray.on("double-click", show);
      if (app.isPackaged) app.setAsDefaultProtocolClient("kirk");
      if (!process.argv.includes("--background")) show();
      companion.start();
    })
    .catch((error) => {
      dialog.showErrorBox(
        "Kirk Companion could not start",
        String(error.message),
      );
      app.quit();
    });
}
