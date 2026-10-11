// Copies the version of package.json, kept by changesets, into the crates and
// the VSCode extension: one version for the whole toolchain.
//
//   node scripts/sync-version.mjs

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");
const write = (path, text) => writeFileSync(new URL(path, root), text);
const readJson = (path) => JSON.parse(read(path));
const writeJson = (path, value) => write(path, `${JSON.stringify(value, null, 2)}\n`);

const { version } = readJson("package.json");
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version)) {
  throw new Error(`version invalide dans package.json : ${version}`);
}

// Cargo.toml: the workspace version, and the one crates require of each other.
const cargo = read("Cargo.toml")
  .replace(/^(\[workspace\.package\][^[]*?\nversion = ")[^"]+(")/m, `$1${version}$2`)
  .replace(/(path = "crates\/[\w-]+", version = ")[^"]+(")/g, `$1${version}$2`);
write("Cargo.toml", cargo);

// The extension and its lockfile.
const extension = readJson("editors/vscode/package.json");
extension.version = version;
writeJson("editors/vscode/package.json", extension);
const lock = readJson("editors/vscode/package-lock.json");
lock.version = version;
lock.packages[""].version = version;
writeJson("editors/vscode/package-lock.json", lock);

// Cargo.lock follows the crate versions.
execFileSync("cargo", ["update", "--workspace", "--quiet"], { cwd: root, stdio: "inherit" });

console.log(`version ${version} : Cargo.toml, Cargo.lock et l'extension à jour`);
