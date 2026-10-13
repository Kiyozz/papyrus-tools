# papyrus-tools

Parser, formatter and linter for **Papyrus**, the scripting language of
Bethesda's Creation Kit — Skyrim, Fallout 4, Starfield.

A native binary with no runtime, serving a CLI and a language server for VSCode
and IntelliJ.

## Status

**v0.7** — `papyrus check`, `papyrus fmt`, `papyrus build`, configurable
lints, checks of calls, types, members, states and dead code across files,
quick fixes, language server with cross-file navigation: definition, hover,
completion, references, rename, signature help, parameter names, call
hierarchy, overrides. The milestones, done and next, are in
[ROADMAP.md](ROADMAP.md).

```sh
papyrus check --game skyrim Data/Source/Scripts   # syntax + lints
papyrus check --fix .                             # apply safe fixes
papyrus check --fix --unsafe .                    # and the case of properties
papyrus fmt .                                     # format in place
papyrus fmt --check --indent tab .                # CI: fail if a file needs formatting
papyrus build                                     # compile with PapyrusCompiler or Caprica
papyrus lints                                     # list the lints
papyrus lsp                                       # language server, for an editor
```

| Crate | Role |
|---|---|
| `papyrus-dialect` | Targets (`GameId`), language features and the game that introduced them |
| `papyrus-syntax` | Lexer, event-based parser, concrete syntax tree (`rowan`) |
| `papyrus-project` | `papyrus.toml`, `.psc` decoding, `.flg`, file discovery |
| `papyrus-lint` | Lint registry, in-code suppression, fixes |
| `papyrus-stdlib` | Embedded signatures of each game's native scripts |
| `papyrus-hir` | Semantic model on `salsa`: cross-file index, name resolution, types, semantic checks |
| `papyrus-fmt` | Canonical formatter |
| `papyrus-build` | Drives `PapyrusCompiler.exe` or Caprica, reads their errors |
| `papyrus-lsp` | Language server |
| `papyrus-cli` | The `papyrus` binary |

On the 29,641 vanilla scripts of the three games: tree identical to the source
byte for byte, no parse error, idempotent formatting, no false positive from
`flags/placement`, `events/unknown` or the checks of calls, types and members,
safe and complete `--fix`.

### Configuration

`papyrus.toml`, looked up from the processed path upwards. Everything in it is
optional; the command line wins.

A project is the folder of a `papyrus.toml`, or the root a script was found
under. Each script follows the `papyrus.toml` that covers it: one command line
or one editor window can span several projects, each with its own settings,
game and script index. A script only sees its own project's scripts.

Without `game` or `--game`, the target is **inferred from the scripts**. Syntax
sets a minimum game (`LockGuard` → Starfield, `Struct` or `Is` → Fallout 4), and
types, native functions and events that only one engine knows vote
(`OnUpdate` → Skyrim, `StartTimer` → Fallout 4 and later). The project takes the
majority of its decisive files. With no clue at all, Starfield syntax applies
and game-specific checks are skipped. Measured on 7,797 non-native vanilla
scripts: no verdict crosses the border between Skyrim and the other games.

```toml
game = "skyrim"
include-fragments = false
skse = true                  # Skyrim: SKSE's functions, events and scripts
# flags = "MyFlags.flg"      # replaces the game's vanilla flags
imports = ["../SkyUI/Scripts/Source"]  # read, never checked

[fmt]
indent = 2                   # or "tab"

[lints]
"naming/variables" = "allow" # allow, warn or error; by name or by code
PAP001 = "error"

[build]
compiler = "C:/…/Skyrim Special Edition/Papyrus Compiler/PapyrusCompiler.exe"  # or Caprica.exe
output = "Scripts"           # where the .pex go, the default
optimize = true
# release = true             # strip DebugOnly code, Fallout 4 onwards
# final = true               # strip BetaOnly code, Fallout 4 onwards
```

`imports` lists folders of scripts the project builds on — a library, a
framework, another mod — relative to papyrus.toml, like the compiler's import
folders. They are read to follow `extends` and calls, for the lints and the
editor, but never checked nor changed. A project's own script wins over an
imported one of the same name.

### Lints

| Code | Name | Default | What it reports |
|---|---|---|---|
| `PAP001` | `encoding/lossy` | warn | Text the target's compiler will corrupt (with `game`) |
| `PAP002` | `flags/placement` | error | Flag the game's `.flg` does not allow on this object |
| `PAP003` | `allow/unknown` | warn | Unknown name in `; papyrus: allow(...)`, which therefore suppresses nothing |
| `PAP004` | `events/unknown` | warn | Event the engine will never call: `OnActivatee`, or `OnCombatStateChanged` on a quest |
| `PAP005` | `calls/arity` | error | Call with too few or too many arguments |
| `PAP006` | `calls/named-arg` | warn | Named argument the callee does not declare, or already given |
| `PAP007` | `types/mismatch` | error | Argument, assignment, initial or return value the compiler does not cast implicitly |
| `PAP008` | `members/unknown` | error | Function, variable, property or field that does not exist — with the closest name |
| `PAP009` | `returns/missing` | warn | Typed function that can end without `Return`, and return the default value |
| `PAP010` | `states/unknown` | error | `GotoState` to a state the script does not declare — with the closest name |
| `PAP011` | `events/signature` | error | Event whose parameters differ from the engine's, or from the empty state's |
| `PAP012` | `properties/readonly` | error | Assignment to an `AutoReadOnly` or `Const` property, or one without `Set` |
| `PAP013` | `calls/not-global` | error | Non-`Global` function called on its script; `self` or a member in a `Global` function |
| `PAP014` | `code/unreachable` | warn | Statements after one that always returns |
| `PAP015` | `locals/unused` | warn | Local variable never read |
| `PAP016` | `params/unused` | warn | Parameter never read, after the last one read |
| `PAP101` | `naming/locals` | warn | Locals and parameters not in lowerCamelCase — fixable |
| `PAP102` | `naming/variables` | warn | Script variables, properties, fields not in lowerCamelCase |
| `PAP103` | `naming/functions` | warn | Functions, events, states, structs not in PascalCase |

To allow a case: `; papyrus: allow(naming/locals)` at the end of the line, or
alone on the line before. `all` covers every lint.

`--fix` only fixes case. Papyrus being case-insensitive, the rename changes
nothing for the compiler and cannot collide with anything. Properties are only
renamed with `--fix --unsafe`: plugins (`.esp`, `.esm`) bind them by name.

### Dead events

`Event OnActivatee()` compiles without a word and never fires. `PAP004`
follows the `extends` chain down to the game's native scripts, through the
project's own scripts, and reports a name close to an event the script
receives — with the correction — or an engine event no object carrying the
script will receive. The engine sends an event to every script of the object:
an `extends ObjectReference` script attached to an actor receives `OnDeath`, a
`ReferenceAlias` the events of its reference, an `ActiveMagicEffect` those of
its actor. Without certainty, the lint stays quiet: unknown parent, name
written in a string (`RegisterForModEvent` callback), name unrelated to any
known event.

Skyrim's vanilla scripts hold seven, all real: `OnCellDetatch`,
`OnCombatStateChange`, `OnContainerChanges`, `onDead`, `onLoad3d`, `Unload`,
and a quest waiting for `OnCombatStateChanged`.

### Calls, types and members

`PAP005` to `PAP009` read the project as the compiler does, across files and
`imports`, down to the game's native scripts, and only report what is
certain:

- a call is only checked once its callee is found, with its signature;
- the compiler's implicit casts are accepted: `int` to `float`, anything to
  `string` and `bool`, an object to one of its parents, `None` to any object
  or array, anything to `var`;
- an object type is only judged when its whole `extends` chain is known: an
  unknown ancestor might be the expected type;
- a member is only unknown when every script it could come from is known —
  the chain of the receiver, and for a bare name the script's chain and all
  its `import`s. The embedded Skyrim tables are vanilla: SKSE's functions,
  events and scripts come with `skse = true`, or when the project or its
  `imports` hold SKSE's sources (`SKSE.psc`). Without them,
  `Game.GetPerkPoints()` is an unknown member. Another script extender's
  (F4SE…) are known once its sources are in `imports`, as they are for the
  compiler.

Without a known target, each file is checked against the native scripts of
every game, and only what all of them reject is reported: `Debug.Trace()`
lacks an argument everywhere, `StartTimer` is only unknown to Skyrim. On the
vanilla
scripts of the three games, `PAP005` to `PAP008` find nothing; `PAP009` finds
93 typed functions that fall through to the default value.

### States, events and dead code

`GotoState("Busy")` to a state the script does not declare compiles, and the
object enters a state with none of its functions: it behaves as the empty
state. `PAP010` checks the name against the states of the script, of its
parents, and of the project's scripts deriving from it, whose instances the
object may be. Skyrim's vanilla scripts hold 17, such as `gotoState("done")`
for a state named `allDone`.

`PAP011` to `PAP013` report what the compiler rejects, while typing: an event
whose parameter types are not those of the event it implements — a remote
event, `Event Actor.OnDeath(Actor akSender, Actor akKiller)`, first receives
its sender —, a read-only property assigned, an instance reached without an
instance.

`PAP015` reports a local only written, or never used. `PAP016` only reports
the parameters after the last one read: a caller must pass the first to reach
the second. The parameters of events, of functions overriding or overridden
— by a state or a derived script of the project —, of empty functions and of
fragments are imposed, and never reported.

Fragments generated by the Creation Kit (`TIF__…`, `QF_…`, `Fragments`
folders) are skipped by default; `--include-fragments` includes them.

### Build

`papyrus build` compiles each project with the Creation Kit's
`PapyrusCompiler.exe` or with [Caprica](https://github.com/Orvid/Caprica),
told apart by their file name, and shows their errors like the lints', on the
line they name. The compiler comes from `--compiler`, `compiler` under
`[build]`, or a default Steam installation of the game.

It is given, in order: the project's source folders — for Fallout 4 and
Starfield, the folder above each namespace —, its `imports`, then the game's
own sources, found next to the Creation Kit's compiler; with Caprica, list
them in `imports`. Each source folder is compiled whole, fragments included.
The flags are `flags`, else the game's, looked up in those folders. Caprica's
language extensions are not supported: `papyrus check` would reject them.
Caprica writes no `.pex` at all once a script fails, where the Creation Kit's
compiler writes the others; and its 0.3.0 cannot read Fallout 4's vanilla
`TestKevinActivationScript.psc`, whose header starts with a space: leave it
out of the folder given in `imports`.

### Editors

`papyrus lsp` speaks LSP over standard input and output: diagnostics as you
type, fixes and "Allow on this line" — and for the semantic lints, the closest
name, a cast, the missing arguments, the unknown function created —,
formatting, document outline, folding,
semantic highlighting, and across files: go to definition, hover with the
signature and docstring, completion, references, rename, workspace symbols,
signature help with the current parameter, the names of the parameters
before the arguments of a call, the uses of the symbol under the cursor,
the overrides of a function or an event (go to implementation), the call
hierarchy, and the count of references above each function, property,
variable and struct.
On startup, it indexes each project's scripts and those of its `imports`
folders, infers the game, and rescans when a `papyrus.toml` appears or
changes.

Names resolve through declarations alone: locals and parameters, members
inherited down to the game's native scripts, global functions of `import`ed
scripts, `self` and `parent`, and members reached through declared types —
variables, properties, return types, casts, array elements, struct fields.
What cannot be known that way stays unknown, never an error. On the vanilla
scripts of the three games, every `extends` chain and 99.99 % of names
resolve, and completion answers in a few milliseconds on a 500-script
project.

Parameter names are shown before literals only, where `true` or `2` says
nothing by itself: `Disable(abFadeOut: true)`. The `inlayHints.parameterNames`
initialization option — `papyrus.inlayHints.parameterNames` in VSCode —
switches them to `all` arguments (except those already named like the
parameter) or to `none`.

A rename covers the whole project, overrides included. It is refused on
native and imported declarations, on events, which the engine calls by name,
and on script names, which are file names; on a property, it warns that
plugins must be filled again in the Creation Kit.

**VSCode** — the extension, in `editors/vscode`, is published once per platform
(Windows x64 and arm64, macOS arm64, Linux x64 and arm64), with the `papyrus`
binary embedded. To build it locally:

```sh
npm ci && npm ci --prefix editors/vscode
cargo build --release -p papyrus-cli
npm run package:extension -- --target win32-x64 --binary target/release/papyrus.exe
code --install-extension dist/papyrus-tools-win32-x64-<version>.vsix
```

The client and its dependencies are bundled by esbuild into
`editors/vscode/dist/extension.js`, the only script the `.vsix` ships:
`node_modules` stays out. The bundle must be built before packaging —
`vsce package` does it through `vscode:prepublish` — and before running the
extension from source (F5): `npm run bundle --prefix editors/vscode`.

The `papyrus.server.path` setting points to another binary; without an
embedded binary, the extension looks for `papyrus` in the `PATH`.

**IntelliJ** — with the [LSP4IJ](https://github.com/redhat-developer/lsp4ij) plugin:
*Settings › Languages & Frameworks › Language Servers › +*, command
`papyrus lsp`, and in *Mappings › File name patterns*: `*.psc`, language
`papyrus`. For highlighting before the server answers, add `editors/vscode` in
*Settings › Editor › TextMate Bundles*.

### The formatter

Lowercase keywords, two-space indent (configurable), at most one blank line, LF
line endings. It never touches identifiers, strings or comments, does not
change the encoding, and refuses a file that does not parse. Before writing, it
checks that the tokens have not changed.

## Principles

**Installation-agnostic.** The binary never looks for an installed game: no
registry, no guessed Steam paths, no official compiler at runtime. It only
reads what the project declares.

**Never an error caused by missing information.** An unresolved type is
unknown, not invalid. Degraded mode is the default mode.

**One grammar, three engines.** The lexer recognizes the superset of all games
and produces `IDENT` for every identifier; only then does the target decide
which words are reserved. Otherwise the thousands of Skyrim scripts declaring a
`Var` or `Guard` variable would produce syntax errors.

**The tree loses nothing.** `parse(src).syntax().to_string() == src`, byte for
byte, even on invalid input: the parser always returns a complete tree, and
puts what it does not understand in error nodes. The formatter and the
language server depend on it.

## Development

```sh
cargo test
```

Tests on the vanilla sources are optional and read their paths from the
environment — no machine path enters the repository, and no installation is
required to build the project.

```sh
PAPYRUS_CORPUS_SKYRIM=".../Skyrim Special Edition/Data/Source/Scripts" \
PAPYRUS_CORPUS_FALLOUT4=".../Fallout 4/Data/Scripts/Source" \
PAPYRUS_CORPUS_STARFIELD=".../Starfield/Data/Scripts/Source" \
  cargo test --release --workspace \
    --features papyrus-syntax/external-corpus,papyrus-fmt/external-corpus,papyrus-lint/external-corpus,papyrus-project/external-corpus,papyrus-hir/external-corpus \
    -- --nocapture
```

The `papyrus-stdlib` tables are regenerated from an installed game's sources.
They only hold signatures, never bodies or docstrings.

```sh
cargo xtask stdlib --game skyrim --from ".../Data/Source/Scripts"
```

Mind the layouts, which share no logic: Skyrim SE keeps its sources in
`Data/Source/Scripts`, Fallout 4 and Starfield in `Data/Scripts/Source`, with
subfolders.

### Versions

One version covers the CLI, the language server and the extension. Every
visible change adds a changeset (`npx changeset`, or a file in `.changeset/`).
On `main`, the *Release* workflow opens a "chore: release" PR that bumps the
version, writes `CHANGELOG.md` and copies it into `Cargo.toml` and the
extension. Once merged, the version is tagged, then binaries and extensions are
built for each platform and attached to the GitHub release — and published on
the Visual Studio Marketplace and Open VSX if the `VSCE_PAT` and `OVSX_PAT`
secrets exist.

## `.psc` encoding

Measured on the three official compilers: **no encoding without a BOM works
everywhere.** Skyrim reads Windows-1252 and only recognizes UTF-8 by its BOM;
Fallout 4 and Starfield read strict UTF-8 and replace every 1252 byte with
U+FFFD. In both cases the corruption is silent — the compiler prints
"Compilation succeeded".

UTF-8 **with BOM** is the only case that works on all three. LF is accepted
everywhere.

`papyrus check --game …` reports it (`PAP001`), in strings and docstrings only
— a comment never reaches the `.pex`. Without `--game`, the check is skipped:
the remedy depends on the game. On the vanilla corpus, a single case:
`DLC2PillarShoutTriggerScript.psc`, whose docstring holds an em dash in UTF-8
without BOM, which the Creation Kit shows as "â€“".

## License

[Apache-2.0](LICENSE).

The ANTLR grammars of [`fireundubh/OpenPapyrus`](https://github.com/fireundubh/OpenPapyrus)
(MIT) served as reference specification. No code is derived from Champollion,
under LGPLv3.
