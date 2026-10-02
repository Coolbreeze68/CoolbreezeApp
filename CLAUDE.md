# CLAUDE.md

Guide pour travailler sur ce dépôt : forge, un générateur d'applications
(backend Rust REST + GraphQL, app Flutter, infra) à partir d'un schéma JSON.

## Commandes

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings  # pedantic sur les crates forge
cargo test --all-features
cargo run -p forge-cli -- validate examples/crm/forge.json
cargo run -p forge-cli -- schema > forge.schema.json    # obligatoire après modification de spec.rs
cargo run -p forge-cli -- generate --dir examples/crm   # obligatoire après modification des templates/codegen
TEST_DATABASE_URL=postgres://… cargo test -p mini_crm    # CRUD du CRM sur PostgreSQL ou MySQL
TEST_DATABASE_URL=postgres://… cargo test -p forge-runtime --test migration  # migrations avec données
TEST_CACHE_URL=redis://localhost:6379 cargo test -p mini_crm --features forge-runtime/redis  # CRM avec Redis
TEST_CACHE_URL=redis://… cargo test -p forge-runtime --features redis --test redis -- --include-ignored
./scripts/e2e-evolution.sh                              # scénario complet (DATABASE_URL : base vide)
```

Tests de cohérence : `json_schema_file_is_up_to_date` échoue si `forge.schema.json`
est obsolète, `crm_example_is_up_to_date` si `examples/crm` ne correspond plus à ce
que produit le générateur. `examples/crm/backend` est membre du workspace : son test
CRUD généré tourne avec `cargo test`.

Postgres, MariaDB et Redis peuvent être installés localement (pas de Docker dans
l'environnement cloud) ; la CI teste PostgreSQL 16 et MySQL 8.4.

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
| `forge-formula` | Langage de formules : `lexer` → `parser` → `ast`, `typecheck` (trait `TypeEnv`), `eval` (trait `Env`, sémantique NULL). `FunctionRegistry` : signatures (arité, types, agrégat, volatile) et implémentations (intégrées et personnalisées). Aucune connaissance du schéma. |
| `forge-schema` | `spec` : types serde du `forge.json` (source du JSON Schema, `deny_unknown_fields`). `graphql` : noms GraphQL dérivés des tables et détection de leurs conflits. `validate` : validation sémantique produisant un `Model` (relations résolues, AST des formules et conditions, ordre topologique des colonnes calculées). `value` : conversion JSON/texte → `TypedValue`, partagée par la validation et le runtime. `graph` : tri et cycles. `names` : identifiants et mots réservés. |
| `forge-codegen` | `layout` : structure de stockage (ce que les migrations créent), comparée à `.forge/snapshot.json`. `diff` : changements entre deux layouts, leur risque (`Safe`/`MayFail`/`DataLoss`) et `Hints` (renommages, défauts tirés du schéma). `migration` : rendu des opérations `Plan` (montée = diff, descente = diff inverse). `backend` : vues des templates. `render` : minijinja + `rustfmt` (si présent). `writer` : politiques `Generated` (réécrit, obsolètes supprimés) / `Once` (jamais écrasé). |
| `forge-runtime` | `app` : `App` (assemblage) et `AppState`. `auth` : `AuthConfig`, jetons, `CurrentUser` (extracteur), middleware, `/api/auth/*` ; `auth::users` : comptes (admin), rôles, admin initial. `rules` : portée d'une action (`Scope`), conditions `when` → SQL. `resource` : trait `Service` (opérations d'une table, indépendantes du transport : liste, lecture, agrégats, écriture, import) implémenté par `Resource<E, H>` pour chaque entité. `rest`, `graphql` (schéma dynamique, `DataLoader` des références) et `csv_io` n'appellent que `Service`. `openapi` : document OpenAPI écrit en JSON depuis le modèle, chargé dans `utoipa` et servi par Swagger UI. `cache` : trait `Cache` (`MemoryCache` moka, `RedisCache` sous feature `redis`), `Reads` (clés versionnées, dépendances par table calculées par `compute::read_dependencies`). `observability` : journaux (`LogFormat`), `x-request-id`, trace, métriques Prometheus, `/health`, `/metrics`. `compute` : formules et lookups (`complete` à la lecture, `propagate` des formules persistées après écriture ; `graph` charge les lignes par lots, `paths` résout les chemins). `aggregate` : `/api/<table>/aggregate` en SQL. `payload` : validation des corps. `query` : pagination/tri/filtres. `links` : tables de jointure. `hooks` : trait `Hooks<E>`. `parameters`. `migration` : `Plan`/`TableDef` (exécution des migrations par base) + migrations système. `cli` : binaire généré. `testing` (feature) : `TestDatabase` (verrou sur base partagée), `TestClient` (connecté en admin, `as_new_user`, `anonymous`, `graphql`, `request_text`), `check_resources` (CRUD REST, aller-retour CSV, GraphQL), `check_rules`. |
| `forge-cli` | Binaire `forge` : `new`, `generate`, `migrate`, `validate`, `schema`. |

Templates : `templates/backend/*.j2`, embarqués via `include_str!` (liste dans
`forge-codegen/src/render.rs`). À prévoir : `packages/forge_flutter` (phase 7).

### Principes

- Le maximum de logique vit dans les runtimes. Le code généré se limite aux
  déclarations typées : entités, migration, branchement (`generated/mod.rs`).
- Le backend généré embarque une copie de `forge.json` (`src/generated/forge.json`) ;
  le runtime la relit pour piloter validation, filtres, défauts, liens, paramètres.
- Régénération sans perte : `src/generated/` et `tests/generated_crud.rs` sont réécrits,
  `src/custom/`, `Cargo.toml`, `main.rs`, `lib.rs` et les migrations jamais.
  Les hooks (`src/custom/hooks/<table>.rs`) sont déclarés par `generated/hooks.rs`
  via `#[path]`, pour que l'ajout d'une table ne touche aucun fichier utilisateur ;
  de même pour `src/custom/functions.rs` et `src/custom/graphql.rs`.
- Les hooks, les fonctions (`src/custom/functions.rs`) et les champs GraphQL
  (`src/custom/graphql.rs`, `App::graphql_query|graphql_mutation|graphql_type`)
  sont les points d'extension du code utilisateur.
- `forge generate` est idempotent (fichier identique = non réécrit).
- L'app générée dépend de `forge-runtime` par chemin (relatif, ou absolu si les
  chemins n'ont que la racine en commun).
- Le code généré doit passer `clippy -D warnings` (lints par défaut) : l'exemple CRM
  est compilé par la CI.

### Runtime : conventions

- Colonnes en base = noms du schéma (une `reference` `entreprise` stocke l'id dans
  la colonne `entreprise`). Tables de jointure : `<table>_<colonne>` (`source_id`, `target_id`).
- Valeurs : `TypedValue` → `values::to_db` produit la variante sea-orm exacte, y compris
  pour `NULL` (`try_set` refuse une variante incorrecte).
- `decimal(19,4)`, `varchar(255)` (longueur vérifiée par `payload`), `bigint` pour les
  entiers, durées et identifiants.
- Suppression : référence obligatoire → `RESTRICT`, facultative → `SET NULL`,
  jointure → `CASCADE`.
- Contraintes nommées d'après la colonne : `fk_<table>_<colonne>`, index unique
  `uq_<table>_<colonne>` (jamais d'unicité en ligne : il faut pouvoir la supprimer).
  Renommer une telle colonne supprime puis recrée la contrainte sous le nouveau nom.

### Authentification et règles

- Middleware sur `/api/` (sauf `login` et `refresh`) : jeton d'accès JWT HS256 lu
  dans `Authorization: Bearer`, `CurrentUser` placé dans les extensions de la requête.
  Les rôles viennent du jeton (rechargés au rafraîchissement).
- Jeton de rafraîchissement : 32 octets aléatoires, stocké en SHA-256, rotation à
  chaque usage. Mots de passe : argon2id (optimisé même en debug, voir `Cargo.toml`).
- Migration système `forge_0002_auth` : colonnes de `users`, tables `roles`,
  `user_roles`, `refresh_tokens`. Rôles du schéma et admin initial créés par
  `App::into_router`.
- `admin` a tous les droits. Sinon `rules::scope` retourne `Scope::All`,
  `Scope::Where(condition SQL)` ou `403`. Lecture hors périmètre → `404` ;
  update/delete vérifient la condition avant (et après pour update) ; create après
  insertion, dans la transaction. `owner` = créateur.
- Calculer les portées **avant** d'ouvrir une transaction : elles lisent la base
  (paramètres) via le pool, qui n'a qu'une connexion en SQLite de test.

### Migrations

- Chaque migration générée est un `Plan` d'opérations explicites et figées (jamais
  recalculées par le runtime) : la migration initiale est le diff depuis un layout
  vide, la descente le diff inverse. `Plan::apply` ordonne les opérations lui-même.
- PostgreSQL / MySQL : opérations directes (suppressions, renommages, créations,
  ajouts et modifications, puis index uniques et clés étrangères).
- SQLite : créations/suppressions directes ; chaque table modifiée est reconstruite
  depuis sa définition complète (`redefine`, émis par le générateur pour toute table
  modifiée), clés étrangères désactivées, dans une transaction, puis
  `PRAGMA foreign_key_check`. Exige une connexion unique : `cli::connect(.., true)`
  et `testing::database` limitent le pool à 1 pour SQLite.
- Les valeurs par défaut du schéma sont posées en base (`.default(...)`) pour remplir
  les lignes existantes ; le runtime continue d'appliquer celles du schéma à la création.
- Nom : `mNNNN_<tables concernées>` (`m0001_init` pour la première).
- Erreurs : `Error` → JSON `{ error: { code, message, fields? } }` ; les erreurs
  internes sont journalisées, pas exposées.

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
- Le schéma du CRM est `examples/crm/forge.json` (le projet de référence est versionné).
- Fonctions personnalisées : déclarées dans `functions` (types des arguments et du
  résultat), implémentées dans `src/custom/functions.rs` (`App::function`) ;
  `into_router` échoue si une implémentation manque ou ne correspond à aucune déclaration.
- Types des formules contrôlés à la validation (seulement si le schéma n'a pas
  d'autre erreur) ; une date-heure est acceptée là où une date est attendue.
- Formules non persistées et lookups : évalués à chaque lecture ; un échec
  d'évaluation donne `null` et est journalisé. Formules persistées : recalculées
  dans la transaction de l'écriture (lignes touchées trouvées en remontant les
  chemins de chaque formule, voisinage lu avant et après l'écriture), écrites
  seulement si la valeur change ; un échec refuse l'écriture (`422`). Un changement
  de paramètre recalcule toutes les lignes dont une formule persistée le lit.
- Décimaux calculés arrondis à 4 décimales (demi s'éloignant de zéro), entiers à l'unité.
- Agrégats : mesures lues depuis `QueryResult` (types variables selon base et
  fonction) et non via `into_json`, qui perd sous SQLite les colonnes sans type déclaré.
- REST, GraphQL et CSV passent par `Service` : toute règle métier (validation,
  autorisation, hooks, calculs) se code une fois, dans `resource.rs` ou en dessous.
  Ne pas convertir une erreur du runtime en erreur GraphQL par `?` (conversion
  `Display` d'async-graphql : code perdu, erreur interne exposée) : `to_graphql`.
- GraphQL : schéma dynamique construit dans `App::into_router` ; un `DataLoader` par
  requête charge les références par lots (`Service::read_many`, droits de
  l'utilisateur ; cible illisible → `null`). `ID` en texte, `Decimal`/`Date`/
  `DateTime`/`JSON` en scalaires, `enum` en énumération GraphQL. Arguments de liste
  convertis en paires de requête REST (mêmes validations). Auth et comptes : REST seul.
- CSV : import tout ou rien, une transaction et un point de sauvegarde par ligne
  (une erreur SQL n'interrompt pas la vérification des lignes suivantes) ; erreur
  interne → arrêt immédiat. `Error::Import(Vec<RowError>)` → `422`, `error.lines`.
  Cellule vide : omise en création (défaut), `null` en modification.
- OpenAPI : JSON désérialisé dans `utoipa::openapi::OpenApi` (chaque schéma doit
  avoir un `type`, chaque paramètre `required`) ; Swagger UI embarqué (`vendored`).
  `/docs`, `/openapi.json` et `/graphql` (GraphiQL) sont hors de `/api/`, donc publics.
- Décimaux renvoyés normalisés (`"500"`, pas `"500.0000"` sous PostgreSQL/MySQL).
- Cache : toute écriture passe par un `Written` (tables écrites : la table, les
  tables recalculées par `propagate`, celles signalées par `HookContext::modified`),
  invalidé **après** le commit (avant, une lecture concurrente remettrait l'ancienne
  valeur en cache sous la nouvelle version). Une nouvelle source d'écriture doit
  faire de même (`AppState::invalidate`). Une nouvelle dépendance de lecture (une
  table lue par la sortie d'une autre) s'ajoute dans `compute::read_dependencies`.
  Le cache est actif par défaut, donc dans tous les tests : une invalidation
  manquante y apparaît comme une lecture périmée.
- Une erreur de cache ne fait jamais échouer une requête (journalisée, lecture en
  base) ; sans versions lisibles, la lecture contourne le cache.
- Métriques : enregistreur Prometheus global au processus (`OnceLock`), libellé
  `path` = modèle de route (`MatchedPath`), jamais l'URL (cardinalité).
- `rust-version` 1.88 (exigé par utoipa-swagger-ui) : let-chains utilisables.
- Tests sur PostgreSQL/MySQL : base partagée, tests d'un binaire sérialisés par le
  verrou de `TestDatabase`.
- Migrations : `DataLoss` refusé sans `--allow-destructive` (`Error::Destructive`, rien
  n'est écrit) ; `MayFail` signalé (avertissement + en-tête de migration). Renommage de
  table non géré (suppression + création). MySQL n'exécute pas les migrations en
  transaction (DDL non transactionnel).
- SQLite stocke les `decimal` en flottant (limite de sea-orm/sqlx) : réservé au
  développement et aux tests.
- Hors périmètre v1 : temps réel, multi-tenant, workflows, upload de fichiers.
