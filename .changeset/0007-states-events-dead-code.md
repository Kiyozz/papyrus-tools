---
"papyrus-tools": minor
---

New lints: `states/unknown` (PAP010) for `GotoState` to an undeclared state, `events/signature` (PAP011) for an event whose parameters differ from the engine's, `properties/readonly` (PAP012) for an assignment to an `AutoReadOnly`, `Const` or `Set`-less property, `calls/not-global` (PAP013) for an instance member reached from a `Global` function or on the script itself, and as warnings `code/unreachable` (PAP014), `locals/unused` (PAP015) and `params/unused` (PAP016). Unused parameters are only reported after the last one read, never on events, overrides or fragments.
