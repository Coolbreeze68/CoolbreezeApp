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
| 1 | Backend : entités, migration initiale, CRUD REST (pagination, tri, filtres), `parameters`, hooks, routes personnalisées ; SQLite, PostgreSQL, MySQL | ✅ |
| 2 | Migrations incrémentales : diff du schéma, renommages, protection des changements destructifs, retour arrière | ✅ |
| 3 | Auth, rôles, moteur de règles | à venir |
| 4 | Évaluation des formules, lookups, `persist`, agrégats | à venir |
| 5 | GraphQL, import/export CSV, OpenAPI | à venir |
| 6 | Observabilité, cache | à venir |
| 7 | Application Flutter | à venir |
| 8 | Docker, docker-compose, GitHub Actions de l'app générée | à venir |

Ce qui n'est **pas encore** disponible (phases suivantes) : l'authentification
(`owner` reste vide), les règles d'autorisation, le calcul des formules et des
lookups (absents des réponses), GraphQL, CSV, OpenAPI, l'application Flutter et Docker.

## Installation

```bash
cargo install --path crates/forge-cli
forge --help
```

Les projets générés dépendent de `forge-runtime` par chemin : le binaire `forge`
pointe vers les sources à partir desquelles il a été compilé (option `--runtime-path`
pour en choisir d'autres).

## Commandes

| Commande | Rôle |
|---|---|
| `forge new <dir> --schema <fichier>` | Crée un projet : copie le schéma dans `<dir>/forge.json` et génère le code. |
| `forge generate [--dir .] [--allow-destructive]` | Régénère le code à partir de `forge.json` et crée une migration si la structure de stockage a changé. Idempotent : sans modification du schéma, aucun fichier ne change. |
| `forge migrate [up\|down\|status] [--dir .]` | Applique, annule ou liste les migrations (`cargo run -- migrate …` dans `backend/`). |
| `forge validate [schema.json]` | Valide un schéma (défaut : `forge.json`) et liste **toutes** les erreurs avec leur chemin. |
| `forge schema` | Affiche le JSON Schema du format d'entrée (contenu de `forge.schema.json`). |

Exemple de sortie de `forge validate` :

```text
schema.json : 3 erreur(s)
  - tables[2].columns[6].formula: `probabilit` n'est ni une colonne ni une relation de la table `opportunite` (position 10)
  - tables[2].rules[2].roles[0]: rôle `vendeur` non déclaré dans `roles`
  - tables[2].columns[13].formula: dépendance circulaire : opportunite.boucle → opportunite.boucle
```

## Tutoriel : un mini CRM

Le schéma de référence [`examples/crm/forge.json`](examples/crm/forge.json) décrit
cinq tables : `entreprise`, `contact`, `opportunite`, `activite` et `tag`.
Le projet généré correspondant est versionné dans [`examples/crm/`](examples/crm).

### 1. Créer le projet

```bash
forge new crm --schema examples/crm/forge.json
```

```text
crm/
├── forge.json                 # votre schéma : la source de vérité
├── .forge/snapshot.json       # structure de stockage à la dernière migration
└── backend/
    ├── Cargo.toml             # à vous (créé une fois)
    ├── src/main.rs, lib.rs    # à vous (créés une fois)
    ├── src/generated/         # NE PAS MODIFIER : réécrit à chaque génération
    │   ├── entities/          #   une entité sea-orm par table
    │   ├── hooks.rs           #   déclare vos fichiers de hooks
    │   ├── migrations.rs      #   liste des migrations
    │   └── mod.rs             #   assemblage : app()
    ├── src/custom/            # votre code, jamais modifié par forge
    │   ├── hooks/<table>.rs   #   hooks de chaque table
    │   └── routes.rs          #   routes HTTP personnalisées
    ├── src/migrations/        # migrations (une fois créées, jamais réécrites)
    └── tests/generated_crud.rs  # test CRUD de chaque table (réécrit)
```

### 2. Démarrer l'API

```bash
cd crm/backend
cargo run                        # SQLite `data.db`, port 8080, migrations appliquées
DATABASE_URL=postgres://user:mdp@localhost/crm cargo run    # ou PostgreSQL / MySQL
```

Variables : `DATABASE_URL`, `FORGE_ADDR` (défaut `0.0.0.0:8080`),
`FORGE_AUTO_MIGRATE` (défaut `true`), `RUST_LOG`.

### 3. Utiliser l'API

```bash
curl -X POST localhost:8080/api/entreprise -H 'content-type: application/json' \
     -d '{"nom": "Acme", "secteur": "industrie", "chiffre_affaires": "1250000.50"}'
curl -X POST localhost:8080/api/tag -H 'content-type: application/json' -d '{"nom": "urgent"}'
curl -X POST localhost:8080/api/opportunite -H 'content-type: application/json' \
     -d '{"titre": "Contrat cadre", "entreprise": 1, "montant": "45000", "tags": [1]}'

curl -g 'localhost:8080/api/opportunite?etape=prospect&montant[gte]=10000&sort=-montant'
curl -X PATCH localhost:8080/api/opportunite/1 -H 'content-type: application/json' -d '{"etape": "gagne"}'
curl -X PUT localhost:8080/api/parameters/tva -H 'content-type: application/json' -d '{"value": 5.5}'
```

Les valeurs par défaut du schéma sont appliquées (`probabilite` vaut 50, `etape`
vaut `prospect`), et les erreurs sont détaillées par champ :

```json
{ "error": { "code": "validation", "message": "données invalides",
  "fields": { "etape": ["`signe` ne fait pas partie des valeurs (prospect, proposition, gagne, perdu)"],
              "titre": ["valeur obligatoire"] } } }
```

### 4. Ajouter un hook

Les hooks d'une table se trouvent dans `backend/src/custom/hooks/<table>.rs`.
Pour refuser les opportunités à montant nul ou négatif :

```rust
use forge_runtime::{Error, HookContext, Hooks};
use sea_orm::prelude::Decimal;

use crate::generated::entities::opportunite::{ActiveModel, Entity};

#[derive(Debug, Default)]
pub struct OpportuniteHooks;

impl Hooks<Entity> for OpportuniteHooks {
    async fn validate(&self, _ctx: &HookContext<'_>, record: &ActiveModel) -> Result<(), Error> {
        match record.montant.try_as_ref() {
            Some(montant) if *montant <= Decimal::ZERO => Err(Error::validation(
                "montant",
                "le montant doit être strictement positif",
            )),
            _ => Ok(()),
        }
    }
}
```

Méthodes disponibles (toutes facultatives) : `before_create`, `after_create`,
`before_update`, `after_update`, `before_delete`, `validate`. Elles s'exécutent dans
la transaction de la requête (`ctx.db()`) ; une erreur annule toute l'opération.
Ordre : `before_create`/`before_update` → `validate` → écriture → `after_*`.

### 5. Ajouter une route

```rust
// backend/src/custom/routes.rs
pub fn routes() -> Router<AppState> {
    Router::new().route("/api/ping", axum::routing::get(|| async { "pong" }))
}
```

### 6. Tester

```bash
cargo test                                            # SQLite en mémoire
TEST_DATABASE_URL=postgres://… cargo test             # base recréée : base de test dédiée !
```

`tests/generated_crud.rs` vérifie création, lecture, liste, filtre, modification,
suppression et contraintes de chaque table. Pour vos propres tests,
`forge_runtime::testing::TestClient` envoie des requêtes à l'application sans
réseau : voir [`examples/crm/backend/tests/hooks.rs`](examples/crm/backend/tests/hooks.rs).

### 7. Faire évoluer le schéma

Ajoutons une colonne `effectif` aux entreprises et renommons `ville` en `commune`
dans `forge.json` :

```json
{ "name": "commune", "type": "string", "renamed_from": "ville" },
{ "name": "effectif", "type": "integer", "default": 10 }
```

```text
$ forge generate
Migration m0002_entreprise :
  - `entreprise` : `ville` renommée en `commune`
  - `entreprise` : nouvelle colonne `effectif`
Appliquez-la avec `forge migrate`.

  créé       backend/src/migrations/m0002_entreprise.rs
  mis à jour backend/src/generated/entities/entreprise.rs
  …
$ forge migrate          # ou au prochain démarrage du serveur
```

La colonne apparaît dans l'entité et l'API, les données sont conservées (`commune`
reprend les valeurs de `ville`, les lignes existantes reçoivent `effectif = 10`), et
`src/custom/` n'est pas touché. `forge migrate down` annule la dernière migration.

Ce que fait `forge generate` :

- il compare la structure de stockage du schéma à celle de la dernière migration
  (`.forge/snapshot.json`) ; les changements sans effet sur le stockage (libellés,
  vues, règles…) ne créent pas de migration ;
- la migration créée (`src/migrations/mNNNN_<tables>.rs`) n'est plus jamais réécrite :
  vous pouvez la compléter (reprise de données, index) avant de l'appliquer ;
- un changement qui **perd des données** (suppression de table ou de colonne,
  changement de type) est refusé, avec la liste des pertes, sauf avec
  `--allow-destructive` ; la migration porte alors un avertissement en en-tête ;
- un changement qui **peut échouer** selon les données (colonne devenue obligatoire
  ou unique, nouvelle référence sur une colonne existante, colonne obligatoire sans
  valeur par défaut) est signalé, et noté dans la migration ;
- sans `renamed_from`, un renommage est vu comme une suppression suivie d'un ajout,
  donc refusé comme destructif. Le renommage d'une table n'est pas pris en charge.

Les valeurs par défaut du schéma deviennent aussi celles de la base : elles
remplissent les lignes existantes quand une colonne est ajoutée.

## API REST générée

| Méthode | Chemin | Effet |
|---|---|---|
| `GET` | `/api/<table>` | Liste : `{ "data": [...], "page", "per_page", "total" }` |
| `POST` | `/api/<table>` | Création → `201` |
| `GET` | `/api/<table>/{id}` | Lecture |
| `PATCH` | `/api/<table>/{id}` | Modification partielle |
| `DELETE` | `/api/<table>/{id}` | Suppression → `204` |
| `GET` | `/api/parameters` | Paramètres et leurs valeurs |
| `GET`/`PUT` | `/api/parameters/{name}` | Lecture / modification : `{ "value": … }` |

Paramètres de liste :

| Paramètre | Exemple | Effet |
|---|---|---|
| `page`, `per_page` | `page=2&per_page=50` | Pagination (25 par défaut, 100 maximum) |
| `sort` | `sort=-montant,titre` | Tri ; `-` pour décroissant |
| `q` | `q=dupont` | Recherche (contient, sans casse) dans les colonnes texte |
| `<colonne>` | `etape=gagne` | Égalité |
| `<colonne>[op]` | `montant[gte]=1000` | `ne`, `lt`, `lte`, `gt`, `gte`, `like`, `in` (`a,b`), `null` (`true`/`false`) |

Seules les colonnes stockées (dont `id`, `owner`, `created_at`, `updated_at`) sont
filtrables et triables. Formats JSON : décimaux en texte (`"12.50"`, nombres acceptés
en entrée), dates `AAAA-MM-JJ`, dates-heures RFC 3339, durées en secondes,
`reference` = identifiant, `reference_list` = liste d'identifiants.

Codes d'erreur : `400` requête mal formée, `404` introuvable, `409` conflit (valeur
unique déjà prise, référence invalide, suppression d'un enregistrement référencé par
une référence obligatoire), `422` validation (détail par champ).

Suppression : une référence obligatoire bloque la suppression de sa cible, une
référence facultative est remise à `null`, les liens `reference_list` sont supprimés.

### Bases de données

| | SQLite | PostgreSQL | MySQL / MariaDB |
|---|---|---|---|
| Usage conseillé | développement, tests | production | production |
| `decimal` | flottant (~15 chiffres significatifs) | `decimal(19,4)` exact | `decimal(19,4)` exact |
| `datetime` | texte | `timestamptz` | `timestamp` (1970–2038, à la seconde) |
| Migrations | reconstruction des tables modifiées (données recopiées), en transaction | en transaction | sans transaction : une migration interrompue reste partielle |

## Format d'entrée

Référence complète et autocomplétion : [`forge.schema.json`](forge.schema.json).
Ajoutez en tête de votre fichier :

```json
{ "$schema": "chemin/vers/forge.schema.json", ... }
```

Exemple complet : [`examples/crm/forge.json`](examples/crm/forge.json).

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
Une `reference_list` crée la table de jointure `<table>_<colonne>`. Un cycle de
références obligatoires (A exige B qui exige A) est refusé : aucun enregistrement ne
pourrait être créé.

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
| `renamed_from` | Ancien nom : la migration renomme la colonne au lieu de la supprimer et de la recréer | Pas le nom d'une colonne existante ; peut rester en place après la migration |

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
├── forge-schema/   format d'entrée : types serde, validation, modèle résolu, valeurs typées, JSON Schema
├── forge-formula/  langage de formules : lexer, parser, AST, registre de fonctions
├── forge-codegen/  génération : structure de stockage, diff et migrations, templates, écriture idempotente
├── forge-runtime/  logique des apps générées : CRUD générique, filtres, hooks, exécution des migrations, CLI
└── forge-cli/      binaire `forge`
templates/backend/  templates minijinja, embarqués dans le binaire
examples/crm/       projet de référence généré (membre du workspace, testé en CI)
scripts/            scénario de bout en bout (évolution du schéma)
```

Le backend généré embarque son schéma (`src/generated/forge.json`) : `forge-runtime`
le relit au démarrage et en tire tout le comportement générique (validation des
corps, filtres, valeurs par défaut, liens N↔N, paramètres). Le code généré se
limite aux entités sea-orm typées, à la migration et au branchement des hooks.

## Développement

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features                              # inclut le CRUD du CRM sur SQLite
TEST_DATABASE_URL=postgres://… cargo test -p mini_crm  # idem sur PostgreSQL ou MySQL
cargo run -p forge-cli -- schema > forge.schema.json   # après modification de spec.rs
cargo run -p forge-cli -- generate --dir examples/crm  # après modification des templates ou du codegen
./scripts/e2e-evolution.sh                             # scénario complet : création, données, évolution, migration
DATABASE_URL=postgres://…/vide ./scripts/e2e-evolution.sh   # idem sur une base PostgreSQL ou MySQL vide
```
