# papyrus-tools

Parser, formateur et linter pour **Papyrus**, le langage de script du Creation
Kit de Bethesda — Skyrim, Fallout 4, Starfield.

Un binaire natif sans runtime, qui servira une CLI et un serveur LSP pour VSCode
et IntelliJ.

## État

**v0.1** — lexer et parser. Le formateur et le linter viennent ensuite.

| Crate | Rôle |
|---|---|
| `papyrus-dialect` | Cibles (`GameId`), fonctionnalités et leur jeu d'apparition |
| `papyrus-syntax` | Lexer, parser event-based, arbre syntaxique concret (`rowan`) |

Sur les 29 641 scripts vanilla des trois jeux : arbre identique à la source
octet pour octet, aucune erreur de parsing, aucune panique.

## Principes

**Installation-agnostique.** Le binaire ne cherche jamais un jeu installé : ni
registre, ni chemins Steam devinés, ni compilateur officiel au runtime. Il ne lit
que ce que le projet déclare.

**Jamais d'erreur causée par une absence d'information.** Un type non résolu est
inconnu, pas invalide. Le mode dégradé est le mode par défaut.

**Une grammaire, trois moteurs.** Le lexer reconnaît le sur-ensemble de tous les
jeux et produit `IDENT` pour tout identifiant ; la cible ne décide qu'ensuite
quels mots sont réservés. Sans quoi les milliers de scripts Skyrim qui déclarent
une variable `Var` ou `Guard` produiraient des erreurs de syntaxe.

**L'arbre ne perd rien.** `parse(src).syntax().to_string() == src`, octet
pour octet, y compris sur une entrée invalide : le parser rend toujours un
arbre complet, et range ce qu'il ne comprend pas dans des nœuds d'erreur. Le
formateur et le LSP en dépendent.

## Développement

```sh
cargo test
```

Les tests sur les sources vanilla sont optionnels et lisent leurs chemins dans
l'environnement — aucun chemin machine n'entre dans le dépôt, et aucune
installation n'est requise pour construire le projet.

```sh
PAPYRUS_CORPUS_SKYRIM=".../Skyrim Special Edition/Data/Source/Scripts" \
PAPYRUS_CORPUS_FALLOUT4=".../Fallout 4/Data/Scripts/Source" \
PAPYRUS_CORPUS_STARFIELD=".../Starfield/Data/Scripts/Source" \
  cargo test --release --features external-corpus -- --nocapture
```

Attention aux dispositions, qui n'ont aucune logique commune : Skyrim SE range
ses sources dans `Data/Source/Scripts`, Fallout 4 et Starfield dans
`Data/Scripts/Source`, avec des sous-dossiers.

## Encodage des `.psc`

Mesuré sur les trois compilateurs officiels : **aucun encodage sans BOM ne
fonctionne partout.** Skyrim lit en Windows-1252 et ne reconnaît l'UTF-8 que par
le BOM ; Fallout 4 et Starfield lisent en UTF-8 strict et remplacent tout octet
1252 par U+FFFD. Dans les deux cas la corruption est silencieuse — le
compilateur affiche « Compilation succeeded ».

UTF-8 **avec BOM** est le seul cas qui passe sur les trois. LF est accepté
partout.

## Licence

MIT OR Apache-2.0, au choix.

Les grammaires ANTLR de [`fireundubh/OpenPapyrus`](https://github.com/fireundubh/OpenPapyrus)
(MIT) ont servi de spécification de référence. Aucun code n'est dérivé de
Champollion, sous LGPLv3.
