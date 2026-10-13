# Papyrus

Diagnostics, formatting and lints for **Papyrus**, the scripting language of
the Creation Kit — Skyrim, Fallout 4, Starfield.

- syntax errors as you type, for the targeted game;
- lints: misspelled events the engine will never call (`OnActivatee`),
  misplaced flags, text the compiler will corrupt, naming conventions;
- one-click fixes, and "Allow" to silence a lint on one line;
- go to definition, hover, completion, references and rename across the
  project's scripts, down to the game's native ones;
- document formatting;
- document outline, workspace symbols, block folding, semantic highlighting.

## Installation

The extension ships with the `papyrus` binary for its platform and runs
`papyrus lsp`. The `papyrus.server.path` setting points to another binary;
without an embedded one, the extension looks for `papyrus` in the `PATH`.

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
