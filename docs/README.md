# Documentation technique de forge

Ces pages expliquent **comment le code de forge est organisé et comment il
fonctionne**. Pour *utiliser* forge (écrire un `forge.json`, lancer une
application), voir le [README](../README.md) du dépôt.

| Page | Contenu |
|---|---|
| [1. Vue d'ensemble](01-vue-d-ensemble.md) | Le principe, les briques, le trajet d'un `forge.json` jusqu'à l'application qui tourne |
| [2. Schéma et formules](02-schema-et-formules.md) | `forge-schema` (lecture, validation, modèle résolu) et `forge-formula` (langage de formules) |
| [3. Génération](03-generation.md) | `forge-codegen` et `forge-cli` : fichiers produits, migrations, régénération sans perte |
| [4. Runtime du backend](04-runtime.md) | `forge-runtime` : vie d'une requête, droits, calculs, cache, fichiers, hooks |
| [5. Interfaces web et Flutter](05-interfaces.md) | `@forge/web` et `forge_flutter` : structure, écrans, personnalisation |
| [6. Contribuer](06-contribuer.md) | Recettes : ajouter un type de colonne, une règle de validation, un template ; tests |

Lecture conseillée : la page 1, puis celle de la brique sur laquelle vous
travaillez. Le fichier [`CLAUDE.md`](../CLAUDE.md) reste la référence courte des
conventions et des décisions prises ; ces pages en donnent les raisons et le
fonctionnement.
