---
"papyrus-tools": minor
---

`members/unknown` (PAP008): a function named as a value — `MyScript.Run`, `self.Run`, `x = Run` — is reported as a function to call, with a fix adding the parentheses, instead of a missing property.
