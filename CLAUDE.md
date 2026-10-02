# CLAUDE.md

Guide pour travailler sur ce dépôt : forge, un générateur d'applications
(backend Rust REST + GraphQL, app Flutter, infra) à partir d'un schéma JSON.

## Commandes

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings    # clippy::pedantic activé au niveau workspace
cargo test
cargo run -p forge-cli -- validate examples/crm/schema.json
cargo run -p forge-cli -- schema > forge.schema.json   # obligatoire après modification de spec.rs
```

Un test (`json_schema_file_is_up_to_date`) échoue si `forge.schema.json` est obsolète.

## Règles de travail

- Développement phase par phase (roadmap dans le README). En fin de phase : fmt, clippy
  et tests passent, README et CLAUDE.md sont à jour, puis on s'arrête pour validation.
- Pas de code mort, pas de TODO vides, pas de fonctionnalités simulées. Une crate
  n'est créée que dans la phase où elle sert.
- Choix structurants non couverts par la spécification : demander avant de coder.
- Langue : identifiants en anglais ; commentaires, docs et messages d'erreur en français.

## Architecture

| Crate | Rôle |
|---|---|
| `forge-formula` | Langage de formules : `lexer` → `parser` → `ast`. `FunctionRegistry` liste les signatures (arité, agrégat, volatile). Aucune connaissance du schéma. |
| `forge-schema` | `spec` : types serde du `forge.json` (source du JSON Schema, `deny_unknown_fields`). `validate` : validation sémantique produisant un `Model` (relations résolues, AST des formules et conditions, ordre topologique des colonnes calculées). `graph` : tri et cycles. `names` : identifiants et mots réservés. |
| `forge-cli` | Binaire `forge` (clap, anyhow). Commandes : `validate`, `schema`. |

Crates prévues (pas encore créées) : `forge-codegen` (templates minijinja embarqués),
`forge-runtime` (logique commune des apps générées), et le package Dart
`packages/forge_flutter`.

### Principes

- Le maximum de logique vit dans les runtimes (`forge-runtime`, `forge_flutter`).
  Le code généré se limite aux déclarations typées : entités, objets GraphQL, branchement.
- Le backend généré embarque le schéma ; le runtime le relit avec `forge-schema`
  pour piloter CRUD, règles, formules, CSV, agrégats et OpenAPI.
- Régénération sans perte : `src/generated/` est réécrit, `src/custom/` jamais.
  Même principe côté Flutter (`lib/generated/`, `lib/custom/`).
  `forge generate` doit être idempotent.
- L'app générée dépend de `forge-runtime` par chemin (`--runtime-path`).

### Validation (`forge-schema/src/validate.rs`)

- Les erreurs sont collectées (`Vec<Issue>`), jamais d'arrêt au premier problème.
- Chaque `Issue` porte un chemin JSON (`tables[2].columns[4].formula`) ; les erreurs de
  formule ajoutent `(position N)`.
- Une erreur découlant d'une erreur déjà signalée n'est pas répétée
  (`resolve` renvoie `Err(None)` dans ce cas).
- Pour ajouter une règle : la coder dans `Validator`, puis ajouter un cas minimal dans
  `crates/forge-schema/tests/validation.rs` (schéma `base()` modifié d'une seule erreur).

### Décisions prises

- Format JSON : ajouts par rapport à la spécification initiale, validés avec l'utilisateur :
  `parameters` à la racine, `label` par colonne, `inverse` sur les relations,
  `renamed_from`, `views.calendar.end|duration`.
- `duration` est stocké en secondes (entier).
- Le rôle `admin` est obligatoire (compte administrateur initial).
- Formules : `=`/`<>` acceptés comme alias de `==`/`!=` ; comparaisons non associatives ;
  agrégats non imbriqués ; argument d'agrégat = chemin vers une relation « plusieurs ».
- Une formule `persist` ne peut pas appeler de fonction volatile (`TODAY`).
- Conditions `when` : uniquement colonnes stockées de la table, `$user.id|email`,
  `$param.*`, littéraux et opérateurs, sans fonctions, pour rester traduisibles en SQL.
- Hors périmètre v1 : temps réel, multi-tenant, workflows, upload de fichiers.
