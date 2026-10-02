# forge

Générateur d'applications écrit en Rust. À partir d'un fichier JSON décrivant des
tables, forge génère :

- un backend Rust (API REST + GraphQL, auth, règles d'autorisation, CSV, cache, observabilité) ;
- une application Flutter unique (web + mobile) : listes, détails, formulaires, calendrier, statistiques ;
- les migrations, les tests, la documentation et la configuration Docker / CI.

Le code généré est fin : l'essentiel de la logique vit dans des bibliothèques
communes (`forge-runtime`, `forge_flutter`), et le code utilisateur (`custom/`)
n'est jamais écrasé par une régénération.

## État d'avancement

| Phase | Contenu | État |
|---|---|---|
| 0 | Workspace, `forge-schema`, parser de formules, JSON Schema, `forge validate` | ✅ |
| 1 | Backend minimal : entités, migrations, CRUD REST, `parameters` | à venir |
| 2 | Migrations incrémentales | à venir |
| 3 | Auth, rôles, moteur de règles | à venir |
| 4 | Évaluation des formules, lookups, `persist`, agrégats | à venir |
| 5 | GraphQL, import/export CSV, OpenAPI | à venir |
| 6 | Observabilité, cache | à venir |
| 7 | Application Flutter | à venir |
| 8 | Docker, docker-compose, GitHub Actions de l'app générée | à venir |

Seules les commandes listées ci-dessous existent aujourd'hui.

## Installation

```bash
cargo install --path crates/forge-cli
forge --help
```

## Commandes

| Commande | Rôle |
|---|---|
| `forge validate [schema.json]` | Valide un schéma (défaut : `forge.json`) et liste **toutes** les erreurs avec leur chemin. Code de sortie 1 en cas d'erreur. |
| `forge schema` | Affiche le JSON Schema du format d'entrée (contenu de `forge.schema.json`). |

Exemple de sortie :

```text
$ forge validate schema.json
schema.json : 3 erreur(s)
  - tables[2].columns[6].formula: `probabilit` n'est ni une colonne ni une relation de la table `opportunite` (position 10)
  - tables[2].rules[2].roles[0]: rôle `vendeur` non déclaré dans `roles`
  - tables[2].columns[13].formula: dépendance circulaire : opportunite.boucle → opportunite.boucle
```

## Format d'entrée

Référence complète et autocomplétion : [`forge.schema.json`](forge.schema.json).
Ajoutez en tête de votre fichier :

```json
{ "$schema": "chemin/vers/forge.schema.json", ... }
```

Exemple complet : [`examples/crm/schema.json`](examples/crm/schema.json).

### Racine

| Clé | Description |
|---|---|
| `app` | `name` (identifiant), `default_locale`, `locales` (ex. `["fr", "en"]`). |
| `roles` | Rôles utilisateurs. `admin` est obligatoire. |
| `parameters` | Paramètres globaux typés (`name`, `type`, `default`, `label`), lisibles dans les formules via `$param.nom`. |
| `tables` | Tables métier. |

Les identifiants (tables, colonnes, rôles, valeurs d'enum) sont en `snake_case`,
63 caractères au plus, et ne doivent pas être des mots réservés de Rust ou de Dart.

### Tables

| Clé | Description |
|---|---|
| `name`, `label` | Nom technique et libellé (texte ou `{ "fr": …, "en": … }`). |
| `columns` | Colonnes métier. |
| `views` | `calendar` : `{ start, end? \| duration? }` ; `stats` : `{ fields, group_by? }`. |
| `rules` | Règles d'autorisation (voir plus bas). Sans règle, seul `admin` accède à la table. |

Ajoutées automatiquement à chaque table : `id`, `created_at`, `updated_at`, `owner`.
Tables système réservées : `users`, `roles`, `user_roles`, `parameters`, `refresh_tokens`.

### Types de colonnes

| Type | Description | Options propres |
|---|---|---|
| `string`, `text` | Texte court / long | |
| `integer`, `decimal` | Nombres | |
| `boolean` | Booléen | |
| `date`, `datetime` | Date (`AAAA-MM-JJ`), date-heure (RFC 3339) | |
| `duration` | Durée en secondes | |
| `enum` | Valeur parmi une liste | `values` |
| `reference` | Relation N→1 | `target`, `inverse` |
| `reference_list` | Relation N↔N (table de jointure) | `target`, `inverse` |
| `lookup` | Valeur lue via un chemin de références, lecture seule | `path` (ex. `entreprise.secteur`) |

`inverse` nomme la relation vue depuis la table cible (par défaut : le nom de la
table source). Il sert dans les agrégats : `SUM(opportunites.montant)`.

### Options de colonne

| Option | Effet | Contraintes |
|---|---|---|
| `label` | Libellé affiché | Langues de `app.locales` |
| `required` | Valeur obligatoire | Pas sur une colonne calculée |
| `unique` | Valeur unique | Colonne stockée et indexable (pas `text`) |
| `hidden` | Absente des vues, reste dans l'API | |
| `title_field` | Compose l'intitulé du record | Ni `hidden` ni `reference_list` |
| `default` | Valeur par défaut, du type de la colonne | Pas sur une relation ni une colonne calculée |
| `formula` | Colonne calculée | Types simples uniquement |
| `persist` | Avec `formula` : valeur stockée en base, recalculée quand une dépendance change | Pas de fonction volatile (`TODAY`) |
| `renamed_from` | Ancien nom, pour une migration par renommage | |

### Formules

```text
montant * probabilite / 100
ROUND(montant * (1 + $param.tva / 100), 2)
IF(etape == "gagne", montant, 0)
DAYS_BETWEEN(TODAY(), date_cloture)
CONCAT(contact.prenom, " ", contact.nom)
SUM(opportunites.montant)      COUNT(tags)
```

- Opérateurs : `+ - * /`, `== != < <= > >=` (alias `=` et `<>`), `AND OR NOT`.
- Littéraux : nombres, `"texte"`, `TRUE`, `FALSE`, `NULL`.
- Références : colonne (`montant`), chemin de références (`entreprise.secteur`), paramètre (`$param.tva`).
- Fonctions : `IF`, `ROUND`, `CONCAT`, `DAYS_BETWEEN`, `TODAY`.
- Agrégats sur une relation « plusieurs » (inverse ou `reference_list`) : `SUM`, `AVG`, `MIN`, `MAX`, `COUNT`.

La validation vérifie la syntaxe, l'existence des colonnes et des relations,
l'usage des agrégats, et détecte les dépendances circulaires (y compris entre tables).

### Règles d'autorisation

```json
{ "roles": ["commercial"], "actions": ["update", "delete"], "when": "owner == $user.id" }
```

`actions` : `read`, `create`, `update`, `delete` ou `*`. La condition `when` utilise
le langage des formules, avec `$user.id` et `$user.email`. Elle doit rester
traduisible en SQL, pour filtrer aussi les listes : uniquement des colonnes stockées
de la table, des comparaisons et `AND`/`OR`/`NOT`, sans fonctions.

## Architecture

```text
crates/
├── forge-schema/   format d'entrée : types serde, validation, modèle résolu, JSON Schema
├── forge-formula/  langage de formules : lexer, parser, AST, registre de fonctions
└── forge-cli/      binaire `forge`
examples/crm/       schéma de référence (validé en CI)
```

Les crates `forge-codegen`, `forge-runtime` et le package `forge_flutter`
seront ajoutés à partir des phases où ils deviennent utiles.

## Développement

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -p forge-cli -- schema > forge.schema.json   # après modification de spec.rs
```
