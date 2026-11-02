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

- **v0.11 — The extension, tested in VSCode.** Integration tests with
  `@vscode/test-cli` on a fixture folder: activation, a diagnostic from the
  server, the `build` and `check` tasks and their problems, "Create
  papyrus.toml"; in CI under `xvfb-run`, against `target/debug/papyrus`.

- **v0.12 — More lints and formatting rules.** What the compilers refuse,
  each case checked against the three official ones: `PAP021` to `PAP033`,
  overrides with defaults compared as written, casts and operands in
  `PAP007`. Modders' habits, `PAP034` to `PAP040`. The formatter sets blocks
  apart, drops the parentheses of conditions, writes keywords in PascalCase
  on demand.

- **v0.13 — The CLI and the server, completed.** `papyrus init`, `papyrus
  explain`, `--stdin`, a baseline for older mods, `--message-format json` and
  SARIF; `--fix` removing what does nothing, `--fix --unsafe` renaming across
  files; a script renamed with its file, namespaces following folders, range
  formatting, type hierarchy; recursion shown in the editor, `PAP041`.
  Exit criterion: vanilla scripts of the three games compile after the fixes.

- **v0.14 — `.ppj` projects**, shipped with v0.15. Pyro's and the Creation
  Kit's project files read as a configuration, and named from `papyrus.toml`.
  Exit criterion: same files and verdict as the official compilers.

- **v0.15 — `.pex`.** `papyrus disassemble` in Rust, a read-only view in the
  editor; one script compiled alone, from the command line and VSCode. Exit
  criterion: the text of `PapyrusAssembler` on the three games, tables in the
  order of the file.

## Next

- **v0.16 — Publication.** The extension on the Marketplace and Open VSX by
  trusted publishing (OIDC), a universal package for the other platforms; the
  release PR opened by a GitHub App, its CI run.
- **v0.17 — A debugger.** Starts with a discovery: reuse or fork the xSE plugin
  of joelday's `papyrus-lang`, or write one; the DAP adapter on our side.
- **v0.18 — A dedicated IntelliJ plugin**, beyond the generic LSP4IJ: the
  binary embedded, settings, `papyrus build` as a run configuration. Starts
  with a discovery of the best way to match the VSCode extension — Kotlin,
  Gradle, the IntelliJ Platform's LSP API or LSP4IJ.
