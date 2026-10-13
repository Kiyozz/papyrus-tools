---
"papyrus-tools": minor
---

New lints check calls, types and members across files, as the compiler does, and only report what is certain: `calls/arity` (PAP005), `calls/named-arg` (PAP006), `types/mismatch` (PAP007), `members/unknown` (PAP008, with the closest name) and `returns/missing` (PAP009). The native scripts now include their properties.
