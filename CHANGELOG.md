# papyrus-tools

## 0.9.0

### Minor Changes

- 39974bc: Quick fixes that write to `papyrus.toml`, created if missing: "Allow `lint` in papyrus.toml", "Make `lint` an error / a warning in papyrus.toml", and "Enable SKSE in papyrus.toml" on `members/skse`. Comments and layout of the file are kept. The server also answers `papyrus.server.createConfig`, which writes a `papyrus.toml` with the inferred game.
- 39974bc: JSON schema of `papyrus.toml`, `schemas/papyrus.schema.json`, generated from the lint registry by `cargo xtask schema`: completion and validation of its keys and lints in TOML editors such as Even Better TOML.
- 39974bc: VSCode extension: "Papyrus: Create papyrus.toml" command; `papyrus: build` and `papyrus: check` tasks with a problem matcher; snippets; a richer TextMate grammar — declarations, types, calls, operators — highlighting before the server answers.
- 39974bc: `papyrus check` and `papyrus build` take `--message-format short`: one line per problem, `path:line:column:end line:end column: error[code]: message`, for editors' problem matchers.
- 39974bc: New lint `locals/duplicate` (PAP019): a local declared twice in a block, named like a parameter or a variable of the script, or hiding one of an enclosing block — what the Creation Kit's compiler refuses. Redeclared with a value, the quick fix assigns instead. `papyrus build` also says when the flags file is in no source folder: with Caprica, the game's sources belong in `imports`.
- 39974bc: Completion of named arguments: at the start of an argument, the parameters not given yet are proposed first, `value = `, with their type and default value.
- 39974bc: New lint `names/case` (PAP020): a name written with another case than its declaration — a call, a variable, a script, a type —, which the compiler accepts. The fix writes the declared name; `papyrus check --fix` and "fix all" apply it. When the use follows the naming convention its declaration breaks — `Initialize()` calling `Function initialize()` —, the editor offers instead to rename the declaration across the project, for the project's own functions, script variables and locals.
- 39974bc: `members/unknown` (PAP008): a function named as a value — `MyScript.Run`, `self.Run`, `x = Run` — is reported as a function to call, with a fix adding the parentheses, instead of a missing property.

### Patch Changes

- 39974bc: The naming lints say which style a name is in before the expected one: "`a_reference` is in snake_case, but a parameter is written in lowerCamelCase: `aReference`."

## 0.8.0

### Minor Changes

- b0af035: Skyrim's embedded native scripts are now vanilla only. SKSE's functions, events and scripts come with `skse = true` in `papyrus.toml`, or on their own when the project or its `imports` hold SKSE's sources (`SKSE.psc`). Without them, calling an SKSE function such as `Game.GetPerkPoints()`, or naming one of its scripts such as `StringUtil`, is the new `members/skse` (PAP017), a warning inviting to install SKSE and enable it.
- e1bf870: The language server keeps up with projects importing a whole game. Each file remembers which names it mentions, so references, reference lenses, rename and the call hierarchy only read the files that may use a name: two to three times faster on Skyrim's 14,000 vanilla scripts, within 80 ms for the most used names. These requests and completion now run on a copy of the model, off the server's lock: an edit cancels them, and the editor asks again (`ContentModified`). Indexing the workspace reports its progress to editors that show it.
- 13c451e: New lint `scripts/unknown` (PAP018): a script, as a type or as the receiver of a call, that neither the project nor its `imports` declare, with the closest name as a quick fix. It only speaks once the game's sources are in the project or its `imports`, since the embedded tables only know the native scripts.

## 0.7.0

### Minor Changes

- 2d1c3ff: New lints: `states/unknown` (PAP010) for `GotoState` to an undeclared state, `events/signature` (PAP011) for an event whose parameters differ from the engine's, `properties/readonly` (PAP012) for an assignment to an `AutoReadOnly`, `Const` or `Set`-less property, `calls/not-global` (PAP013) for an instance member reached from a `Global` function or on the script itself, and as warnings `code/unreachable` (PAP014), `locals/unused` (PAP015) and `params/unused` (PAP016). Unused parameters are only reported after the last one read, never on events, overrides or fragments.
- 27a3b70: The editor offers quick fixes for the semantic lints: cast a value with `as`, add the missing arguments of a call, create the unknown function with a parameter per argument, or replace a misspelled name with the closest one. A cast between objects is only suggested toward a descendant, which the compiler accepts.
- f9d7f59: The language server highlights the uses of the symbol under the cursor, telling writes from reads, goes to the overrides of a function or an event — in states and derived scripts, or across the project for a native event —, shows the call hierarchy, and counts the references of each function, property, variable and struct above it.
- 33e62c7: `papyrus build` compiles each project with the Creation Kit's `PapyrusCompiler.exe` or with Caprica, and shows their errors like the lints', on the line they name. The `[build]` section of `papyrus.toml` sets the compiler, the output folder and `optimize`, `release`, `final`. The project's sources, its `imports` and the game's own sources, found next to the official compiler, are passed as import folders; fragments are compiled too.

## 0.6.0

### Minor Changes

- a7a8921: New lints check calls, types and members across files, as the compiler does, and only report what is certain: `calls/arity` (PAP005), `calls/named-arg` (PAP006), `types/mismatch` (PAP007), `members/unknown` (PAP008, with the closest name) and `returns/missing` (PAP009). The native scripts now include their properties.
- a7a8921: The language server shows the signature of the function being called, with the current parameter highlighted, and the names of the parameters before literal arguments. The `papyrus.inlayHints.parameterNames` setting shows them before every argument, or hides them.

## 0.5.0

### Minor Changes

- e667ba6: The language server navigates across files: go to definition, hover with signature and docstring, completion, references, rename and workspace symbols. Names resolve through the `extends` chain down to the native scripts, `import`, `self` and `parent`, and members reached through declared types.
- e667ba6: `imports = [...]` in `papyrus.toml` lists folders of scripts the project builds on, like the compiler's import folders. They are read to follow `extends` and calls, never checked nor changed.
- e667ba6: `papyrus check --fix --unsafe` also fixes the case of properties, which plugins bind by name; the editor offers it as a separate quick fix.

### Patch Changes

- 98b4ae0: Each script follows the `papyrus.toml` that covers it. The CLI and the language server resolve settings, detect the game and index `extends` per project, so a Skyrim mod opened next to a Fallout 4 one is no longer checked as Fallout 4.
- 98b4ae0: Diagnostics, errors, lint descriptions and language server actions are now in English.

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
