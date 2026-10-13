---
"papyrus-tools": minor
---

New lint `locals/duplicate` (PAP019): a local declared twice in a block, named like a parameter or a variable of the script, or hiding one of an enclosing block — what the Creation Kit's compiler refuses. Redeclared with a value, the quick fix assigns instead. `papyrus build` also says when the flags file is in no source folder: with Caprica, the game's sources belong in `imports`.
