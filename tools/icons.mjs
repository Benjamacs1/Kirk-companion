// Rasterize our original vector geometry; no third-party font or image assets.
import { _electron } from "playwright";
import { readFile, writeFile, mkdtemp, rm } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tmp = await mkdtemp(path.join(tmpdir(), "kirk-icons-"));
const script = path.join(tmp, "main.cjs");
await writeFile(
  script,
  "const {app,BrowserWindow}=require('electron');app.whenReady().then(()=>{const w=new BrowserWindow({width:256,height:256,show:false,webPreferences:{sandbox:true}});w.loadURL('about:blank')});",
);
const desktop = await _electron.launch({ args: [script], cwd: root });
try {
  const page = await desktop.firstWindow();
  const svg = await readFile(path.join(root, "src", "ui", "icon.svg"), "utf8");
  const pngUrl = await page.evaluate(async (svg) => {
    const img = new Image();
    img.src = "data:image/svg+xml;base64," + btoa(svg);
    await img.decode();
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 256;
    canvas.getContext("2d").drawImage(img, 0, 0);
    return canvas.toDataURL("image/png");
  }, svg);
  const png = Buffer.from(pngUrl.split(",")[1], "base64");
  await writeFile(path.join(root, "src", "ui", "icon.png"), png);
  // ICO permits a 256x256 PNG image in a single directory entry.
  const header = Buffer.alloc(22);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(1, 4);
  header.writeUInt16LE(1, 10);
  header.writeUInt16LE(32, 12);
  header.writeUInt32LE(png.length, 14);
  header.writeUInt32LE(22, 18);
  await writeFile(
    path.join(root, "src", "ui", "icon.ico"),
    Buffer.concat([header, png]),
  );
  console.log("Generated icon.png and icon.ico from icon.svg");
} finally {
  await desktop.close();
  await rm(tmp, { recursive: true, force: true });
}
