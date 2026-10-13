---
"papyrus-tools": minor
---

The language server keeps up with projects importing a whole game. Each file remembers which names it mentions, so references, reference lenses, rename and the call hierarchy only read the files that may use a name: two to three times faster on Skyrim's 14,000 vanilla scripts, within 80 ms for the most used names. These requests and completion now run on a copy of the model, off the server's lock: an edit cancels them, and the editor asks again (`ContentModified`). Indexing the workspace reports its progress to editors that show it.
