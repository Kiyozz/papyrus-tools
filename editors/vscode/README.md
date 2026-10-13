# Papyrus

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
  unknown function created —, and "Allow" to silence a lint on one line;
- go to definition, hover, completion, references and rename across the
  project's scripts, down to the game's native ones;
- signature help while typing a call, and the names of the parameters before
  its arguments;
- document formatting;
- document outline, workspace symbols, block folding, semantic highlighting.

## Installation

The extension ships with the `papyrus` binary for its platform and runs
`papyrus lsp`. The `papyrus.server.path` setting points to another binary;
without an embedded one, the extension looks for `papyrus` in the `PATH`.

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
