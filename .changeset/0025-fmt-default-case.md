---
"papyrus-tools": patch
---

`papyrus fmt` keeps the case of `True`, `False` and `None` in a parameter's default value. The compilers of the three games compare it as text with the default of the overridden function — the parent's, or the empty state's —, so lowercasing `False` broke the compilation of a script overriding one that writes `False`, as many vanilla scripts do.
