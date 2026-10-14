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

- **v0.7 — States, overrides and builds.** `PAP010` to `PAP016`:
  undeclared states, event signatures, read-only properties, instance members
  without an instance, unreachable code, unused locals and parameters; quick
  fixes for the semantic lints — cast, missing arguments, function created,
  closest name —; highlights, go to implementation, call hierarchy, reference
  lenses; `papyrus build` with `PapyrusCompiler.exe` or Caprica. The 0.6.0
  release built for every platform.

- **v0.8 — Skyrim without SKSE, large projects.** Skyrim's tables hold
  vanilla only, SKSE's come with `skse = true` or its sources; the names each
  file mentions, so that references, lenses and rename read only the files
  that may hold a name; the requests reading a whole project run on a copy of
  the model, cancelled by an edit; indexing progress. Measured on Skyrim's
  14,000 vanilla scripts imported: every request within 80 ms.

- **v0.9 — The VSCode extension, polished.** `papyrus.toml` stays the source
  of truth: quick fixes setting a lint's level or SKSE there, created if
  missing; "Create papyrus.toml"; its JSON schema for TOML editors.
  `papyrus build` and `check` as tasks with their problems
  (`--message-format short`), snippets, a TextMate grammar highlighting
  without the server, named arguments completed. `PAP019` duplicate locals,
  `PAP020` the case of names, a function named without parentheses.

- **v0.10 — The grammar, tested.** `vscode-tmgrammar-test` in CI: annotated
  `.psc` files, the scope expected on each token. Fixed on the way: names
  declared with a primitive type, the scopes of `End*` keywords, the
  punctuation of strings and comments.

## Next

- **v0.11 — The extension, tested in VSCode.** A few integration tests with
  `@vscode/test-cli`: activation, "Create papyrus.toml", the `build` and
  `check` tasks, a diagnostic from the server; on Linux, under `xvfb-run`.
- **v0.12 — More lints and formatting rules.** To define: a survey of what
  is missing, against the compiler's errors and the habits of modders.
- **v0.13 — A dedicated IntelliJ plugin**, beyond the generic LSP4IJ: the
  binary embedded, settings, `papyrus build` as a run configuration. Starts
  with a discovery of the best way to match the VSCode extension — Kotlin,
  Gradle, the IntelliJ Platform's LSP API or LSP4IJ.
- **v0.14 — Publication** (may move): the extension on the Marketplace and
  Open VSX (tokens), CI on the release PR.
