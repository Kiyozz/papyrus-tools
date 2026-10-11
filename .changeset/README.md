# Changesets

Chaque changement visible ajoute un fichier ici, `0000-mon-changement.md` :

```md
---
"papyrus-tools": minor
---

Description of the change in English.
```

`patch`, `minor` ou `major`. Une seule version couvre la CLI, le serveur LSP
et l'extension VSCode : `npm run release:version` la monte, écrit
`CHANGELOG.md` et la recopie dans `Cargo.toml` et l'extension.
