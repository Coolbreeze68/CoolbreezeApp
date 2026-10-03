# CLAUDE.md

Guide pour travailler sur ce dépôt : forge, un générateur d'applications
(backend Rust REST + GraphQL, interface Flutter et/ou web React, infra) à partir d'un schéma JSON.

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
(cd packages/forge_flutter && dart format --set-exit-if-changed lib test && flutter analyze && flutter test)
(cd examples/crm/app && dart format --set-exit-if-changed lib test && flutter analyze && flutter test)
FORGE_E2E_URL=http://127.0.0.1:8080 flutter test test/api_test.dart   # dans examples/crm/app, backend du CRM lancé
(cd packages/forge_web && npm ci && npm run lint && npm run typecheck && npm test)
(cd examples/crm/web && npm ci && npm run typecheck && npm test && npm run build)
```

Docker : `dockerd` se lance dans l'environnement cloud (`nohup dockerd &`), mais
les conteneurs ne passent que par le proxy HTTPS : pour un test local, construire
avec `--network host`, les `--build-arg http(s)_proxy`, le certificat
`/root/.ccr/ca-bundle.crt` et apt en HTTPS (copie locale du Dockerfile, jamais
versionnée).

Flutter n'est pas préinstallé dans l'environnement cloud : archive stable de
`storage.googleapis.com/flutter_infra_release` décompressée dans `/opt/flutter`.
Pour tester l'app web dans Chromium (Playwright) : `flutter build web
--no-web-resources-cdn` (CDN bloqué) et `locale: 'fr-FR'` sur la page (la langue
par défaut du Chromium headless fait échouer le moteur Flutter).

Tests de cohérence : `json_schema_file_is_up_to_date` échoue si `forge.schema.json`
est obsolète, `crm_example_is_up_to_date` si `examples/crm` ne correspond plus à ce
que produit le générateur. `examples/crm/backend` est membre du workspace : son test
CRUD généré tourne avec `cargo test`.

Postgres, MariaDB et Redis peuvent être installés localement ; la CI teste
PostgreSQL 16 et MySQL 8.4, et construit l'image Docker du CRM.

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
| `forge-schema` | `spec` : types serde du `forge.json` (source du JSON Schema, `deny_unknown_fields`). `graphql` : noms GraphQL dérivés des tables et détection de leurs conflits. `validate` : validation sémantique produisant un `Model` (relations résolues, AST des formules et conditions, ordre topologique des colonnes calculées). `value` : conversion JSON/texte → `TypedValue` (contraintes d'une colonne : `Domain`, valeurs d'enum, note maximale), partagée par la validation et le runtime. `graph` : tri et cycles. `names` : identifiants et mots réservés. |
| `forge-codegen` | `infra` : Dockerfile, docker-compose, `.env.example`, CI GitHub et `scripts/use-forge.sh` du projet (templates `templates/infra/`, créés une fois). `flutter` : fichiers de `app/` (schéma et modèles Dart écrits en Rust, le reste par templates `templates/flutter/`). `web` : fichiers de `web/` (schéma en JSON, modèles TypeScript écrits en Rust, le reste par templates `templates/web/`). `frontend` : ce que les deux interfaces partagent (colonne affichée d'un lookup, opérations d'une règle). `dart` : expressions Dart mises en forme comme `dart format` (80 colonnes, un élément par ligne sinon). `layout` : structure de stockage (ce que les migrations créent), comparée à `.forge/snapshot.json`. `diff` : changements entre deux layouts, leur risque (`Safe`/`MayFail`/`DataLoss`) et `Hints` (renommages, défauts tirés du schéma). `migration` : rendu des opérations `Plan` (montée = diff, descente = diff inverse). `backend` : vues des templates. `render` : minijinja + `rustfmt` (si présent). `writer` : politiques `Generated` (réécrit, obsolètes supprimés) / `Once` (jamais écrasé). |
| `forge-runtime` | `app` : `App` (assemblage) et `AppState`. `auth` : `AuthConfig`, jetons, `CurrentUser` (extracteur), middleware, `/api/auth/*` ; `auth::users` : comptes (admin), rôles, admin initial. `rules` : portée d'une action (`Scope`), conditions `when` → SQL. `resource` : trait `Service` (opérations d'une table, indépendantes du transport : liste, lecture, agrégats, écriture, import) implémenté par `Resource<E, H>` pour chaque entité. `rest`, `graphql` (schéma dynamique, `DataLoader` des références) et `csv_io` n'appellent que `Service`. `openapi` : document OpenAPI écrit en JSON depuis le modèle, chargé dans `utoipa` et servi par Swagger UI. `cache` : trait `Cache` (`MemoryCache` moka, `RedisCache` sous feature `redis`), `Reads` (clés versionnées, dépendances par table calculées par `compute::read_dependencies`). `observability` : journaux (`LogFormat`), `x-request-id`, trace, métriques Prometheus, `/health`, `/metrics`. `compute` : formules et lookups (`complete` à la lecture, `propagate` des formules persistées après écriture ; `graph` charge les lignes par lots, `paths` résout les chemins). `aggregate` : `/api/<table>/aggregate` en SQL. `payload` : validation des corps. `query` : pagination/tri/filtres. `links` : tables de jointure. `hooks` : trait `Hooks<E>` ; `Written` (effets après commit : cache, fichiers). `files` : `/api/files` (téléversement, contenu par URL signée), trait `Storage` (`DiskStorage`), table `forge_files`, rattachement aux enregistrements, `expand` (identifiant → description). `parameters`. `migration` : `Plan`/`TableDef` (exécution des migrations par base) + migrations système. `cli` : binaire généré. `testing` (feature) : `TestDatabase` (verrou sur base partagée), `TestClient` (connecté en admin, `as_new_user`, `anonymous`, `graphql`, `request_text`, `upload`), `MemoryStorage`, `check_resources` (CRUD REST, aller-retour CSV, GraphQL), `check_rules`. |
| `forge-cli` | Binaire `forge` : `new`, `generate`, `migrate`, `validate`, `schema`. |
| `packages/forge_flutter` | Package Dart. `schema` : `AppSchema`/`TableSchema`/`ColumnSchema` (const, générés). `api/` : `ForgeClient` (session, renouvellement unique sur `401`, `ApiException`), `TableClient<T>`, `ListQuery`/`Filter`, `KeyValueStore` (`SecureStore`, `MemoryStore`). `values` : JSON ↔ Dart, `ValueFormat` (mise en forme par langue, intitulés). `forge` : `Forge.of(context)`, `DataChanges` (rechargement après écriture), `TitleCache` (intitulés de références par lots). `app`/`router` : `ForgeApp`, go_router, `Paths`. `ui/` : pages et champs. `customization` : `ForgeCustomization`. `l10n/strings` : textes fr/en. `testing.dart` : `FakeApi`. `theme`/`palette` : identité visuelle (`ForgeColors`, `forgeTheme`), couleurs des énumérations. `ui/home_page` : tableau de bord. `api/file` : `ForgeFile`. `ui/field_models` : affichage et saisie des modèles de champ, `createRecordFor`. |
| `packages/forge_web` | Package npm `@forge/web` (React 19, Mantine, react-router), consommé en sources TypeScript. Même découpage que `forge_flutter` : `schema`, `api/` (`ForgeClient`, `TableClient<T>`, `ListQuery`, `BrowserStore`), `values` (`ValueFormat`, `Intl`), `i18n`, `titles` (`TitleCache`), `context` (`useForge`), `hooks` (`useAsync`, `useRecordTitle`), `app` (`ForgeApp`, routes), `ui/` (pages et champs), `customization`, `theme`/`palette`, `testing` (`FakeApi`), `api/file` (`ForgeFile`), `ui/models` (modèles de champ), `ui/CreateRecord` (création d'une référence dans une fenêtre). |

Templates : `templates/{backend,flutter,web,infra}/*.j2`, embarqués via
`include_str!` (liste dans `forge-codegen/src/render.rs`).

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
- App Flutter : `app/lib/generated/` et `app/test/generated_test.dart` sont réécrits,
  `lib/custom/`, `main.dart`, `pubspec.yaml`, `analysis_options.yaml`, `web/` jamais.
  `generated/app.dart` importe `../custom/customization.dart` (même principe que `#[path]`).
- Le code Dart généré doit être stable sous `dart format` (vérifié en CI sur
  l'exemple) : `trailing_commas: preserve` dans `analysis_options.yaml`, et
  `dart::Expr` coupe une expression trop longue un élément par ligne.
- `forge generate` est idempotent (fichier identique = non réécrit).
- App web : `web/src/generated/` est réécrit ; `src/custom/`, `src/main.tsx`,
  `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html` jamais.
  `generated/app.tsx` importe `../custom/customization` (même principe).
- L'app générée dépend de `forge-runtime`, `forge_flutter` et `@forge/web` par chemin (relatif,
  ou absolu si les chemins n'ont que la racine en commun).
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
- App Flutter : client REST (pas GraphQL) ; description des tables générée en Dart
  (pas d'endpoint de schéma) ; un lookup y prend le type de la colonne lue ; les
  règles servent à masquer les actions impossibles, les conditions `when` restent
  vérifiées par le serveur. Colonnes `hidden` absentes de toute l'interface.
- Modèles Dart : classes `PascalCase` (suffixe `Record` si homonyme d'un type de
  `dart:core` ou importé), champs `camelCase` (suffixe `Value` si réservé ou membre),
  une énumération par colonne `enum` (`<Table><Colonne>`, `value` = valeur API) ;
  `toJson` = colonnes modifiables.
- Après une écriture, toutes les pages ouvertes se rechargent (`DataChanges`) : une
  écriture peut changer des formules d'autres tables.
- `setState` ne doit jamais recevoir une fonction fléchée qui renvoie un `Future`
  (`setState(() => _x = _load())` lève une assertion) : bloc `{ … }`.
- `app.frontend` : `"flutter"` (défaut), `"web"` ou les deux ; seules les interfaces
  choisies sont générées (un dossier d'une interface retirée reste, avec un
  avertissement). Les deux interfaces ont les mêmes fonctions et la même identité
  (dégradé indigo → violet, tuiles d'indicateurs, accueil tableau de bord).
- Couleurs des énumérations : palette catégorielle validée (8 teintes, clair et
  sombre, `palette.dart`/`palette.ts`), dans l'ordre de déclaration, gris neutre
  au-delà ; texte toujours neutre (pastille colorée à côté). Barres de groupes
  dans l'ordre de déclaration, groupe vide en dernier.
- App web : modèles TypeScript = types du JSON de l'API (décimaux et dates en
  texte) ; `@forge/web` est lu en sources depuis le projet : `preserveSymlinks` et
  `dedupe` (Vite), `server.deps.inline` (vitest), pour n'avoir qu'un React et un
  Mantine (ceux du projet). `useBlocker` des formulaires : une ref `saved` posée
  avant `navigate`, sinon l'enregistrement est bloqué. Tests : transitions Mantine
  désactivées, options et menus avec `hidden: true`, nombres comparés par regex
  (`\u202f` d'`Intl` non normalisé). Session dans `localStorage` (échec toléré).
- Session Flutter : un échec du stockage (`SecureStore` indisponible sur une page
  web non sécurisée : `http` hors `localhost`) n'empêche jamais la connexion ;
  la session n'est alors pas reprise au rechargement.
- CORS : désactivé par défaut, `FORGE_CORS_ORIGINS` (cli du runtime).
- Déploiement : une image (API + app web servie par `FORGE_STATIC_DIR`, routes
  `/api/` inconnues toujours en JSON) ; PostgreSQL 17 et Redis (profil) dans
  docker-compose ; aucune publication d'image (pas de registre). Les sources de
  forge entrent dans l'image comme contexte nommé `forge` (`Options::forge_path` :
  racine des sources, deux niveaux au-dessus de `crates/forge-runtime`) ;
  `scripts/use-forge.sh` y repointe `backend/Cargo.toml`, `app/pubspec.yaml` et
  `web/package.json` (+ `package-lock.json` : npm exige un chemin relatif identique
  dans les deux). L'interface web est construite sur `node:22-slim` (sinon Flutter).
  L'image ne copie que `Cargo.toml`, `Cargo.lock`, `crates/` et
  `packages/forge_{flutter,web}` : les membres du workspace hors `crates/` doivent être
  facultatifs (motif `examples/*/backend`).
- Variables d'environnement vides = absentes (docker-compose transmet `${X:-}`).
- `mon_app health` : `GET /health` local sans client HTTP (`HEALTHCHECK`).
- Modèles de champ (`color`, `email`, `url`, `phone`, `markdown`, `rating`,
  `percent`, `money`, `file`, `image`) : nouveaux `ColumnType` reposant sur un type
  de base (`ColumnType::base`) pour le stockage, les filtres, les formules et les
  conversions ; tout `match` du runtime sur un type stocké passe par `base()` (ou
  un bras `model => …(model.base())`). Validation propre dans `value::check`
  (couleur normalisée en minuscules). Options `max`, `currency` (+ `app.currency`),
  `max_size`, `accept`, résolues pour les interfaces par `frontend::field_options`.
  Changer `string` → `email` ne crée pas de migration (même stockage).
- `percent` stocké en proportion (`0.25`) ; les interfaces saisissent et affichent
  des %. Conversion en texte sans flottant (`shiftDecimal` côté web, `Decimal` côté Dart).
- Fichiers : stockage disque (`FORGE_UPLOAD_DIR`, volume `uploads` en Docker)
  derrière le trait `Storage` ; métadonnées dans `forge_files` (migration système
  `forge_0003_files`, appliquée même après les migrations de l'application). Un
  fichier se téléverse pour une colonne (droit de créer ou modifier la table),
  appartient à son auteur jusqu'à son rattachement (`files::attach`, dans la
  transaction de l'écriture), un seul rattachement ; remplacé ou supprimé, il est
  retiré du stockage **après** le commit (`Written::remove_file`,
  `AppState::committed`). Orphelins de plus d'un jour supprimés au téléversement.
- Lecture d'un fichier : description `{ id, name, size, content_type, url }`,
  `url` signée HMAC (secret des jetons), valable jusqu'à la fin de la fenêtre
  d'une heure suivante ; la clé de cache d'une table à fichiers contient le numéro
  de fenêtre (une lecture en cache ne sert jamais d'URL expirée). `GET
  /api/files/{id}` est hors authentification (signature vérifiée), `nosniff`,
  `attachment` sauf images. Type d'image lu dans le contenu (jamais SVG). En
  écriture, la colonne accepte l'`id` ou la description relue ; CSV : l'`id`.
- `check_rules` téléverse les fichiers au nom du rôle testé (un fichier ne se
  rattache que par son auteur) ; `TestClient` utilise un `MemoryStorage`.
- Création d'une référence depuis son champ : Flutter empile le formulaire de la
  cible (`Paths.pick`, paramètre `_pick` : retour avec l'identifiant) ; web, une
  fenêtre (`CreateRecordButton`) qui partage `FieldGrid` avec `FormPage`.
- Markdown : `flutter_markdown_plus` et `react-markdown` (dépendance du projet
  web, comme Mantine) ; liens ouverts par `url_launcher` côté Flutter. Tests web :
  jsdom n'a pas `document.fonts` (zones de texte extensibles de Mantine) : simulé
  dans `test-setup.ts`.
- Sélecteurs de date : raccourcis définis une fois par interface et identiques
  (`datePresets` côté web, `DatePreset` côté Dart) ; web = `DateInput`/
  `DateTimePicker` de Mantine (`presets`), Flutter = `showForgeDatePicker`
  (calendrier, heure et raccourcis dans une seule fenêtre). Les filtres gardent
  leurs sélecteurs d'intervalle.
- Hors périmètre v1 : temps réel, multi-tenant, workflows.
