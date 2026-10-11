# Papyrus

Diagnostics, formatage et lints pour **Papyrus**, le langage de script du
Creation Kit — Skyrim, Fallout 4, Starfield.

- erreurs de syntaxe à la frappe, selon le jeu visé ;
- lints : événement mal orthographié que le moteur n'appellera jamais
  (`OnActivatee`), flags mal placés, texte que le compilateur corrompra,
  conventions de nommage ;
- correctifs en un clic, et « Autoriser » pour faire taire un lint sur une
  ligne ;
- formatage du document ;
- plan du fichier, repli des blocs, coloration sémantique.

## Installation

L'extension lance `papyrus lsp`. Le binaire `papyrus` doit être dans le
`PATH`, ou désigné par le réglage `papyrus.server.path`.

## Configuration

Un `papyrus.toml` à la racine du projet fixe le jeu, l'indentation et le
niveau de chaque lint. Sans lui, le jeu est déduit des scripts.

```toml
game = "skyrim"

[fmt]
indent = 2

[lints]
"naming/variables" = "allow"
```
