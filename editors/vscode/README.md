# Papyrus Toolchain

Diagnostics, formatting and lints for **Papyrus**, the scripting language of
the Creation Kit — Skyrim, Fallout 4, Starfield.

- syntax errors as you type, for the targeted game;
- lints: misspelled events the engine will never call (`OnActivatee`),
  misplaced flags, text the compiler will corrupt, naming conventions;
- checks across files: missing or extra arguments, values of the wrong type,
  unknown functions and members (with the closest name), typed functions
  that can end without `Return`, undeclared states, events with the wrong
  parameters, read-only properties assigned, unreachable code, unused
  variables and parameters;
- one-click fixes — the closest name, a cast, the missing arguments, the
  unknown function created —, "Allow" to silence a lint on one line, and,
  written to `papyrus.toml`, a lint's level for the whole project or SKSE
  enabled;
- go to definition, hover, completion — named arguments included —, references and rename across the
  project's scripts, down to the game's native ones;
- the uses of the symbol under the cursor, the overrides of a function or an
  event, the call hierarchy, and reference counts above declarations;
- signature help while typing a call, and the names of the parameters before
  its arguments;
- document formatting;
- document outline, workspace symbols, block folding, semantic highlighting;
- highlighting and snippets — `scriptname`, `function`, `event`,
  `property`, `state`, `if`, `while`, `foreach`… — even before the server
  answers;
- `papyrus build` and `papyrus check` as tasks, their problems in the
  Problems view.

## Commands and tasks

- **Papyrus: Create papyrus.toml** writes one at the root of a workspace
  folder, the game inferred from its scripts, the other keys commented out.
- **Papyrus: Restart the server**.
- **Papyrus: Compile this script**, in the palette and the context menus of
  the editor and the explorer, saves and compiles the open or selected scripts
  alone, their errors in the Problems view. It needs their `papyrus.toml`.
- **Tasks: Run Build Task** › `papyrus: build` compiles the folder's projects
  with the compiler of `papyrus.toml`; `papyrus: check` reports the lints of
  every script, open or not. In `tasks.json`:

  ```json
  { "type": "papyrus", "command": "build", "args": ["--release"] }
  ```

## Installation

The extension ships with the `papyrus` binary for its platform and runs
`papyrus lsp`. The `papyrus.server.path` setting points to another binary;
without an embedded one, the extension looks for `papyrus` in the `PATH`.

Windows, macOS and Linux, x64 and arm64, get the binary. Elsewhere — Linux on
32-bit ARM, Alpine… — the extension comes without it: install
`papyrus` from the [GitHub releases](https://github.com/Kiyozz/papyrus-tools/releases),
or build it with `cargo install --git https://github.com/Kiyozz/papyrus-tools papyrus-cli`.

The [Papyrus extension of joelday](https://marketplace.visualstudio.com/items?itemName=joelday.papyrus-lang-vscode)
claims the same `.psc` files: with both, the diagnostics and completions come
twice. Disable one of them for the workspace.

## Settings

- `papyrus.inlayHints.parameterNames`: `literals` (default) shows the name of
  the parameter before literal arguments, `Disable(abFadeOut: true)`; `all`
  before every argument not already named like it; `none` hides them.
  `editor.inlayHints.enabled` turns every hint off.
- `papyrus.server.path`: another `papyrus` binary.

## Configuration

A `papyrus.toml` at the project root sets the game, the indent, the level of
each lint and the folders of scripts the project builds on. Without it, the
game is inferred from the scripts. A window with several projects gives each
one its own `papyrus.toml`.

```toml
game = "skyrim"
imports = ["../SkyUI/Scripts/Source"]

[fmt]
indent = 2

[lints]
"naming/variables" = "allow"
```

With [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml),
`papyrus.toml` gets completion and validation of its keys and lints, from
[its JSON schema](https://raw.githubusercontent.com/Kiyozz/papyrus-tools/main/schemas/papyrus.schema.json).
