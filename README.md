# forge

Générateur d'applications écrit en Rust. À partir d'un fichier JSON décrivant des
tables, forge génère :

- un backend Rust (API REST + GraphQL, auth, règles d'autorisation, CSV, cache, observabilité) ;
- l'interface de votre choix : une application web React, une application Flutter
  (web, Android, iOS), ou les deux, avec tableau de bord, listes, fiches,
  formulaires, calendrier et statistiques ;
- les migrations, les tests, la documentation et la configuration Docker / CI.

Le code généré est fin : l'essentiel de la logique vit dans des bibliothèques
communes (`forge-runtime`, `@forge/web`, `forge_flutter`), et le code utilisateur (`custom/`)
n'est jamais écrasé par une régénération.

## État d'avancement

| Phase | Contenu | État |
|---|---|---|
| 0 | Workspace, `forge-schema`, parser de formules, JSON Schema, `forge validate` | ✅ |
| 1 | Backend : entités, migration initiale, CRUD REST (pagination, tri, filtres), `parameters`, hooks, routes personnalisées ; SQLite, PostgreSQL, MySQL | ✅ |
| 2 | Migrations incrémentales : diff du schéma, renommages, protection des changements destructifs, retour arrière | ✅ |
| 3 | Authentification JWT, comptes et rôles, règles d'autorisation (conditions traduites en SQL) | ✅ |
| 4 | Évaluation des formules, lookups, `persist`, fonctions personnalisées, endpoint d'agrégats | ✅ |
| 5 | GraphQL (schéma dynamique, parité avec REST, résolveurs personnalisés), import/export CSV, OpenAPI + Swagger UI | ✅ |
| 6 | Observabilité (journaux JSON, identifiant de requête, `/metrics`, `/health`), cache des lectures (mémoire ou Redis) invalidé à l'écriture | ✅ |
| 7 | Application Flutter (web et mobile) : listes, fiches, formulaires, calendrier, statistiques, CSV, comptes ; modèles Dart typés | ✅ |
| 8 | Image Docker (API et app web), docker-compose (PostgreSQL, Redis en option), GitHub Actions du projet généré | ✅ |
| 9 | Choix de l'interface (`app.frontend` : web React, Flutter ou les deux), identité visuelle « Material moderne » commune, accueil en tableau de bord | ✅ |
| 10 | Modèles de champ (couleur, e-mail, URL, téléphone, Markdown, note, pourcentage, montant, fichier, image), fichiers téléversés (stockage disque, URL signées), création d'une référence depuis son champ | ✅ |

## Installation

```bash
cargo install --path crates/forge-cli
forge --help
```

Les projets générés dépendent de `forge-runtime` et de `forge_flutter` par chemin :
le binaire `forge` pointe vers les sources à partir desquelles il a été compilé
(options `--runtime-path`, `--flutter-path` et `--web-path` pour en choisir d'autres).
L'application web demande [Node.js](https://nodejs.org) 22 ; l'application Flutter,
[Flutter](https://docs.flutter.dev/get-started/install) 3.38 ou plus récent (Dart 3.10).

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
├── Dockerfile, .dockerignore  # image : API + application web (créés une fois)
├── docker-compose.yml         # avec PostgreSQL, Redis en option (créé une fois)
├── .env.example               # secrets à recopier dans .env (créé une fois)
├── .github/workflows/ci.yml   # CI du projet (créée une fois)
├── scripts/use-forge.sh       # repointe les dépendances vers des sources de forge (Docker, CI)
├── web/                       # application web React (si `frontend` contient "web")
│   ├── package.json, vite.config.ts, index.html  # à vous (créés une fois)
│   ├── src/main.tsx           # à vous (créé une fois)
│   ├── src/generated/         # NE PAS MODIFIER : schema.ts, models.ts (types), app.tsx, tests
│   └── src/custom/            # votre code : customization.tsx
├── app/                       # application Flutter (si `frontend` contient "flutter")
│   ├── pubspec.yaml           # à vous (créé une fois)
│   ├── lib/main.dart          # à vous (créé une fois)
│   ├── lib/generated/         # NE PAS MODIFIER : réécrit à chaque génération
│   │   ├── schema.dart        #   description des tables, lue par l'interface
│   │   ├── models.dart        #   modèles typés et énumérations, pour votre code
│   │   └── app.dart           #   assemblage : buildApp()
│   ├── lib/custom/            # votre code, jamais modifié par forge
│   │   ├── customization.dart #   points d'extension de l'interface
│   │   └── theme.dart         #   thèmes clair et sombre
│   ├── web/                   # page d'accueil web (créée une fois)
│   └── test/generated_test.dart  # chaque écran s'ouvre, modèles lus et écrits (réécrit)
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
    │   ├── functions.rs       #   fonctions de formule personnalisées
    │   ├── graphql.rs         #   champs GraphQL personnalisés
    │   └── routes.rs          #   routes HTTP personnalisées
    ├── src/migrations/        # migrations (une fois créées, jamais réécrites)
    └── tests/generated_crud.rs  # test CRUD de chaque table (réécrit)
```

### 2. Démarrer l'API

```bash
cd crm/backend
export FORGE_ADMIN_EMAIL=admin@exemple.fr FORGE_ADMIN_PASSWORD='un mot de passe solide'
export FORGE_JWT_SECRET='une longue chaîne aléatoire'
cargo run                        # SQLite `data.db`, port 8080, migrations appliquées
DATABASE_URL=postgres://user:mdp@localhost/crm cargo run    # ou PostgreSQL / MySQL
```

Au premier démarrage, si la base n'a aucun utilisateur, le compte administrateur
est créé à partir de `FORGE_ADMIN_EMAIL` et `FORGE_ADMIN_PASSWORD`.

L'application expose aussi `/health` (état du serveur et de la base) et `/metrics`
(métriques Prometheus), sans authentification : voir
[Observabilité et cache](#observabilité-et-cache). La documentation de l'API est
servie par l'application : Swagger UI sur
[`/docs`](http://localhost:8080/docs) (bouton « Authorize » avec le jeton d'accès),
le document OpenAPI sur `/openapi.json`, et l'éditeur GraphQL (GraphiQL) sur
[`/graphql`](http://localhost:8080/graphql) : ajoutez l'en-tête
`{ "Authorization": "Bearer <jeton>" }` dans son panneau « Headers ».

| Variable | Défaut | Rôle |
|---|---|---|
| `DATABASE_URL` | `sqlite://data.db?mode=rwc` | Base de données |
| `FORGE_ADDR` | `0.0.0.0:8080` | Adresse d'écoute |
| `FORGE_AUTO_MIGRATE` | `true` | Migrations appliquées au démarrage |
| `FORGE_JWT_SECRET` | aléatoire | Signature des jetons ; sans elle, les sessions sont perdues à chaque redémarrage |
| `FORGE_ADMIN_EMAIL`, `FORGE_ADMIN_PASSWORD` | — | Compte administrateur initial |
| `FORGE_CACHE_TTL` | `60` | Durée de vie des lectures en cache, en secondes ; `0` désactive le cache |
| `FORGE_CACHE_URL` | — | Cache Redis partagé (`redis://hôte:6379`), feature `redis` ; sinon cache en mémoire |
| `FORGE_CORS_ORIGINS` | — | Origines autorisées à appeler l'API depuis un navigateur (virgules ; `*` pour toutes) |
| `FORGE_UPLOAD_DIR` | `uploads` | Dossier des fichiers téléversés (colonnes `file` et `image`) |
| `FORGE_STATIC_DIR` | — | Application web servie par l'API (`web/dist` après `npm run build`, ou `app/build/web` après `flutter build web`) |
| `RUST_LOG` | `info,sqlx=warn` | Niveau des journaux |
| `FORGE_LOG_FORMAT` | `text` (debug), `json` (release) | Format des journaux |

### 3. Utiliser l'API

Toute l'API exige un jeton d'accès, obtenu à la connexion :

```bash
TOKEN=$(curl -s -X POST localhost:8080/api/auth/login -H 'content-type: application/json' \
     -d '{"email": "admin@exemple.fr", "password": "un mot de passe solide"}' | jq -r .access_token)
api() { curl -s -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' "$@"; }

api -X POST localhost:8080/api/entreprise \
    -d '{"nom": "Acme", "secteur": "industrie", "chiffre_affaires": "1250000.50"}'
api -X POST localhost:8080/api/tag -d '{"nom": "urgent"}'
api -X POST localhost:8080/api/opportunite \
    -d '{"titre": "Contrat cadre", "entreprise": 1, "montant": "45000", "tags": [1]}'

api -g 'localhost:8080/api/opportunite?etape=prospect&montant[gte]=10000&sort=-montant'
api -X PATCH localhost:8080/api/opportunite/1 -d '{"etape": "gagne"}'
api -X PUT localhost:8080/api/parameters/tva -d '{"value": 5.5}'

# Un commercial, qui ne pourra modifier que ce qu'il a créé (règle `owner == $user.id`)
api -X POST localhost:8080/api/users \
    -d '{"email": "alice@exemple.fr", "password": "mot-de-passe-alice", "roles": ["commercial"]}'

# Les mêmes données en GraphQL, références résolues
api -X POST localhost:8080/api/graphql -d '{"query": "{ opportunite_list(filter: { etape: { eq: \"gagne\" } }) { total data { titre montant entreprise { nom } tags { nom } } } }"}'

# Export CSV (filtres de liste), puis réimport après modification dans un tableur
api 'localhost:8080/api/entreprise/export?secteur=industrie&delimiter=;' > entreprises.csv
curl -s -H "authorization: Bearer $TOKEN" --data-binary @entreprises.csv localhost:8080/api/entreprise/import
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

Un hook qui écrit dans une table doit le signaler, pour que les lectures en cache
qui en dépendent soient invalidées ([exemple](examples/crm/backend/src/custom/hooks/activite.rs)) :

```rust
async fn after_create(&self, ctx: &HookContext<'_>, record: &Model) -> Result<(), Error> {
    // … écriture dans `opportunite` avec ctx.db() …
    ctx.modified("opportunite");
    Ok(())
}
```

### 5. Ajouter une route ou un champ GraphQL

```rust
// backend/src/custom/routes.rs
pub fn routes() -> Router<AppState> {
    Router::new().route("/api/ping", axum::routing::get(|| async { "pong" }))
}
```

```rust
// backend/src/custom/graphql.rs
use forge_runtime::async_graphql::dynamic::{Field, FieldFuture, FieldValue, TypeRef};
use forge_runtime::{App, CurrentUser};

pub fn register(app: App) -> App {
    app.graphql_query(Field::new("bonjour", TypeRef::named_nn(TypeRef::STRING), |ctx| {
        FieldFuture::new(async move {
            let user = ctx.data::<CurrentUser>()?;
            Ok(Some(FieldValue::value(format!("Bonjour {}", user.email))))
        })
    }))
}
```

Les routes héritent de l'authentification si leur chemin commence par `/api/`.
Une route qui écrit en base appelle `state.invalidate(&["table"]).await` une fois
l'écriture validée, pour le cache.
Un résolveur GraphQL lit l'état de l'application (`ctx.data::<AppState>()`, base
par `state.db()`) et l'utilisateur connecté (`ctx.data::<CurrentUser>()`) ;
`graphql_mutation` et `graphql_type` complètent `graphql_query`.

### 6. Ajouter une fonction de formule

Déclarez la fonction dans `forge.json`, puis implémentez-la dans
`backend/src/custom/functions.rs` (créé une fois, jamais réécrit) :

```json
"functions": [{ "name": "REMISE", "args": ["number", "number"], "returns": "number" }]
```

```rust
use forge_runtime::formula::{Decimal, Value};

pub fn register(app: App) -> App {
    app.function("REMISE", |args| match args {
        [Value::Number(montant), Value::Number(taux)] => {
            Ok(Value::Number(*montant * (Decimal::ONE - *taux / Decimal::ONE_HUNDRED)))
        }
        _ => Ok(Value::Null),
    })
}
```

La validation contrôle les appels (nombre et types des arguments) ; l'application
refuse de démarrer si une fonction déclarée n'est pas implémentée. Une erreur
renvoyée (`Err("message")`) refuse l'écriture (`422`) pour une formule persistée,
et donne `null` (journalisé) pour une formule calculée à la lecture.

### 7. Tester

```bash
cargo test                                            # SQLite en mémoire
TEST_DATABASE_URL=postgres://… cargo test             # base recréée : base de test dédiée !
```

`tests/generated_crud.rs` vérifie création, lecture, liste, filtre, modification,
suppression et contraintes de chaque table, l'aller-retour export → import CSV,
la lecture, la création et la suppression en GraphQL, puis les droits de lecture
et de création de chaque rôle. Pour vos propres tests, `forge_runtime::testing::TestClient`
envoie des requêtes à l'application sans réseau, connecté en administrateur ;
`as_new_user` crée un compte avec les rôles voulus. Exemples :
[`tests/hooks.rs`](examples/crm/backend/tests/hooks.rs) et
[`tests/rules.rs`](examples/crm/backend/tests/rules.rs) (règle `owner == $user.id`),
[`tests/formulas.rs`](examples/crm/backend/tests/formulas.rs) (formules et agrégats),
[`tests/graphql.rs`](examples/crm/backend/tests/graphql.rs) et
[`tests/csv.rs`](examples/crm/backend/tests/csv.rs),
[`tests/observability.rs`](examples/crm/backend/tests/observability.rs) (santé,
métriques, cache) ; `TestClient` offre aussi `graphql(requête, variables)`,
`request_text` (corps CSV) et `request_headers`. Avec la feature `redis` de
`forge-runtime` et `TEST_CACHE_URL=redis://…`, les tests utilisent un cache Redis.

### 8. Choisir et lancer l'interface

L'interface se choisit dans `forge.json` ; `forge generate` crée le dossier
correspondant (`web/` ou `app/`) :

```json
"app": { "name": "crm", "default_locale": "fr", "locales": ["fr", "en"], "frontend": "web" }
```

| `frontend` | Interface générée |
|---|---|
| `"flutter"` (défaut) | `app/` : application Flutter, une seule base de code pour le web, Android et iOS |
| `"web"` | `web/` : application web React + TypeScript (Vite, composants [Mantine](https://mantine.dev)) |
| `["flutter", "web"]` | les deux (le CRM d'exemple) ; l'image Docker sert l'application web React |

Les deux interfaces offrent les mêmes écrans, avec la même identité visuelle
(indigo et violet, dégradés, couleurs des valeurs d'énumération communes), en
thème clair ou sombre :

- **connexion**, session renouvelée automatiquement ; langue au choix parmi `locales` ;
- **tableau de bord** d'accueil : nombre d'enregistrements par table, répartition
  de chaque vue `stats`, prochaines échéances de chaque vue `calendar` ;
- **listes** paginées : recherche, tri par colonne, filtres par type (valeurs
  d'énumération, intervalles de nombres et de dates, référence, texte contenu) ;
- **fiches** : valeurs mises en forme, valeurs d'énumération en pastilles de
  couleur, références cliquables, formules et lookups, et les enregistrements des
  autres tables qui la référencent (« Tout voir », création pré-remplie) ;
- **formulaires** de création et de modification : un champ par type (dates et
  dates-heures avec calendrier, heure et raccourcis « Aujourd'hui », « Demain »,
  « Dans une semaine » ou « Maintenant », « Dans une heure », « Demain matin »,
  durées `h:mm`, énumérations, références avec recherche, listes de références),
  valeurs par défaut, erreurs de validation du serveur sous chaque champ ; seules
  les colonnes modifiées sont envoyées. Une référence se crée aussi depuis son
  champ (bouton « + ») : formulaire de la table cible, puis retour au formulaire
  en cours avec l'enregistrement choisi ;
- **modèles de champ** : sélecteur de couleur (nuancier et code hexadécimal),
  liens cliquables (e-mail, site, téléphone), étoiles, pourcentage et montant
  saisis et affichés dans la langue de l'utilisateur, Markdown avec aperçu,
  fichiers et images envoyés dès leur choix (miniatures dans les listes,
  agrandissement au clic) ;
- **calendrier** (`views.calendar`) : mois et agenda du jour, création à une date ;
- **statistiques** (`views.stats`) : totaux, moyennes, extrêmes et barres par groupe
  (couleur de chaque valeur, ordre de déclaration), sur les enregistrements filtrés ;
- **export et import CSV** de la liste filtrée ;
- pour les administrateurs : **paramètres** et **comptes** (rôles, activation) ;
  pour tous : changement de mot de passe.

Les boutons suivent les règles (`rules`) : une action qu'aucun rôle de
l'utilisateur ne permet n'est pas proposée ; une condition `when` est vérifiée par
le serveur. Les colonnes `hidden` n'apparaissent pas dans l'interface.

| Tableau de bord (web) | Statistiques (web) |
|---|---|
| ![Tableau de bord](docs/images/web-accueil.png) | ![Statistiques des opportunités](docs/images/web-statistiques.png) |
| **Fiche (web)** | **Calendrier (web)** |
| ![Fiche entreprise](docs/images/web-fiche.png) | ![Calendrier des opportunités](docs/images/web-calendrier.png) |
| **Thème sombre (web)** | **Tableau de bord (Flutter)** |
| ![Liste en thème sombre](docs/images/web-sombre.png) | ![Tableau de bord Flutter](docs/images/flutter-accueil.png) |
| **Téléphone (web)** | **Téléphone (Flutter, thème sombre)** |
| ![Liste sur téléphone](docs/images/web-mobile.png) | ![Accueil Flutter sur téléphone](docs/images/flutter-mobile.png) |
| **Modèles de champ : fiche (web)** | **Modèles de champ : formulaire (web)** |
| ![Logo, site, note, montant, Markdown](docs/images/web-modeles-fiche.png) | ![Image, note, montant, aperçu Markdown](docs/images/web-modeles-formulaire.png) |
| **Création d'une référence depuis son champ (web)** | **Modèles de champ : fiche (Flutter)** |
| ![Nouvelle entreprise depuis une opportunité](docs/images/web-creation-reference.png) | ![Fiche entreprise Flutter](docs/images/flutter-modeles-fiche.png) |

#### Application web (React)

```bash
cd crm/backend && cargo run          # API sur le port 8080
cd crm/web && npm install && npm run dev   # http://localhost:5173
```

En développement, Vite relaie `/api` vers le backend (`FORGE_API_URL`, défaut
`http://localhost:8080`) : même origine pour le navigateur, pas de CORS.
`npm run build` produit `dist/`, que le backend sert lui-même avec
`FORGE_STATIC_DIR=web/dist` (c'est ce que fait l'image Docker). La session est
conservée dans le `localStorage` du navigateur.

#### Application Flutter

```bash
cd crm/backend && FORGE_CORS_ORIGINS=http://localhost:5000 cargo run   # API, ouverte à l'app web
cd crm/app
flutter run -d chrome --web-port 5000 --dart-define=FORGE_API_URL=http://localhost:8080
```

La session est conservée dans le stockage chiffré du système. Sur le web, ce
stockage exige HTTPS ou `localhost` : ouverte en `http` par une autre adresse
(IP d'un serveur), l'application fonctionne mais il faut se reconnecter à chaque
rechargement de la page. L'adresse de l'API vient de
`--dart-define=FORGE_API_URL=…` ; sans elle, la version web appelle l'origine qui
la sert. `FORGE_CORS_ORIGINS` (origines séparées par des virgules, ou `*`)
autorise un navigateur à appeler l'API depuis une autre origine. Pour Android et
iOS, ajoutez une fois les dossiers de plateforme avec
`flutter create --platforms=android,ios .` dans `app/` (ils vous appartiennent
ensuite).

#### Personnaliser l'interface

`src/custom/customization.tsx` (web) et `lib/custom/customization.dart` (Flutter)
ne sont jamais réécrits. `ForgeCustomization` y déclare les mêmes points
d'extension : thème, icônes des tables, libellés des valeurs d'énumération,
champs et affichages par colonne, blocs des fiches, pages ajoutées au menu,
textes. Côté web ([exemple du CRM](examples/crm/web/src/custom/customization.tsx)) :

```tsx
export const customization: ForgeCustomization = {
  theme: { primaryColor: 'indigo' },          // thème Mantine
  tableIcons: { entreprise: IconBuilding },
  enumLabels: { 'opportunite.etape': { gagne: { fr: 'Gagnée', en: 'Won' } } },
  fields: { 'tag.couleur': ColorField },      // composants React (FieldProps)
  cells: { 'tag.couleur': ColorDot },         // composants React (CellProps)
  detailSections: { entreprise: [EntrepriseSummary] },
  pages: [{ path: 'accueil', label: 'Accueil', icon: IconHome, component: Accueil }],
};
```

`useForge()` donne accès, dans tout composant, au client de l'API, au schéma, à
l'utilisateur connecté et à la mise en forme des valeurs. `src/generated/models.ts`
déclare le type de chaque enregistrement (tel que l'API l'envoie) et un client typé :

```ts
const gagnees = await opportuniteApi(forge.client).listAll({ filters: [equals('etape', 'gagne')] });
```

Côté Flutter ([exemple du CRM](examples/crm/app/lib/custom/customization.dart)) :

```dart
final customization = ForgeCustomization(
  theme: lightTheme,
  darkTheme: darkTheme,
  tableIcons: const {'entreprise': Icons.business},
  enumLabels: const {
    'opportunite.etape': {'gagne': Label({'fr': 'Gagnée', 'en': 'Won'})},
  },
  // Champ de formulaire ou affichage d'une colonne remplacés (vos widgets).
  fields: {'tag.couleur': (context, field) => ColorField(field)},
  cells: {'tag.couleur': (context, record, value) => ColorDot(value)},
  // Blocs ajoutés à une fiche, pages ajoutées au menu.
  detailSections: {'entreprise': [(context, record) => EntrepriseSummary(id: record['id'])]},
  pages: [CustomPage(path: 'accueil', label: Label.plain('Accueil'), icon: Icons.home, builder: …)],
  // Textes de l'interface dans une autre langue que fr et en.
  strings: {'de': ForgeStrings(…)},
);
```

`Forge.of(context)` donne accès, dans tout widget, au client de l'API, au schéma,
à l'utilisateur connecté et à la mise en forme des valeurs ; `context.go(Paths.record('contact', 3))`
navigue. Les modèles typés de `lib/generated/models.dart` simplifient l'accès à
l'API depuis votre code :

```dart
final gagnees = await Opportunite.api(client).list(
  const ListQuery(filters: [Filter.equals('etape', 'gagne')]),
);
final total = gagnees.items.fold(Decimal.zero, (sum, o) => sum + o.montant);
```

#### Tester l'interface

```bash
cd crm/web && npm test && npm run typecheck   # web
cd crm/app && flutter test                     # Flutter
```

Les tests générés ouvrent la liste et le formulaire de chaque table ; côté
Flutter, ils vérifient aussi que chaque modèle relit et réécrit un
enregistrement (côté web, `npm run typecheck` contrôle les types). Pour vos
tests, `@forge/web/testing` et `package:forge_flutter/testing.dart` fournissent
`FakeApi`, une API en mémoire (routes déclarées par le test, compte connecté). Le CRM contient aussi un parcours
contre un backend réel, [`test/api_test.dart`](examples/crm/app/test/api_test.dart),
lancé avec `FORGE_E2E_URL=http://localhost:8080 flutter test test/api_test.dart`.

### 9. Déployer avec Docker

L'image contient l'API et l'application web, qu'elle sert elle-même
(`FORGE_STATIC_DIR`) : une seule origine, donc pas de CORS. Le projet dépendant
des sources de forge, celles-ci sont passées à la construction comme contexte
nommé `forge` (chemin dans `FORGE_SOURCES`, préréglé par `forge new`).

```bash
cd crm
cp .env.example .env          # renseignez FORGE_JWT_SECRET, POSTGRES_PASSWORD, compte admin
docker compose up --build     # http://localhost:8080 : application, API, /docs, /graphql
docker compose --profile redis up --build   # avec le cache Redis (voir .env.example)
```

| Fichier | Contenu |
|---|---|
| `Dockerfile` | trois étapes : l'interface web (`npm run build` sur `node:22-slim` si `web` est dans `app.frontend`, sinon `flutter build web`, SDK épinglé par `FLUTTER_VERSION`), `cargo build --release` (features en option : `CARGO_FEATURES`), image finale Debian slim, utilisateur non root, données dans `/data` |
| `docker-compose.yml` | l'application (fichiers téléversés dans le volume `uploads`), PostgreSQL 17 (volume `db`, démarrage attendu), Redis dans le profil `redis` |
| `.env.example` | secrets et réglages ; `.env` n'est ni versionné ni copié dans l'image |

Sans docker-compose, l'image démarre seule sur SQLite (`/data/data.db`) :

```bash
docker build --build-context forge=../forge -t crm .
docker run -p 8080:8080 -v crm-data:/data -e FORGE_JWT_SECRET=… -e FORGE_ADMIN_EMAIL=… -e FORGE_ADMIN_PASSWORD=… crm
```

Le binaire a une sous-commande `health` (`mini_crm health`), utilisée par le
`HEALTHCHECK` de l'image : elle interroge `/health` sur le port de `FORGE_ADDR`.

#### Intégration continue

`.github/workflows/ci.yml` (créé une fois) teste à chaque push et pull request :
le backend (`cargo fmt`, `clippy`, tests sur SQLite puis PostgreSQL),
chaque interface (web : `tsc`, tests, `npm run build` ; Flutter : `flutter analyze`,
tests, compilation web), puis construit l'image
et vérifie qu'elle démarre, sert l'application et accepte une connexion. Aucune
image n'est publiée. Le workflow récupère les sources de forge depuis le dépôt
indiqué par la variable `FORGE_REPOSITORY` du dépôt GitHub (`FORGE_REF` pour une
branche ou un tag, secret `FORGE_TOKEN` si ce dépôt est privé).

### 10. Faire évoluer le schéma

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
| `GET` | `/api/<table>/aggregate` | Agrégats (voir plus bas) |
| `GET` | `/api/<table>/export` | Export CSV (voir plus bas) |
| `POST` | `/api/<table>/import` | Import CSV, tout ou rien |
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
filtrables et triables. Formats JSON : décimaux en texte sans zéros superflus
(`"12.5"`, nombres acceptés en entrée), dates `AAAA-MM-JJ`, dates-heures RFC 3339, durées en secondes,
`reference` = identifiant, `reference_list` = liste d'identifiants.

Colonnes calculées : les formules non persistées et les lookups sont évalués à
chaque lecture ; les formules persistées sont stockées et recalculées dans la
transaction de l'écriture qui les affecte, y compris dans les autres tables
(`entreprise.pipeline` quand une opportunité change, est déplacée ou supprimée) et
quand un paramètre qu'elles lisent est modifié. Elles sont donc filtrables et
triables, et utilisables dans les agrégats.

Agrégats : `GET /api/opportunite/aggregate?fields=montant,montant_pondere&group_by=etape`
renvoie `count` et, pour chaque colonne de `fields` (numérique, stockée),
`sum`, `avg`, `min`, `max` (décimaux en texte), par groupe et au total :

```json
{ "fields": ["montant"], "group_by": "etape",
  "groups": [{ "key": "gagne", "count": 2, "montant": { "sum": "4000", "avg": "2000", "min": "1000", "max": "3000" } }],
  "total": { "count": 4, "montant": { … } } }
```

`group_by` est facultatif (colonne stockée, sauf `text`). Les filtres et la
recherche de liste s'appliquent, ainsi que le périmètre de lecture de l'utilisateur.

Codes d'erreur : `400` requête mal formée, `401` non authentifié, `403` non autorisé,
`404` introuvable, `409` conflit (valeur
unique déjà prise, référence invalide, suppression d'un enregistrement référencé par
une référence obligatoire), `422` validation (détail par champ).

Suppression : une référence obligatoire bloque la suppression de sa cible, une
référence facultative est remise à `null`, les liens `reference_list` sont supprimés.

### Import et export CSV

`GET /api/<table>/export` renvoie un fichier CSV : `id`, toutes les colonnes du
schéma (calculées comprises), `owner`, `created_at`, `updated_at`. Les filtres, la
recherche et le tri de liste s'appliquent (sans pagination), ainsi que le périmètre
de lecture ; `delimiter=;` produit un fichier pour Excel en français. Une cellule
vide vaut `null`, une `reference_list` s'écrit `1,2,3`.

`POST /api/<table>/import` reçoit le fichier dans le corps de la requête :

- la première ligne nomme les colonnes ; le séparateur (`,` ou `;`) est détecté ;
- une ligne avec un `id` modifie l'enregistrement, une ligne sans `id` le crée (ses
  cellules vides prennent la valeur par défaut) ;
- les colonnes calculées et système sont ignorées : un export se réimporte tel quel ;
- chaque ligne passe par les mêmes validations, règles d'autorisation et hooks
  qu'une requête REST ;
- **tout ou rien** : à la moindre erreur, rien n'est enregistré et la réponse `422`
  liste les erreurs de chaque ligne (numéro de ligne du fichier, en-tête = 1) :

```json
{ "error": { "code": "import", "message": "import refusé : 2 ligne(s) en erreur, aucune donnée enregistrée",
  "lines": [ { "line": 3, "code": "validation", "message": "données invalides",
               "fields": { "montant": ["valeur \"abc\" invalide : attendu un nombre"] } },
             { "line": 4, "code": "conflict", "message": "référence invalide, …" } ] } }
```

En cas de succès : `{ "created": 12, "updated": 3 }`. Le corps est limité à 2 Mo
(limite par défaut d'axum). Un fichier (`file`, `image`) s'exporte par son
identifiant, qui se réimporte tel quel.

### Fichiers et images

Une colonne `file` ou `image` reçoit un fichier téléversé au préalable :

```bash
curl -X POST "localhost:8080/api/files?table=entreprise&column=logo&name=logo.png" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: image/png" --data-binary @logo.png
# { "id": "1d82c277-…", "name": "logo.png", "size": 2048, "content_type": "image/png", "url": "/api/files/1d82c277-…?expires=…&signature=…" }
curl -X PATCH localhost:8080/api/entreprise/1 -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" -d '{"logo": "1d82c277-…"}'
```

- Le téléversement exige le droit de créer ou de modifier dans la table ; la
  taille (`max_size`, 10 Mo par défaut) et le type (`accept`) sont vérifiés. Une
  image est reconnue à son contenu (PNG, JPEG, GIF, WebP), jamais à l'en-tête
  annoncé : un SVG ou du HTML déguisé est refusé.
- Écrire l'`id` dans la colonne rattache le fichier à l'enregistrement ; seul
  l'auteur du téléversement (ou un administrateur) peut le faire, une seule fois.
  Un fichier jamais rattaché est supprimé au bout d'un jour.
- À la lecture, la colonne contient la description du fichier (ci-dessus) ; `url`
  est un lien signé et temporaire (une à deux heures), utilisable sans jeton (balise
  `<img>`, téléchargement) : il hérite des règles de lecture de la table.
- Remplacé, vidé, ou supprimé avec son enregistrement, le fichier est retiré du
  stockage une fois la transaction validée.
- Stockage : dossier `FORGE_UPLOAD_DIR` (`uploads` par défaut, volume `uploads` en
  Docker), derrière le trait `Storage` (`App::storage`) pour brancher un autre
  stockage (S3…).

## Observabilité et cache

### Journaux et identifiant de requête

Les journaux passent par `tracing` : texte lisible en développement, une ligne
JSON par événement en production (`FORGE_LOG_FORMAT`). Chaque requête reçoit un
identifiant `x-request-id` (repris s'il est fourni par le client ou le proxy),
renvoyé dans la réponse et présent dans tous ses journaux :

```json
{"timestamp":"…","level":"INFO","message":"finished processing request","latency":"3 ms","status":200,
 "span":{"method":"GET","path":"/api/tag","request_id":"5c47fd1e-…","name":"requête"}}
```

Les erreurs internes sont journalisées avec leur détail, qui n'est jamais renvoyé
au client.

### Santé et métriques

| Chemin | Contenu |
|---|---|
| `GET /health` | `200 {"status": "ok", "database": "ok"}`, ou `503` si la base ne répond pas (2 s) |
| `GET /metrics` | Métriques Prometheus |

| Métrique | Libellés |
|---|---|
| `http_requests_total` | `method`, `path` (modèle de route : `/api/opportunite/{id}`), `status` |
| `http_request_duration_seconds` (histogramme) | `method`, `path` |
| `forge_cache_requests_total` | `table`, `result` (`hit`, `miss`) |

Ces deux routes ne demandent pas d'authentification : restreignez `/metrics` au
réseau interne au niveau du proxy.

### Cache des lectures

Les listes, lectures et agrégats (REST et GraphQL) sont mis en cache ; l'export
CSV et les références chargées par GraphQL lisent toujours la base.

- La clé d'une lecture contient sa requête complète et le périmètre de
  l'utilisateur (règles) : deux utilisateurs ne partagent une entrée que s'ils
  voient exactement les mêmes enregistrements.
- Chaque table a une version, incrémentée après chaque écriture validée. La clé
  contient les versions des tables dont la lecture dépend : la table elle-même,
  les cibles de ses relations, les tables lues par ses lookups et formules
  calculées à la lecture, les paramètres et les comptes (`owner`). Une écriture
  invalide donc exactement les lectures concernées, y compris les formules
  persistées recalculées dans d'autres tables. Une table avec des formules
  calculées à la lecture est aussi relue chaque jour (`TODAY`).
- Une écriture faite hors de forge doit être signalée : `ctx.modified("table")`
  dans un hook, `state.invalidate(&["table"])` dans une route personnalisée.
  Une écriture directe en base (autre application, SQL manuel) n'est vue qu'à
  l'expiration des entrées (`FORGE_CACHE_TTL`).

Par défaut, le cache est en mémoire (10 000 entrées, 60 s), propre à chaque
instance. Pour plusieurs instances, activez Redis, partagé par toutes :

```toml
# backend/Cargo.toml
forge-runtime = { path = "…", features = ["redis"] }
```

```bash
FORGE_CACHE_URL=redis://localhost:6379 cargo run
```

Une panne de Redis ne fait pas échouer les requêtes : elle est journalisée et les
lectures se font en base. Dans le code, `App::cache(...)` accepte toute
implémentation du trait `forge_runtime::cache::Cache`, et `App::without_cache()`
désactive le cache.

## API GraphQL

`POST /api/graphql` (authentifié comme le reste de `/api/`) ; éditeur interactif sur
`/graphql`. Le schéma est construit au démarrage à partir de `forge.json` et donne
accès aux mêmes opérations que l'API REST, avec les mêmes validations, règles et
hooks. Pour la table `opportunite` :

| Opération | Effet |
|---|---|
| `opportunite(id: ID!): Opportunite` | Lecture (`null` si introuvable ou hors périmètre) |
| `opportunite_list(page, per_page, sort, q, filter): OpportunitePage!` | Liste : `data`, `page`, `per_page`, `total` |
| `opportunite_aggregate(fields, group_by, q, filter): JSON!` | Agrégats, comme `/aggregate` |
| `create_opportunite(data: OpportuniteInput!)` | Création |
| `update_opportunite(id: ID!, data: OpportuniteInput!)` | Modification : seuls les champs fournis changent |
| `delete_opportunite(id: ID!): Boolean!` | Suppression |
| `parameters: JSON!`, `update_parameter(name, value)` | Paramètres |

```graphql
{
  opportunite_list(sort: "-montant", filter: { montant: { gte: "1000" }, etape: { in: ["gagne", "proposition"] } }) {
    total
    data { titre montant montant_ttc etape entreprise { nom secteur } tags { nom } }
  }
}
```

- Types : `ID` (identifiants), `Int`, `Boolean`, `String`, et les scalaires `Decimal`
  (texte), `Date`, `DateTime`, `JSON` ; une colonne `enum` devient une énumération
  GraphQL (`OpportuniteEtape`).
- Une `reference` se lit comme l'enregistrement cible, une `reference_list` comme
  la liste des cibles ; elles sont chargées par lots, avec les droits de
  l'utilisateur (une cible hors de son périmètre vaut `null`).
- Filtres : `{ colonne: { eq, ne, lt, lte, gt, gte, like, in, is_null } }`, sur les
  colonnes stockées.
- Erreurs : `extensions.code` (`validation`, `forbidden`, `conflict`…) et, pour une
  validation, `extensions.fields` (détail par champ, comme en REST).
- L'authentification et la gestion des comptes restent en REST.
### Authentification et comptes

| Méthode | Chemin | Effet |
|---|---|---|
| `POST` | `/api/auth/login` | `{ email, password }` → `{ access_token, refresh_token, expires_in, user }` |
| `POST` | `/api/auth/refresh` | `{ refresh_token }` → nouvelle session ; l'ancien jeton est révoqué |
| `POST` | `/api/auth/logout` | `{ refresh_token }` → révocation |
| `GET` | `/api/auth/me` | Utilisateur connecté (id, email, nom, rôles) |
| `PUT` | `/api/auth/password` | `{ current_password, new_password }` ; ferme les autres sessions |
| `GET`/`POST` | `/api/users` | Comptes (admin) : liste (`page`, `per_page`, `q`), création |
| `GET`/`PATCH`/`DELETE` | `/api/users/{id}` | Compte (admin) : `email`, `password`, `display_name`, `active`, `roles` |

- Jeton d'accès JWT de 15 minutes (`Authorization: Bearer …`), jeton de
  rafraîchissement opaque de 30 jours, stocké haché et remplacé à chaque usage.
- Mots de passe hachés avec argon2id, 8 caractères minimum.
- Désactiver un compte ou changer son mot de passe ferme ses sessions ; un jeton
  d'accès déjà émis reste valable jusqu'à son expiration (15 minutes au plus).
- Un administrateur ne peut ni supprimer son propre compte, ni se retirer le rôle
  `admin`, ni se désactiver.
- Lecture des paramètres : tout utilisateur connecté ; modification : `admin`.
- Le nombre de tentatives de connexion n'est pas limité par forge : à confier au
  proxy placé devant le serveur.

### Autorisations à l'exécution

Évaluées à chaque requête, pour la table et l'action concernées :

- le rôle `admin` a tous les droits ;
- sinon, il faut une règle listant l'un des rôles de l'utilisateur et l'action
  (`read`, `create`, `update`, `delete` ou `*`), faute de quoi la requête est
  refusée (`403`) ;
- une règle sans `when` donne accès à tous les enregistrements ; avec `when`, seuls
  ceux qui vérifient la condition (plusieurs règles s'additionnent).

Les conditions sont traduites en SQL : elles filtrent les listes, et un
enregistrement hors du périmètre de lecture répond `404`. Modifier ou supprimer
exige que l'enregistrement vérifie la condition de l'action, avant et après la
modification ; créer, que le nouvel enregistrement la vérifie. `owner` reçoit
l'identifiant du créateur, ce qui rend possible la règle `owner == $user.id`.

Codes : `401` sans jeton ou jeton invalide, `403` action non autorisée.

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
| `app` | `name` (identifiant), `default_locale`, `locales` (ex. `["fr", "en"]`), `frontend` (`"flutter"` par défaut, `"web"`, ou `["flutter", "web"]`), `currency` (devise par défaut des colonnes `money`, code ISO 4217, `EUR` par défaut). |
| `roles` | Rôles utilisateurs. `admin` est obligatoire. |
| `parameters` | Paramètres globaux typés (`name`, `type`, `default`, `label`), lisibles dans les formules via `$param.nom`. |
| `functions` | Fonctions de formule implémentées en Rust : `name`, `args` et `returns` (`number`, `text`, `boolean`, `date`, `datetime`, `any`), `volatile` (résultat non déterministe : interdite dans `persist`). |
| `tables` | Tables métier. |

Les identifiants (tables, colonnes, rôles, valeurs d'enum) sont en `snake_case`,
63 caractères au plus, et ne doivent pas être des mots réservés de Rust ou de Dart.

### Tables

| Clé | Description |
|---|---|
| `name`, `label` | Nom technique et libellé (texte ou `{ "fr": …, "en": … }`). Les noms GraphQL qui en dérivent ne doivent pas entrer en conflit (`a_b` et l'énumération `b` de `a` donnent tous deux `AB`). |
| `columns` | Colonnes métier. |
| `views` | `calendar` : `{ start, end? \| duration? }` ; `stats` : `{ fields, group_by? }`. |
| `rules` | Règles d'autorisation (voir plus bas). Sans règle, seul `admin` accède à la table. |

Ajoutées automatiquement à chaque table : `id`, `created_at`, `updated_at`, `owner`
(créateur de l'enregistrement).
Tables système réservées : `users`, `roles`, `user_roles`, `parameters`, `refresh_tokens` ;
noms réservés par l'API : `auth`, `graphql`.

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

Les **modèles de champ** reposent sur un type de base (stockage, filtres, type
dans les formules), avec une validation et une présentation propres :

| Modèle | Base | Valeur et validation | Interface | Options propres |
|---|---|---|---|---|
| `color` | `string` | `#rrggbb` (mis en minuscules) | sélecteur de couleur, pastille | |
| `email` | `string` | adresse e-mail | lien `mailto:` | |
| `url` | `string` | adresse `http://` ou `https://` | lien ouvert dans un nouvel onglet | |
| `phone` | `string` | 6 à 15 chiffres, `+`, espaces, `.`, `-`, parenthèses | lien `tel:` | |
| `markdown` | `text` | texte Markdown | éditeur avec aperçu, rendu dans la fiche | |
| `rating` | `integer` | note de 0 à `max` | étoiles | `max` (1 à 10, 5 par défaut) |
| `percent` | `decimal` | proportion : `0.25` pour 25 % | saisie et affichage en % | |
| `money` | `decimal` | montant | saisie et affichage dans la devise | `currency` (ISO 4217, défaut `app.currency`) |
| `file` | — | fichier téléversé (voir [Fichiers](#fichiers-et-images)) | choix, lien de téléchargement | `max_size` (Mo, 10 par défaut), `accept` (`application/pdf`, `image/*`, `.docx`…) |
| `image` | — | image PNG, JPEG, GIF ou WebP | miniature, agrandissement | `max_size` |

Un modèle numérique ou texte peut être calculé (`"type": "money", "formula": …`),
servir de paramètre, de valeur par défaut ou de vue `stats` (`rating`, `percent`,
`money`) ; `rating` peut aussi regrouper une vue `stats`. Changer une colonne
`string` en `email` (ou `decimal` en `money`…) ne crée pas de migration : le
stockage est le même, la validation s'applique aux écritures suivantes. Les
fichiers ne sont ni uniques, ni intitulés, ni calculés, ni triables.

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
- Fonctions : `IF`, `ROUND`, `CONCAT`, `DAYS_BETWEEN(début, fin)`, `TODAY`, et celles de `functions`.
- Agrégats sur une relation « plusieurs » (inverse ou `reference_list`) : `SUM`, `AVG`, `MIN`, `MAX`, `COUNT`.

La validation vérifie la syntaxe, l'existence des colonnes et des relations,
l'usage des agrégats, les types (le résultat d'une formule doit correspondre au type
de sa colonne, les arguments à la signature des fonctions) et détecte les
dépendances circulaires (y compris entre tables).

À l'évaluation, `NULL` se propage (`NULL + 1` vaut `NULL`, `AND`/`OR` suivent la
logique à trois valeurs), une division par zéro donne `NULL`, les agrégats ignorent
les `NULL` (`SUM` d'un ensemble vide vaut 0). Une date plus ou moins un nombre
décale de ce nombre de jours. Les résultats sont arrondis à 4 décimales pour un
`decimal`, à l'entier pour un `integer`.

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
├── forge-formula/  langage de formules : lexer, parser, AST, typage, évaluation, registre de fonctions
├── forge-codegen/  génération : structure de stockage, diff et migrations, templates, écriture idempotente
├── forge-runtime/  logique des apps générées : CRUD générique (REST, GraphQL, CSV), OpenAPI, filtres,
│                   agrégats, calcul des formules, auth, hooks, fichiers, cache, observabilité,
│                   migrations, CLI
└── forge-cli/      binaire `forge`
packages/
├── forge_web/      logique de l'app web (React, TypeScript, Mantine) : client de l'API, tableau
│                   de bord, listes, fiches, formulaires, filtres, calendrier, statistiques, CSV, comptes
└── forge_flutter/  la même chose pour l'app Flutter
templates/          templates minijinja (backend/, web/, flutter/, infra/), embarqués dans le binaire
examples/crm/       projet de référence généré (membre du workspace, testé en CI)
scripts/            scénario de bout en bout (évolution du schéma)
```

Le backend généré embarque son schéma (`src/generated/forge.json`) : `forge-runtime`
le relit au démarrage et en tire tout le comportement générique (validation des
corps, filtres, valeurs par défaut, liens N↔N, paramètres, formules, schéma GraphQL,
document OpenAPI). Le code généré se limite aux entités sea-orm typées, à la
migration et au branchement des hooks.

De même, les interfaces générées contiennent la description des tables
(`src/generated/schema.ts`, `lib/generated/schema.dart`) et les types des
enregistrements ; tous les écrans vivent dans `@forge/web` et `forge_flutter`, qui
se mettent à jour sans régénération. Le code Dart généré est déjà formaté comme le
produirait `dart format`.

## Développement

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features                              # inclut le CRUD du CRM sur SQLite
TEST_DATABASE_URL=postgres://… cargo test -p mini_crm  # idem sur PostgreSQL ou MySQL
cargo run -p forge-cli -- schema > forge.schema.json   # après modification de spec.rs
cargo run -p forge-cli -- generate --dir examples/crm  # après modification des templates ou du codegen
(cd packages/forge_web && npm ci && npm run lint && npm run typecheck && npm test)
(cd examples/crm/web && npm ci && npm run typecheck && npm test)   # app web générée du CRM
(cd packages/forge_flutter && flutter analyze && flutter test)
(cd examples/crm/app && flutter analyze && flutter test)  # app générée du CRM
./scripts/e2e-evolution.sh                             # scénario complet : création, données, évolution, migration
DATABASE_URL=postgres://…/vide ./scripts/e2e-evolution.sh   # idem sur une base PostgreSQL ou MySQL vide
```
