---
"papyrus-tools": minor
---

`PAP021` to `PAP033`, what the compilers of Skyrim, Fallout 4 and Starfield refuse, each case checked against the official compilers: `ScriptName` other than the file's name, an `extends` cycle, members declared twice, `Return` with or without a value against the declaration, overrides whose parameters, defaults or return type differ — defaults compared as written, `False` is not `false`, with the parent's as a fix —, functions only a state declares, the shape of property accessors, array sizes, assignments to a call or `.Length`, initial values that are not literals, and the Fallout 4 and Starfield rules (default order, `Native` functions, integer range, native overrides in Starfield). `PAP007` also reports casts, comparisons and arithmetic between unrelated types, and the value of a call that returns nothing. `Import MyNs:Helper` now lets `Helper` name the script.
