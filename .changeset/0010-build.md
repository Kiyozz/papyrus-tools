---
"papyrus-tools": minor
---

`papyrus build` compiles each project with the Creation Kit's `PapyrusCompiler.exe` or with Caprica, and shows their errors like the lints', on the line they name. The `[build]` section of `papyrus.toml` sets the compiler, the output folder and `optimize`, `release`, `final`. The project's sources, its `imports` and the game's own sources, found next to the official compiler, are passed as import folders; fragments are compiled too.
