---
"papyrus-tools": minor
---

The TextMate grammar is tested in CI with `vscode-tmgrammar-test`, on annotated `.psc` files that give the scope expected on each token. Fixed on the way: a variable declared with a primitive type (`Int count`, `Float x = 0.0`, a parameter `Int aiCount`) now has its name highlighted; `EndProperty`, `EndState`, `EndStruct` and `EndGroup` carry the scope of their opening keyword; quotes and comment delimiters carry their punctuation scope.
