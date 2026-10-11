// Prints the notes of one version, taken from CHANGELOG.md.
//
//   node scripts/release-notes.mjs 0.4.0 > notes.md

import { readFileSync } from "node:fs";

const version = process.argv[2];
if (!version) {
  console.error("usage : release-notes.mjs <version>");
  process.exit(2);
}

const changelog = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8");
const lines = changelog.split(/\r?\n/);
const start = lines.findIndex((line) => line.trim() === `## ${version}`);
if (start === -1) {
  console.error(`CHANGELOG.md n'a pas de section ${version}`);
  process.exit(1);
}
const end = lines.findIndex((line, i) => i > start && line.startsWith("## "));
const section = lines.slice(start + 1, end === -1 ? undefined : end);
console.log(section.join("\n").trim());
