# Roadmap

Each milestone is a minor version: `v0.6` ships as `0.6.0`. What each
version changed in detail is in [CHANGELOG.md](CHANGELOG.md).

## Done

- **v0.1 — Parser.** Lossless lexer and event-based parser for Skyrim,
  Fallout 4, Fallout 76 and Starfield. Exit criterion: every vanilla script
  parses without error, and its tree rebuilds the source byte for byte.
- **v0.2 — `check` and `fmt`.** The `papyrus` binary, the canonical
  formatter, source decoding and encoding checks. Exit criterion: idempotent
  formatting of the whole vanilla corpus.
- **v0.3 — Configuration and lints.** `papyrus.toml`, the target game
  inferred from the scripts, the lint registry with in-code suppression and
  safe fixes, the embedded signatures of the native scripts.
- **v0.4 — Language server.** `PAP004 events/unknown`, `papyrus lsp`, the
  VSCode extension with the binary embedded per platform.
- **v0.5 — Cross-file index.** The `papyrus-hir` semantic model on `salsa`:
  definition, hover, completion, references, rename, workspace symbols;
  `imports` in `papyrus.toml`; `--fix --unsafe` for the case of properties;
  settings and index per project.
- **v0.6 — Types and semantic lints.** The compiler's implicit casts and
  complete `extends` chains; `PAP005` to `PAP009` (arity, named arguments,
  type mismatches, unknown members, missing returns), checked against every
  game without a target; the properties of native scripts; signature help
  and parameter name hints. No false positive on the vanilla corpus.

## Next

- **v0.7 — Planned, not started.**
  - **Release 0.6.0**: Marketplace and Open VSX tokens, CI on the release PR.
  - **Lints**, certain only: `GotoState` to an undeclared state, an event
    whose parameters differ from the native one, assignment to an
    `AutoReadOnly` or `Const` property, a non-`Global` function called on its
    script or `self` in a `Global` one, code after `Return`, unused locals and
    parameters.
  - **Quick fixes** for the semantic lints: add `as int`, add the missing
    argument, create the unknown function, replace with the closest name.
  - **Navigation**: highlight the symbol under the cursor, go to overrides,
    call hierarchy, references CodeLens.
  - **`papyrus build`**: drive `PapyrusCompiler.exe` or Caprica with the
    imports of `papyrus.toml`, its errors reported like the lints'.
  - **Upkeep**: Skyrim tables without SKSE, or SKSE as an option; an IntelliJ
    integration beyond LSP4IJ; large projects in the language server.
