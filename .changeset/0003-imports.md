---
"papyrus-tools": minor
---

`imports = [...]` in `papyrus.toml` lists folders of scripts the project builds on, like the compiler's import folders. They are read to follow `extends` and calls, never checked nor changed.
