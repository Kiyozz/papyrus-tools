// Integration tests: a VSCode downloaded into `.vscode-test`, the extension
// from `dist/extension.js` (`npm run bundle` first), the fixture folder open.
// The server is the `papyrus` of the `PATH`, `target/debug` first:
// `cargo build -p papyrus-cli` before, or `PAPYRUS_BIN_DIR` for another one.
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@vscode/test-cli";

const here = path.dirname(fileURLToPath(import.meta.url));
const bin = process.env.PAPYRUS_BIN_DIR ?? path.resolve(here, "../../target/debug");
// `Path` on Windows: a second `PATH` key would make two.
const key = Object.keys(process.env).find((name) => name.toUpperCase() === "PATH") ?? "PATH";

export default defineConfig({
  files: "tests/integration/**/*.test.js",
  workspaceFolder: path.join(here, "tests/fixture"),
  launchArgs: ["--disable-extensions"],
  env: { [key]: `${bin}${path.delimiter}${process.env[key]}` },
  mocha: { ui: "bdd", timeout: 60_000 },
});
