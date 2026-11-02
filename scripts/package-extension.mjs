// Packages the VSCode extension for one platform, `papyrus` binary
// included, and archives the bare binary for the GitHub release.
//
//   node scripts/package-extension.mjs --target win32-x64 \
//     --binary target/x86_64-pc-windows-msvc/release/papyrus.exe
//
// Writes to `dist/`:
//   papyrus-tools-<target>-<version>.vsix
//   papyrus-<version>-<target>.zip (Windows) or .tar.gz
//
// `--target universal`, without a binary, packages the extension alone for
// the other platforms: `papyrus` is then taken from the `PATH` or the
// `papyrus.server.path` setting.

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const TARGETS = ["win32-x64", "win32-arm64", "darwin-arm64", "linux-x64", "linux-arm64"];

const { values } = parseArgs({
  options: {
    target: { type: "string" },
    binary: { type: "string" },
    out: { type: "string", default: "dist" },
  },
});
const { target, binary, out } = values;
const universal = target === "universal";
if (!(universal || (TARGETS.includes(target ?? "") && binary))) {
  console.error(`usage: --target <${TARGETS.join("|")}> --binary <path> [--out dist]`);
  console.error("       --target universal [--out dist]");
  process.exit(2);
}

const root = fileURLToPath(new URL("../", import.meta.url));
const extension = join(root, "editors", "vscode");
const dist = resolve(root, out);
const { version } = JSON.parse(readFileSync(join(extension, "package.json"), "utf8"));
const windows = target.startsWith("win32");
const exe = windows ? "papyrus.exe" : "papyrus";

/** Copies the binary into `dir`, executable. */
function install(dir) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  copyFileSync(resolve(binary), join(dir, exe));
  if (!windows) chmodSync(join(dir, exe), 0o755);
}

mkdirSync(dist, { recursive: true });

// The binary goes where `extension.js` looks for it; the changelog, kept at
// the root, is shown on the stores' page.
const server = join(extension, "server");
const changelog = join(extension, "CHANGELOG.md");
rmSync(server, { recursive: true, force: true });
if (!universal) install(server);
copyFileSync(join(root, "CHANGELOG.md"), changelog);
try {
  // vsce's entry point is run with node directly: no shell, no quoting.
  const vsce = join(extension, "node_modules", "@vscode", "vsce", "vsce");
  const name = universal ? `papyrus-tools-${version}.vsix` : `papyrus-tools-${target}-${version}.vsix`;
  const args = [vsce, "package", "--out", join(dist, name)];
  if (!universal) args.push("--target", target);
  execFileSync(process.execPath, args, { cwd: extension, stdio: "inherit" });
} finally {
  rmSync(server, { recursive: true, force: true });
  rmSync(changelog, { force: true });
}

if (universal) {
  console.log(`universal ${version}: extension in ${out}/`);
  process.exit(0);
}

// The bare binary, with the license, for the CLI and other editors.
const archive = `papyrus-${version}-${target}`;
const staging = join(dist, archive);
install(staging);
copyFileSync(join(root, "LICENSE"), join(staging, "LICENSE"));
copyFileSync(join(root, "README.md"), join(staging, "README.md"));
if (windows) {
  const command = `Compress-Archive -Force -Path '${archive}' -DestinationPath '${archive}.zip'`;
  execFileSync("powershell", ["-NoProfile", "-Command", command], { cwd: dist, stdio: "inherit" });
} else {
  execFileSync("tar", ["-czf", `${archive}.tar.gz`, archive], { cwd: dist, stdio: "inherit" });
}
rmSync(staging, { recursive: true, force: true });
console.log(`${target} ${version}: extension and archive in ${out}/`);
