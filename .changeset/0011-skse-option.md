---
"papyrus-tools": minor
---

Skyrim's embedded native scripts are now vanilla only. SKSE's functions, events and scripts come with `skse = true` in `papyrus.toml`, or on their own when the project or its `imports` hold SKSE's sources (`SKSE.psc`). Without them, calling an SKSE function such as `Game.GetPerkPoints()` is the new `members/skse` (PAP017), a warning inviting to install SKSE and enable it.
