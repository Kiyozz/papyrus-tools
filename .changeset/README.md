# Changesets

Every visible change adds a file here, `0000-my-change.md`:

```md
---
"papyrus-tools": minor
---

Description of the change in English.
```

`patch`, `minor` or `major`. One version covers the CLI, the language server
and the VSCode extension: `npm run release:version` bumps it, writes
`CHANGELOG.md` and copies it into `Cargo.toml` and the extension.
