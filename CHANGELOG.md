# papyrus-tools

## 0.4.0

### Minor Changes

- New lint `PAP004 events/unknown`: events the engine will never call, such as a misspelled `OnActivatee` or `OnCombatStateChanged` on a quest. The `extends` chain is followed through the project's own scripts down to the native ones.
- New language server, `papyrus lsp`: diagnostics as you type, quick fixes, "allow on this line", formatting, document symbols, folding and semantic highlighting.
- New VSCode extension, shipped with the `papyrus` binary for Windows (x64, arm64), macOS (arm64) and Linux (x64, arm64).
- Licensed under Apache-2.0.

## 0.3.0

### Minor Changes

- `papyrus.toml` configuration: target game, fragments, formatter indent, lint levels, custom `.flg` file.
- The target game is detected from the scripts when not configured.
- Lint registry with in-code suppression (`; papyrus: allow(...)`) and safe fixes (`papyrus check --fix`): `PAP001 encoding/lossy`, `PAP002 flags/placement`, `PAP003 allow/unknown`, `PAP101`–`PAP103` naming conventions.
- Embedded signatures of each game's native scripts.

## 0.2.0

### Minor Changes

- `papyrus check` and `papyrus fmt`.
- Canonical formatter: lowercase keywords, configurable indent, LF, no BOM added.
- Source decoding and encoding checks: text the target compiler would silently corrupt.

## 0.1.0

### Minor Changes

- Lossless lexer and event-based parser for Skyrim, Fallout 4, Fallout 76 and Starfield.
