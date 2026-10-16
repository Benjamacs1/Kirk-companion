import { ZipArchive } from "archiver";
import { createWriteStream } from "node:fs";
import { mkdir, readFile, writeFile, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { version } = JSON.parse(
  await readFile(path.join(root, "package.json"), "utf8"),
);
await mkdir(path.join(root, "dist"), { recursive: true });
const destination = path.join(
  root,
  "dist",
  `kirk-companion-${version}-source.zip`,
);
const zip = new ZipArchive({ zlib: { level: 9 } });
const out = createWriteStream(destination);
const done = new Promise((resolve, reject) => {
  out.on("close", resolve);
  zip.on("error", reject);
  out.on("error", reject);
});
zip.pipe(out);
// Explicit source allowlist: never include userData, credentials, logs, binaries, .git or node_modules.
for (const folder of ["src", "tests", "tools", "docs", ".github"])
  zip.directory(path.join(root, folder), folder);
for (const file of [
  "package.json",
  "package-lock.json",
  "LICENSE",
  "README.md",
  "SECURITY.md",
  "THIRD_PARTY_NOTICES.md",
  ".gitignore",
  ".gitattributes",
])
  zip.file(path.join(root, file), { name: file });
await zip.finalize();
await done;
const files = (await readdir(path.join(root, "dist"))).filter(
  (name) =>
    name === path.basename(destination) ||
    name === `Kirk Companion Setup ${version}.exe`,
);
const sums = [];
for (const name of files)
  sums.push(
    createHash("sha256")
      .update(await readFile(path.join(root, "dist", name)))
      .digest("hex") +
      "  " +
      name,
  );
await writeFile(
  path.join(root, "dist", "SHA256SUMS.txt"),
  sums.join("\n") + "\n",
);
console.log(destination);
