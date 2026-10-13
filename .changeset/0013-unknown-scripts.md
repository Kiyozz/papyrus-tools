---
"papyrus-tools": minor
---

New lint `scripts/unknown` (PAP018): a script, as a type or as the receiver of a call, that neither the project nor its `imports` declare, with the closest name as a quick fix. It only speaks once the game's sources are in the project or its `imports`, since the embedded tables only know the native scripts.
