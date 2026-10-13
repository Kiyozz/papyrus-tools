---
"papyrus-tools": minor
---

New lint `names/case` (PAP020): a name written with another case than its declaration — a call, a variable, a script, a type —, which the compiler accepts. The fix writes the declared name; `papyrus check --fix` and "fix all" apply it. When the use follows the naming convention its declaration breaks — `Initialize()` calling `Function initialize()` —, the editor offers instead to rename the declaration across the project, for the project's own functions, script variables and locals.
