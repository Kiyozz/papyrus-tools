# papyrus-tools

Parser, formatter and linter for **Papyrus**, the scripting language of
Bethesda's Creation Kit — Skyrim, Fallout 4, Starfield.

A native binary with no runtime, serving a CLI and a language server for VSCode
and IntelliJ.

## Status

**v0.16** — `papyrus check`, `papyrus fmt`, `papyrus build`, `papyrus
disassemble`, configurable lints, what the official compilers refuse, checks
of calls, types, members, states and dead code across files, SKSE as an
option, quick fixes, `.ppj` projects, a baseline for older mods, JSON and
SARIF reports; a language server with cross-file navigation: definition,
hover, completion, references, rename, signature help, parameter names, call
and type hierarchies, overrides — fast on projects importing a whole game. The
VSCode extension is on the
[Marketplace](https://marketplace.visualstudio.com/items?itemName=kiyozz.papyrus-toolchain)
and [Open VSX](https://open-vsx.org/extension/kiyozz/papyrus-toolchain); the
binaries of each platform are in the
[releases](https://github.com/Kiyozz/papyrus-tools/releases). The milestones,
done and next, are in [ROADMAP.md](ROADMAP.md).

```sh
papyrus check --game skyrim Data/Source/Scripts   # syntax + lints
papyrus check --fix .                             # apply safe fixes
papyrus check --fix --unsafe .                    # and the case of properties
papyrus fmt .                                     # format in place
papyrus fmt --check --indent tab .                # CI: fail if a file needs formatting
papyrus build                                     # compile with PapyrusCompiler or Caprica
papyrus build Source/MyScript.psc                 # that script alone
papyrus disassemble Build/MyScript.pex            # a compiled script, as assembly
papyrus init                                      # write a papyrus.toml, on top of a .ppj if there is one
papyrus lints                                     # list the lints
papyrus explain PAP015                            # what a lint means, and what to do
papyrus check --write-baseline                    # adopt on an old mod: only new problems from now on
papyrus check --message-format sarif > out.sarif  # or json: a report on standard output
papyrus fmt --stdin --stdin-filename My.psc       # for editors and hooks; check has --stdin too
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
# sources = ["Source"]       # the folders above the namespaces

[fmt]
indent = 2                   # or "tab"
keywords = "lower"           # or "pascal": EndFunction

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

Its JSON schema, [`schemas/papyrus.schema.json`](schemas/papyrus.schema.json),
generated from the lint registry by `cargo xtask schema`, gives TOML editors
completion and validation; a `papyrus.toml` created by the editor points to it
with `#:schema`.

`imports` lists folders of scripts the project builds on — a library, a
framework, another mod — relative to papyrus.toml, like the compiler's import
folders. They are read to follow `extends` and calls, for the lints and the
editor, but never checked nor changed. A project's own script wins over an
imported one of the same name.

`sources` names the folders above the namespaces of the project's scripts, for
Fallout 4 and Starfield; a `.ppj`'s `Imports` and `Folders` say the same. It
changes neither what is checked nor what is compiled. Under such a folder, the
name of a script must follow its folders: `PAP021` reports `ScriptName Stray`
in `Source/MyMod/Stray.psc`, and a script out of place no longer moves the root
that `papyrus build` gives the others. Without it, the root is what each
script's name says, and a name without namespace goes unnoticed in a subfolder.

A `.ppj`, the Creation Kit's or [Pyro](https://github.com/fireundubh/pyro)'s,
needs no configuration: in a folder without `papyrus.toml`, it is one. `check`,
`fmt`, `build` and the editor read its `Game`, `Imports`, `Folders`, `Scripts`,
`Flags`, `Output`, `Optimize`, `Release`, `Final` and `Variables`. Its project
is what it lists, wherever that is, rather than the scripts of the folder. An
import that is not on this machine or is remote is skipped with a note. To set
what a `.ppj` cannot say, `papyrus init` writes a `papyrus.toml` naming it with
`ppj = "mod.ppj"`: the `.ppj` stays the source, and what `papyrus.toml` sets
wins, `imports` adding up. Several `.ppj` in a folder need that key, or
`papyrus init --ppj`. Pyro's packages, archives and build events are left to
Pyro: nothing of a `.ppj` is ever run.

### Lints

| Code | Name | Default | What it reports |
|---|---|---|---|
| `PAP001` | `encoding/lossy` | warn | Text the target's compiler will corrupt (with `game`) |
| `PAP002` | `flags/placement` | error | Flag the game's `.flg` does not allow on this object |
| `PAP003` | `allow/unknown` | warn | Unknown name in `; papyrus: allow(...)`, which therefore suppresses nothing |
| `PAP004` | `events/unknown` | warn | Event the engine will never call: `OnActivatee`, or `OnCombatStateChanged` on a quest |
| `PAP005` | `calls/arity` | error | Call with too few or too many arguments |
| `PAP006` | `calls/named-arg` | warn | Named argument the callee does not declare, or already given |
| `PAP007` | `types/mismatch` | error | Argument, assignment, initial or return value the compiler does not cast implicitly; cast, comparison or arithmetic between unrelated types; the value of a call that returns nothing |
| `PAP008` | `members/unknown` | error | Function, variable, property or field that does not exist — with the closest name |
| `PAP009` | `returns/missing` | warn | Typed function that can end without `Return`, and return the default value |
| `PAP010` | `states/unknown` | error | `GotoState` to a state the script does not declare — with the closest name |
| `PAP011` | `events/signature` | error | Event whose parameters differ from the engine's, or from the empty state's |
| `PAP012` | `properties/readonly` | error | Assignment to an `AutoReadOnly` or `Const` property, or one without `Set` |
| `PAP013` | `calls/not-global` | error | Non-`Global` function called on its script; `self` or a member in a `Global` function |
| `PAP014` | `code/unreachable` | warn | Statements after one that always returns |
| `PAP015` | `locals/unused` | warn | Local variable never read |
| `PAP016` | `params/unused` | warn | Parameter never read, after the last one read |
| `PAP017` | `members/skse` | warn | SKSE member or script in a Skyrim project without `skse = true` nor SKSE's sources |
| `PAP018` | `scripts/unknown` | error | Script that does not exist, once the game's sources are in the project or its `imports` |
| `PAP019` | `locals/duplicate` | error | Local declared twice in a block, named like a parameter or a script variable, or hiding one of an enclosing block |
| `PAP020` | `names/case` | warn | Name written with another case than its declaration, which the compiler ignores (fixable) |
| `PAP021` | `scripts/name` | error | `ScriptName` other than the file's name, or a namespace not matching its folders |
| `PAP022` | `scripts/cycle` | error | Script that extends itself, directly or through its ancestors |
| `PAP023` | `members/duplicate` | error | Function or event twice in a state, variable or property twice, a second `Auto` state; from Fallout 4, a property named like a variable |
| `PAP024` | `returns/value` | error | `Return value` in a function without return type or an event, `Return` alone in a typed function |
| `PAP025` | `overrides/signature` | error | Override whose parameters, defaults or return type differ; a `Global` function redefined; from Fallout 4, a function named like an event; in Starfield, a native function overridden |
| `PAP026` | `states/function` | error | Function in a state that neither the empty state nor an ancestor declares, `Global` function in a state |
| `PAP027` | `properties/accessors` | error | `Get` or `Set` of the wrong shape, another function in a property, initial value on a full property, `AutoReadOnly` without one |
| `PAP028` | `arrays/size` | error | `new T[n]` beyond 128; in Skyrim, a size that is not a literal from 1 to 128 |
| `PAP029` | `code/not-assignable` | error | Assignment to a call or to `.Length` |
| `PAP030` | `values/not-literal` | error | Initial value of a script variable or a property, or default of a parameter, that is not a literal |
| `PAP031` | `params/default-order` | error | From Fallout 4, a parameter without default after one with a default |
| `PAP032` | `functions/native` | error | From Fallout 4, a `Native` function in a script that is not `Native` |
| `PAP033` | `literals/int-range` | error | Integer literal beyond 32 bits; from Fallout 4, a decimal one above 2147483647 |
| `PAP034` | `updates/repeating` | warn | `RegisterForUpdate` or `RegisterForUpdateGameTime`, which keep firing in saves after the mod is removed — with the `Single` version |
| `PAP035` | `calls/get-player` | warn | `Game.GetPlayer()` called on each iteration of a loop |
| `PAP036` | `bools/comparison` | warn | Boolean compared with `True` or `False` — with the simpler form |
| `PAP037` | `code/self-assign` | warn | Variable, property or element assigned to itself |
| `PAP038` | `code/self-compare` | warn | Value compared with itself |
| `PAP039` | `variables/unused` | warn | Script variable never read, `Conditional` ones excepted |
| `PAP040` | `imports/unused` | warn | `Import` of a script none of whose global functions or types is used unqualified |
| `PAP041` | `calls/infinite-recursion` | warn | Function that comes back to itself on every path, directly or through others |
| `PAP101` | `naming/locals` | warn | Locals and parameters not in lowerCamelCase — fixable |
| `PAP102` | `naming/variables` | warn | Script variables, properties, fields not in lowerCamelCase |
| `PAP103` | `naming/functions` | warn | Functions, events, states, structs not in PascalCase |

To allow a case: `; papyrus: allow(naming/locals)` at the end of the line, or
alone on the line before. `all` covers every lint.

`--fix` fixes case: Papyrus being case-insensitive, the rename changes nothing
for the compiler and cannot collide with anything. It also removes what does
nothing: an unused `Import`, an assignment to itself, a variable no code names,
a local whose value is a literal, a comparison with `True` or `False`.

`--fix --unsafe` adds what may break outside the scripts: properties, which
plugins (`.esp`, `.esm`) bind by name, and the names of `naming/*` across the
project, unless the new one is taken in the file. The editor offers the same
fixes as quick fixes.

`papyrus explain <code>` says why a lint matters, with an example and what to
do about it.

### Baseline

To adopt `papyrus` on a mod that already has many problems, `papyrus check
--write-baseline` records them in `papyrus-baseline.json`, next to
`papyrus.toml`: commit it. `papyrus check` then only reports new problems, and
says how many it hides and when some are gone.

As in PHPStan, a problem is known by its file, its lint and its message, never
by its line, and each is counted: editing around it keeps it known, one more in
the file is reported. Syntax errors are never recorded. `--no-baseline` shows everything,
`--write-baseline` again records what remains, and on a single file leaves the
others alone. The editor ignores the baseline. `papyrus explain baseline` says
all this in the terminal.

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
  `Game.GetPerkPoints()`, `StringUtil.Find()` or a `ColorForm` property are
  `PAP017 members/skse`, a warning inviting to install SKSE: the compiler accepts it once SKSE's sources sit in the
  game's, which papyrus cannot tell. A script itself is only unknown
  (`PAP018`) once the project or its `imports` hold the game's sources: the
  embedded tables only know the native scripts. Another script extender's
  (F4SE…) are known once its sources are in `imports`, as they are for the
  compiler.
- a function named as a value, `MyScript.Run` or `x = Run`, is said to be
  one, with the parentheses to call it as a fix.

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

`PAP019` reports the locals the Creation Kit's compiler refuses: declared
twice in a block, named like a parameter or a variable of the script, or
hiding one of an enclosing block. Sibling blocks — `If` and `Else` — may reuse
a name. Redeclared with a value, it suggests assigning instead.

`PAP020` reports a name written with another case than its declaration —
`debug.trace` for `Debug.Trace`, `objectReference`, a local `COUNT` declared
`count` —, which the compiler accepts. Its fix writes the declared name, and
`--fix` applies it. When the use follows the naming convention the
declaration breaks — `Initialize()` calling `Function initialize()` —, the
editor offers instead to rename the declaration, across the project; only
for the project's own functions, script variables and locals, never an
imported or native one, a property or an event. The vanilla scripts mix cases freely: 40,000 such names
in Skyrim's.

### What the compiler refuses

`PAP021` to `PAP033`, and the cast, comparison and arithmetic checks of
`PAP007`, report what the Creation Kit's compiler stops on, each case checked
against the official compilers of Skyrim, Fallout 4 and Starfield — they
differ, and each rule follows the target. Some are surprising:

- an override's default values are compared **as written** for `True`,
  `False`, `None` and strings: `Bool a = false` overriding `Bool a = False`
  does not compile, while `1.0` and `1.00`, `16` and `0x10` match. The fix
  writes the parent's;
- Starfield refuses any override of a native function in a script that is not
  `Native`, even with the same signature;
- a call that returns nothing passes for `None`: Skyrim accepts it where an
  object or a `bool` is expected, never a number; from Fallout 4, not as a
  condition either.

Without a target, only what every game refuses is reported. None of these
rules reports anything on the vanilla scripts of the three games.

`PAP034` to `PAP040` are habits rather than errors, and the vanilla scripts
are full of them: Skyrim's call `RegisterForUpdate` 169 times and
`Game.GetPlayer()` in a loop 114 times, compare a `bool` with `True` 1,420
times, keep 380 script variables never read and 670 useless `Import`s. A
`Conditional` variable is read by the game's conditions, never reported; an
`Import MyNs:Helper` is used by naming `Helper`.

Fragments generated by the Creation Kit (`TIF__…`, `QF_…`, `Fragments`
folders) are skipped by default; `--include-fragments` includes them.

### Build

`papyrus build` compiles each project with the Creation Kit's
`PapyrusCompiler.exe` or with [Caprica](https://github.com/Orvid/Caprica),
told apart by their file name, and shows their errors like the lints', on the
line they name. The compiler comes from `--compiler`, `compiler` under
`[build]`, or a default Steam installation of the game. Given `.psc` files,
`papyrus build Source/MyScript.psc` compiles those alone; the folders of the
project stay available for what they use.

It is given, in order: the project's source folders — for Fallout 4 and
Starfield, the folder above each namespace —, its `imports`, then the game's
own sources, found next to the Creation Kit's compiler; with Caprica, list
them in `imports`. Each source folder is compiled whole, fragments included,
unless a `.ppj` lists only part of it: its scripts then go one by one.
The flags are `flags`, else the game's, looked up in those folders. Caprica's
language extensions are not supported: `papyrus check` would reject them.
Caprica writes no `.pex` at all once a script fails, where the Creation Kit's
compiler writes the others; and its 0.3.0 cannot read Fallout 4's vanilla
`TestKevinActivationScript.psc`, whose header starts with a space: leave it
out of the folder given in `imports`.

`--message-format short`, on `build` and `check`, prints each problem on one
line, `path:line:column:end line:end column: error[code]: message`, for
editors' problem matchers.

### Compiled scripts

`papyrus disassemble` prints a `.pex` of Skyrim, Fallout 4, Fallout 76 or
Starfield, whichever compiler wrote it, in the format of `PapyrusAssembler -D`,
without needing the game's tools. Two differences: tables follow the order of
the file, where `PapyrusAssembler` follows its own, and the dates in comments
are in UTC. In VSCode, opening a `.pex` shows this text, read-only, and it
follows the file when it is compiled again.

### Editors

`papyrus lsp` speaks LSP over standard input and output: diagnostics as you
type, fixes and "Allow on this line" — and for the semantic lints, the closest
name, a cast, the missing arguments, the unknown function created —, a lint's
level or SKSE set in `papyrus.toml`, created if missing,
formatting, document outline, folding,
semantic highlighting, and across files: go to definition, hover with the
signature and docstring, completion — named arguments included —, references, rename, workspace symbols,
signature help with the current parameter, the names of the parameters
before the arguments of a call, the uses of the symbol under the cursor,
the overrides of a function or an event (go to implementation), the call
hierarchy, and the count of references above each function, property,
variable and struct.
On startup, it indexes each project's scripts and those of its `imports`
folders, showing its progress, infers the game, and rescans when a
`papyrus.toml` appears or changes.

A project may import a whole game: with Skyrim's 14,000 vanilla scripts in
`imports`, fragments included, hover and checks answer in under a
millisecond, completion in 35 ms, references in 3 to 80 ms. Each file
remembers which names it mentions, so a search only reads the files that may
use the name; and the requests that read the whole project run apart, an edit
cancelling them.

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
native and imported declarations, and on events, which the engine calls by
name; on a property, it warns that plugins must be filled again in the Creation
Kit. Renaming a script moves its file, and moving or renaming a `.psc` in the
editor renames its script and every use, with the same warning. From Fallout 4,
namespaces are the folders: `MyMod:Base` lives in `MyMod/Base.psc`, and moving it
to `Other/` makes it `Other:Base`. A script whose file is not where its name says
is left alone.

A recursive function shows `↻ recursive` above it and after each call that comes
back, directly or through other functions, across files. An override counts as
the function it overrides, but `Parent.F()` does not recurse.

The type hierarchy follows `extends` through the scripts with a source: add the
game's sources to `imports` to see the native ones. Formatting a selection only
changes the selected lines.

**VSCode** — the extension, in `editors/vscode`, is on the
[Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=kiyozz.papyrus-toolchain)
and [Open VSX](https://open-vsx.org/extension/kiyozz/papyrus-toolchain). It is
published once per platform (Windows, macOS and Linux, x64 and arm64), with
the `papyrus` binary embedded, and on Open VSX once without it for the others,
`papyrus` then taken from the `PATH`. To build it locally:

```sh
npm ci && npm ci --prefix editors/vscode
cargo build --release -p papyrus-cli
npm run package:extension -- --target win32-x64 --binary target/release/papyrus.exe
code --install-extension dist/papyrus-toolchain-win32-x64-<version>.vsix
```

The client and its dependencies are bundled by esbuild into
`editors/vscode/dist/extension.js`, the only script the `.vsix` ships:
`node_modules` stays out. The bundle must be built before packaging —
`vsce package` does it through `vscode:prepublish` — and before running the
extension from source (F5): `npm run bundle --prefix editors/vscode`.

The `papyrus.server.path` setting points to another binary; without an
embedded binary, the extension looks for `papyrus` in the `PATH`. It adds a
TextMate grammar and snippets, which work without the server, the
**Papyrus: Create papyrus.toml** command, `papyrus: build` and
`papyrus: check` tasks with their problem matcher, and the schema of
`papyrus.toml` for [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml).
The settings that belong to a project live in `papyrus.toml`, never in
VSCode's: the command line and every editor read the same.

**IntelliJ** — with the [LSP4IJ](https://github.com/redhat-developer/lsp4ij) plugin:
*Settings › Languages & Frameworks › Language Servers › +*, command
`papyrus lsp`, and in *Mappings › File name patterns*: `*.psc`, language
`papyrus`. For highlighting before the server answers, add `editors/vscode` in
*Settings › Editor › TextMate Bundles*.

### The formatter

Lowercase keywords — or PascalCase, `EndFunction`, with `keywords = "pascal"`
—, two-space indent (configurable), at most one blank line, one around each
function, event, state, struct, group and full property, no parentheses around
a whole `If` or `While` condition, LF line endings. It never touches
identifiers, strings or comments, nor `True`,
`False` and `None` as a parameter's default — the compiler compares it as text
with the overridden function's, so `False` against `false` fails. It does not
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
PAPYRUS_COMPILER_SKYRIM=".../PapyrusCompiler.exe" \
PAPYRUS_COMPILER_FALLOUT4=".../PapyrusCompiler.exe" \
PAPYRUS_COMPILER_STARFIELD=".../PapyrusCompiler.exe" \
  cargo test --release --workspace \
    --features papyrus-syntax/external-corpus,papyrus-fmt/external-corpus,papyrus-lint/external-corpus,papyrus-project/external-corpus,papyrus-hir/external-corpus \
    -- --nocapture
```

The TextMate grammar is tested on annotated `.psc` files,
`editors/vscode/tests/grammar`: under each line, `^` marks a token and the
scope it must carry.

```sh
npm run test:grammar --prefix editors/vscode
```

The extension is tested in a real VSCode, downloaded into
`editors/vscode/.vscode-test`, on `editors/vscode/tests/fixture`. The server
is `target/debug/papyrus`, or the one in `PAPYRUS_BIN_DIR`; on Linux without a
display, under `xvfb-run -a`.

```sh
cargo build -p papyrus-cli
npm run test:integration --prefix editors/vscode
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
the Visual Studio Marketplace and Open VSX by trusted publishing (OIDC), Open
VSX also getting a universal extension, without the binary, for the other
platforms — the Marketplace keeps a universal package alone. Both
stores trust `release.yml`; Open VSX only once the extension has a version,
the first one published with the `OVSX_PAT` secret. The release PR is opened by a GitHub App (`RELEASE_APP_ID`,
`RELEASE_APP_KEY`): one opened with `GITHUB_TOKEN` would start no CI.

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
