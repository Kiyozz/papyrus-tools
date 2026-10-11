# papyrus-tools

Parser, formateur et linter pour **Papyrus**, le langage de script du Creation
Kit de Bethesda — Skyrim, Fallout 4, Starfield.

Un binaire natif sans runtime, qui servira une CLI et un serveur LSP pour VSCode
et IntelliJ.

## État

**v0.3** — `papyrus check`, `papyrus fmt`, lints configurables.

```sh
papyrus check --game skyrim Data/Source/Scripts   # syntaxe + lints
papyrus check --fix .                             # applique les correctifs sûrs
papyrus fmt .                                     # formate sur place
papyrus fmt --check --indent tab .                # CI : échoue si à reformater
papyrus lints                                     # liste les lints
```

| Crate | Rôle |
|---|---|
| `papyrus-dialect` | Cibles (`GameId`), fonctionnalités et leur jeu d'apparition |
| `papyrus-syntax` | Lexer, parser event-based, arbre syntaxique concret (`rowan`) |
| `papyrus-project` | `papyrus.toml`, décodage des `.psc`, `.flg`, découverte des fichiers |
| `papyrus-lint` | Registre de lints, suppression dans le code, correctifs |
| `papyrus-stdlib` | Signatures des scripts natifs de chaque jeu, embarquées |
| `papyrus-fmt` | Formateur canonique |
| `papyrus-cli` | Le binaire `papyrus` |

Sur les 29 641 scripts vanilla des trois jeux : arbre identique à la source
octet pour octet, aucune erreur de parsing, formatage idempotent, aucun faux
positif de `flags/placement`, `--fix` sûr et complet.

### Configuration

`papyrus.toml`, cherché en remontant depuis le chemin traité. Tout y est
facultatif ; la ligne de commande l'emporte.

Sans `game` ni `--game`, la cible est **déduite des scripts**. La syntaxe fixe
un jeu minimal (`LockGuard` → Starfield, `Struct` ou `Is` → Fallout 4), et les
types, fonctions natives et événements qu'un seul moteur connaît votent
(`OnUpdate` → Skyrim, `StartTimer` → Fallout 4 et après). Le projet prend la
majorité de ses fichiers décisifs. Sans aucun indice, la syntaxe de Starfield
s'applique et les vérifications propres au jeu sont sautées. Mesuré sur 7 797
scripts vanilla non natifs : aucun verdict ne franchit la frontière entre
Skyrim et les autres jeux.

```toml
game = "skyrim"
include-fragments = false
# flags = "MesFlags.flg"     # remplace les flags vanilla du jeu

[fmt]
indent = 2                   # ou "tab"

[lints]
"naming/variables" = "allow" # allow, warn ou error ; par nom ou par code
PAP001 = "error"
```

### Lints

| Code | Nom | Défaut | Ce qu'il signale |
|---|---|---|---|
| `PAP001` | `encoding/lossy` | warn | Texte que le compilateur de la cible corrompra (avec `game`) |
| `PAP002` | `flags/placement` | error | Flag que le `.flg` du jeu n'autorise pas sur cet objet |
| `PAP003` | `allow/unknown` | warn | Nom inconnu dans `; papyrus: allow(...)`, qui ne supprime donc rien |
| `PAP101` | `naming/locals` | warn | Locales et paramètres hors lowerCamelCase — corrigeable |
| `PAP102` | `naming/variables` | warn | Variables de script, properties, champs hors lowerCamelCase |
| `PAP103` | `naming/functions` | warn | Fonctions, événements, states, structures hors PascalCase |

Pour autoriser un cas : `; papyrus: allow(naming/locals)` en fin de ligne, ou
seul sur la ligne qui précède. `all` couvre tous les lints.

`--fix` ne corrige que la casse. Papyrus y étant insensible, le renommage ne
change rien pour le compilateur et ne peut entrer en collision avec rien. Les
properties ne sont jamais renommées : les `.esp` les lient par leur nom.

Les fragments générés par le Creation Kit (`TIF__…`, `QF_…`, dossiers
`Fragments`) sont ignorés par défaut ; `--include-fragments` les inclut.

### Le formateur

Mots-clés en minuscules, indentation de deux espaces (réglable), au plus une
ligne vide, fins de ligne LF. Il ne touche jamais aux identifiants, aux chaînes
ni aux commentaires, ne change pas l'encodage, et refuse un fichier qui ne se
parse pas. Avant d'écrire, il vérifie que les tokens n'ont pas changé.

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
  cargo test --release --workspace \
    --features papyrus-syntax/external-corpus,papyrus-fmt/external-corpus,papyrus-lint/external-corpus,papyrus-project/external-corpus \
    -- --nocapture
```

Les tables de `papyrus-stdlib` se régénèrent depuis les sources d'un jeu
installé. Elles ne contiennent que des signatures, jamais de corps ni de
docstrings.

```sh
cargo xtask stdlib --game skyrim --from ".../Data/Source/Scripts"
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

`papyrus check --game …` le signale (`PAP001`), dans les chaînes et les
docstrings seulement — un commentaire n'atteint jamais le `.pex`. Sans
`--game`, la vérification est sautée : le remède dépend du jeu. Sur le corpus
vanilla, un seul cas : `DLC2PillarShoutTriggerScript.psc`, dont une docstring
contient un tiret cadratin en UTF-8 sans BOM, que le Creation Kit affiche
« â€“ ».

## Licence

MIT OR Apache-2.0, au choix.

Les grammaires ANTLR de [`fireundubh/OpenPapyrus`](https://github.com/fireundubh/OpenPapyrus)
(MIT) ont servi de spécification de référence. Aucun code n'est dérivé de
Champollion, sous LGPLv3.
