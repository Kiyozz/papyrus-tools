---
"papyrus-tools": minor
---

New lint `names/case` (PAP020): a name written with another case than its declaration — a call, a variable, a script, a type —, which the compiler accepts. The fix writes the declared name; `papyrus check --fix` and "fix all" apply it.
