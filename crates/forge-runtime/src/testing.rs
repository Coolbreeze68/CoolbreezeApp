//! Outils de test des applications générées (feature `testing`).
//!
//! ```ignore
//! let db = forge_runtime::testing::database::<Migrator>().await;
//! forge_runtime::testing::check_resources(app()?, db).await;
//!
//! forge_runtime::testing::check_rules(app()?, database::<Migrator>().await).await;
//!
//! // Requêtes libres, pour tester votre propre code. Le client est connecté en
//! // administrateur ; `as_new_user` en crée un autre avec les rôles voulus.
//! let admin = TestClient::new(app()?, database::<Migrator>().await).await;
//! let (status, body) = admin.request(Method::POST, "/api/tag", Some(json!({ "nom": "x" }))).await;
//! let commercial = admin.as_new_user("vente@exemple.fr", &["commercial"]).await;
//! ```
//!
//! La base de test vient de `TEST_DATABASE_URL` (SQLite en mémoire par défaut).
//! Attention : sur PostgreSQL et MySQL, toutes ses tables sont supprimées puis recréées.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};

use axum::Router;
use axum::body::Body;
use axum::http::Request;
pub use axum::http::{Method, StatusCode};
use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::{self, TypedValue};
use forge_schema::{Model, graphql};
use http_body_util::BodyExt;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde_json::{Map, Value, json};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tower::ServiceExt;

use crate::app::App;
use crate::auth::{AuthConfig, InitialAdmin};
use crate::columns;
use forge_schema::spec::Action;

/// Administrateur créé dans la base de test.
pub const ADMIN_EMAIL: &str = "admin@test.local";
/// Mot de passe de tous les comptes de test.
pub const PASSWORD: &str = "mot-de-passe-de-test";

/// Base de test prête à l'emploi.
///
/// Sur PostgreSQL ou MySQL, la base est partagée : un verrou, gardé jusqu'à la
/// fin du test (par le [`TestClient`] qui la reçoit), sérialise les tests d'un
/// même binaire. Une base SQLite en mémoire est propre à chaque test.
#[derive(Debug)]
pub struct TestDatabase {
    connection: DatabaseConnection,
    guard: Option<Arc<OwnedMutexGuard<()>>>,
}

impl TestDatabase {
    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}

static SHARED_DATABASE: LazyLock<Arc<Mutex<()>>> = LazyLock::new(Arc::default);

/// Connexion à une base de test vierge, migrations appliquées.
///
/// # Panics
///
/// Si la connexion ou les migrations échouent.
pub async fn database<M: MigratorTrait>() -> TestDatabase {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let in_memory = url.starts_with("sqlite::memory:");
    let guard = if in_memory {
        None
    } else {
        Some(Arc::new(SHARED_DATABASE.clone().lock_owned().await))
    };
    let mut options = ConnectOptions::new(url.clone());
    if url.starts_with("sqlite:") {
        // En mémoire, chaque connexion est une base distincte ; et les migrations
        // SQLite exigent une connexion unique (voir `migration`).
        options.max_connections(1);
    }
    let connection = Database::connect(options)
        .await
        .expect("connexion à la base de test");
    let migrated = if in_memory {
        M::up(&connection, None).await
    } else {
        M::fresh(&connection).await
    };
    migrated.expect("migrations de la base de test");
    TestDatabase { connection, guard }
}

/// Vérifie le cycle CRUD complet de chaque table du schéma, via l'API HTTP.
///
/// # Panics
///
/// À la première vérification en échec, avec la table et l'étape concernées.
pub async fn check_resources(app: App, db: TestDatabase) {
    let model = app.schema().clone();
    let mut checker = Checker {
        client: TestClient::new(app, db).await,
        counter: 0,
    };
    for table in model.tables() {
        checker.check_table(&model, table).await;
    }
}

/// Vérifie, pour chaque rôle (hors `admin`) et chaque table, que la lecture et
/// la création sont accordées ou refusées comme le prévoient les règles, et que
/// l'API exige d'être connecté. Les droits conditionnels (`when`) dépendent des
/// données : ils relèvent de tests écrits pour l'application.
///
/// # Panics
///
/// À la première vérification en échec, avec le rôle et la table concernés.
pub async fn check_rules(app: App, db: TestDatabase) {
    let model = app.schema().clone();
    let mut checker = Checker {
        client: TestClient::new(app, db).await,
        counter: 0,
    };
    let anonymous = checker.client.anonymous();
    for table in model.tables() {
        let (status, _) = anonymous
            .request(Method::GET, &format!("/api/{}", table.name), None)
            .await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{} : accès sans jeton",
            table.name
        );
    }

    for role in model.spec().roles.iter().filter(|r| *r != "admin") {
        let user = checker
            .client
            .as_new_user(&format!("{role}@test.local"), &[role])
            .await;
        for table in model.tables() {
            let collection = format!("/api/{}", table.name);
            let context = format!("rôle `{role}`, table `{}`", table.name);

            let (status, body) = user.request(Method::GET, &collection, None).await;
            match grant(table, role, Action::Read) {
                Grant::Denied => assert_eq!(
                    status,
                    StatusCode::FORBIDDEN,
                    "{context} : lecture : {body}"
                ),
                Grant::All | Grant::Conditional => {
                    assert_eq!(status, StatusCode::OK, "{context} : lecture : {body}");
                }
            }

            let grant = grant(table, role, Action::Create);
            if grant == Grant::Conditional {
                continue;
            }
            let sample = checker.sample(&model, table, 0).await;
            let (status, body) = user
                .request(Method::POST, &collection, Some(Value::Object(sample)))
                .await;
            let expected = if grant == Grant::All {
                StatusCode::CREATED
            } else {
                StatusCode::FORBIDDEN
            };
            assert_eq!(status, expected, "{context} : création : {body}");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grant {
    Denied,
    All,
    Conditional,
}

/// Droit d'un rôle sur une action, d'après les règles du schéma.
fn grant(table: &Table, role: &str, action: Action) -> Grant {
    let applicable: Vec<_> = table
        .rules
        .iter()
        .filter(|r| {
            r.roles.iter().any(|x| x == role)
                && r.actions.iter().any(|a| *a == action || *a == Action::All)
        })
        .collect();
    if applicable.is_empty() {
        Grant::Denied
    } else if applicable.iter().any(|r| r.when.is_none()) {
        Grant::All
    } else {
        Grant::Conditional
    }
}

/// Client HTTP en mémoire : les requêtes sont traitées par le routeur, sans
/// réseau, avec le jeton d'accès de l'utilisateur connecté.
#[derive(Debug, Clone)]
pub struct TestClient {
    router: Router,
    token: Option<String>,
    /// Verrou de la base partagée, gardé tant que le client existe.
    guard: Option<Arc<OwnedMutexGuard<()>>>,
}

impl TestClient {
    /// Démarre l'application et connecte l'administrateur de test.
    ///
    /// # Panics
    ///
    /// Si l'application ne démarre pas ou si la connexion échoue.
    pub async fn new(app: App, db: TestDatabase) -> Self {
        let mut auth = AuthConfig::new("secret-de-test");
        auth.initial_admin = Some(InitialAdmin {
            email: ADMIN_EMAIL.into(),
            password: PASSWORD.into(),
        });
        let router = app
            .into_router(db.connection, auth)
            .await
            .expect("démarrage de l'application");
        let client = Self {
            router,
            token: None,
            guard: db.guard,
        };
        client.login(ADMIN_EMAIL).await
    }

    /// Même application, sans jeton d'accès.
    #[must_use]
    pub fn anonymous(&self) -> Self {
        Self {
            router: self.router.clone(),
            token: None,
            guard: self.guard.clone(),
        }
    }

    /// Se connecte avec `email` (mot de passe [`PASSWORD`]).
    ///
    /// # Panics
    ///
    /// Si la connexion échoue.
    pub async fn login(&self, email: &str) -> Self {
        let mut client = self.anonymous();
        let credentials = json!({ "email": email, "password": PASSWORD });
        let (status, session) = client
            .request(Method::POST, "/api/auth/login", Some(credentials))
            .await;
        assert_eq!(status, StatusCode::OK, "connexion de {email} : {session}");
        client.token = session["access_token"].as_str().map(str::to_owned);
        client
    }

    /// Crée un compte avec `roles` (le client courant doit être administrateur)
    /// et retourne un client connecté avec ce compte.
    ///
    /// # Panics
    ///
    /// Si la création ou la connexion échoue.
    pub async fn as_new_user(&self, email: &str, roles: &[&str]) -> Self {
        let account = json!({ "email": email, "password": PASSWORD, "roles": roles });
        let (status, body) = self
            .request(Method::POST, "/api/users", Some(account))
            .await;
        assert_eq!(status, StatusCode::CREATED, "création de {email} : {body}");
        self.login(email).await
    }

    /// Envoie une requête JSON ; retourne le statut et le corps (`Null` si vide).
    ///
    /// # Panics
    ///
    /// Si la requête ne peut pas être construite ou traitée.
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
        let (status, bytes) = self.send(method, uri, "application/json", body).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Envoie un corps texte (un fichier CSV…) ; retourne le statut et le corps en texte.
    ///
    /// # Panics
    ///
    /// Si la requête ne peut pas être construite ou traitée.
    pub async fn request_text(
        &self,
        method: Method,
        uri: &str,
        content_type: &str,
        body: &str,
    ) -> (StatusCode, String) {
        let (status, bytes) = self
            .send(method, uri, content_type, Body::from(body.to_owned()))
            .await;
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Exécute une requête GraphQL ; retourne la réponse (`data`, `errors`).
    ///
    /// # Panics
    ///
    /// Si la requête ne peut pas être traitée.
    pub async fn graphql(&self, query: &str, variables: Value) -> Value {
        let body = json!({ "query": query, "variables": variables });
        let (status, response) = self.request(Method::POST, "/api/graphql", Some(body)).await;
        assert_eq!(status, StatusCode::OK, "GraphQL : {response}");
        response
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        content_type: &str,
        body: Body,
    ) -> (StatusCode, axum::body::Bytes) {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", content_type);
        if let Some(token) = &self.token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let request = request.body(body).expect("requête valide");
        let response = self.router.clone().oneshot(request).await.expect("réponse");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("corps de réponse")
            .to_bytes();
        (status, bytes)
    }
}

/// Vérification CRUD : génère des données de test et enchaîne les requêtes.
struct Checker {
    client: TestClient,
    counter: i64,
}

type Sample<'a> = Pin<Box<dyn Future<Output = Map<String, Value>> + 'a>>;

impl Checker {
    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.client.request(method, uri, body).await
    }

    fn next(&mut self) -> i64 {
        self.counter += 1;
        self.counter
    }

    /// Corps valide pour `table`. Les références obligatoires sont créées au besoin ;
    /// les `reference_list` ne sont remplies qu'au premier niveau (`depth == 0`).
    fn sample<'a>(&'a mut self, model: &'a Model, table: &'a Table, depth: usize) -> Sample<'a> {
        Box::pin(async move {
            let mut body = Map::new();
            for column in table.columns.iter().filter(|c| columns::is_writable(c)) {
                let n = self.next();
                let value = match column.ty {
                    ColumnType::String => json!(format!("test-{n}")),
                    ColumnType::Text => json!(format!("texte {n}")),
                    ColumnType::Integer => json!(n),
                    ColumnType::Decimal => json!(format!("{n}.25")),
                    ColumnType::Boolean => json!(n % 2 == 0),
                    ColumnType::Date => json!(format!("2026-01-{:02}", 1 + n % 28)),
                    ColumnType::Datetime => json!(format!("2026-01-15T10:{:02}:00Z", n % 60)),
                    ColumnType::Duration => json!(60 * n),
                    ColumnType::Enum => json!(column.values.as_ref().and_then(|v| v.first())),
                    ColumnType::Reference if column.required => {
                        json!(
                            self.create(model, target(model, table, &column.name), depth + 1)
                                .await
                        )
                    }
                    ColumnType::ReferenceList if depth == 0 => {
                        json!([self
                            .create(model, target(model, table, &column.name), depth + 1)
                            .await])
                    }
                    ColumnType::Reference | ColumnType::ReferenceList | ColumnType::Lookup => {
                        continue;
                    }
                };
                body.insert(column.name.clone(), value);
            }
            body
        })
    }

    /// Crée un enregistrement de test et retourne son identifiant.
    async fn create(&mut self, model: &Model, table: &Table, depth: usize) -> i64 {
        let body = self.sample(model, table, depth).await;
        let (status, record) = self
            .send(
                Method::POST,
                &format!("/api/{}", table.name),
                Some(Value::Object(body)),
            )
            .await;
        assert_eq!(
            status,
            StatusCode::CREATED,
            "{} : création de dépendance : {record}",
            table.name
        );
        record["id"].as_i64().expect("identifiant")
    }

    /// Un export CSV d'un enregistrement se réimporte tel quel (modification).
    async fn check_csv(&mut self, table: &Table, id: i64) {
        let name = &table.name;
        let (status, csv) = self
            .client
            .request_text(
                Method::GET,
                &format!("/api/{name}/export?id={id}"),
                "text/plain",
                "",
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{name} : export CSV : {csv}");
        assert_eq!(csv.lines().count(), 2, "{name} : export CSV : {csv}");
        let (status, report) = self
            .client
            .request_text(
                Method::POST,
                &format!("/api/{name}/import"),
                "text/csv",
                &csv,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{name} : import CSV : {report}");
        let report: Value = serde_json::from_str(&report).expect("bilan JSON");
        assert_eq!(
            report,
            json!({ "created": 0, "updated": 1 }),
            "{name} : import CSV"
        );
    }

    /// Lecture, liste filtrée, création et suppression par GraphQL.
    async fn check_graphql(&mut self, model: &Model, table: &Table, id: i64) {
        let name = &table.name;
        let query = format!(
            "query($id: ID!) {{ {name}(id: $id) {{ id }} {list}(filter: {{ id: {{ eq: $id }} }}) {{ total }} }}",
            list = graphql::list(name),
        );
        let response = self.client.graphql(&query, json!({ "id": id })).await;
        assert_eq!(
            response["data"][name]["id"],
            json!(id.to_string()),
            "{name} : GraphQL : {response}"
        );
        assert_eq!(
            response["data"][graphql::list(name)]["total"],
            json!(1),
            "{name} : GraphQL : {response}"
        );

        let sample = self.sample(model, table, 0).await;
        let mutation = format!(
            "mutation($data: {input}!) {{ created: {create}(data: $data) {{ id }} }}",
            input = graphql::input(name),
            create = graphql::create(name),
        );
        let response = self
            .client
            .graphql(&mutation, json!({ "data": sample }))
            .await;
        let created = &response["data"]["created"]["id"];
        assert!(
            created.is_string(),
            "{name} : création GraphQL : {response}"
        );
        let mutation = format!(
            "mutation($id: ID!) {{ deleted: {}(id: $id) }}",
            graphql::delete(name)
        );
        let response = self
            .client
            .graphql(&mutation, json!({ "id": created }))
            .await;
        assert_eq!(
            response["data"]["deleted"],
            json!(true),
            "{name} : suppression GraphQL : {response}"
        );
    }

    /// La vue `stats` de la table, si elle existe, est servie par `/aggregate`.
    async fn check_stats(&mut self, table: &Table) {
        let Some(view) = &table.views.stats else {
            return;
        };
        let name = &table.name;
        let mut uri = format!("/api/{name}/aggregate?fields={}", view.fields.join(","));
        if let Some(group_by) = &view.group_by {
            uri = format!("{uri}&group_by={group_by}");
        }
        let (status, body) = self.send(Method::GET, &uri, None).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{name} : agrégats de la vue stats : {body}"
        );
    }

    async fn check_table(&mut self, model: &Model, table: &Table) {
        let name = &table.name;
        let collection = format!("/api/{name}");

        let has_mandatory = table
            .columns
            .iter()
            .any(|c| c.required && c.default.is_none() && columns::is_writable(c));
        if has_mandatory {
            let (status, body) = self.send(Method::POST, &collection, Some(json!({}))).await;
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{name} : création vide : {body}"
            );
        }

        let payload = self.sample(model, table, 0).await;
        let (status, created) = self
            .send(
                Method::POST,
                &collection,
                Some(Value::Object(payload.clone())),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{name} : création : {created}");
        assert_matches(table, &payload, &created, "création");
        for column in table.columns.iter().filter(|c| c.is_computed()) {
            assert!(
                created.get(&column.name).is_some(),
                "{name} : colonne calculée `{}` absente",
                column.name
            );
        }
        let id = created["id"].as_i64().expect("identifiant");
        let item = format!("{collection}/{id}");

        let (status, read) = self.send(Method::GET, &item, None).await;
        assert_eq!(status, StatusCode::OK, "{name} : lecture : {read}");
        assert_eq!(read, created, "{name} : lecture différente de la création");
        self.check_csv(table, id).await;
        self.check_graphql(model, table, id).await;

        let (status, list) = self
            .send(Method::GET, &format!("{collection}?id={id}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{name} : liste filtrée : {list}");
        assert_eq!(list["total"], json!(1), "{name} : liste filtrée : {list}");
        assert_eq!(
            list["data"][0]["id"],
            json!(id),
            "{name} : liste filtrée : {list}"
        );
        let (status, list) = self
            .send(
                Method::GET,
                &format!("{collection}?sort=-created_at&per_page=1"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{name} : liste triée : {list}");

        self.check_stats(table).await;

        let changes = self.sample(model, table, 0).await;
        let (status, updated) = self
            .send(Method::PATCH, &item, Some(Value::Object(changes.clone())))
            .await;
        assert_eq!(status, StatusCode::OK, "{name} : modification : {updated}");
        assert_matches(table, &changes, &updated, "modification");
        assert_eq!(
            updated["created_at"], created["created_at"],
            "{name} : created_at modifié"
        );

        // Une cible de référence obligatoire ne peut pas être supprimée.
        for column in table
            .columns
            .iter()
            .filter(|c| c.required && c.ty == ColumnType::Reference)
        {
            let target = target(model, table, &column.name);
            let referenced = format!("/api/{}/{}", target.name, updated[&column.name]);
            let (status, body) = self.send(Method::DELETE, &referenced, None).await;
            assert_eq!(
                status,
                StatusCode::CONFLICT,
                "{name} : suppression de {referenced} référencé : {body}"
            );
        }

        let (status, body) = self.send(Method::DELETE, &item, None).await;
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "{name} : suppression : {body}"
        );
        let (status, _) = self.send(Method::GET, &item, None).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{name} : lecture après suppression"
        );
    }
}

fn target<'a>(model: &'a Model, table: &Table, column: &str) -> &'a Table {
    let relation = model
        .relations()
        .iter()
        .find(|r| r.source.table == table.name && r.source.column == column)
        .expect("relation résolue par le schéma");
    model.table(&relation.target).expect("table cible")
}

/// Compare les valeurs envoyées à celles renvoyées, au sens du type de colonne
/// (`"12.25"` et `12.2500` sont égaux, comme deux écritures d'une même date-heure).
fn assert_matches(table: &Table, sent: &Map<String, Value>, record: &Value, step: &str) {
    for (key, expected) in sent {
        let column = columns::find(table, key).expect("colonne du schéma");
        let actual = &record[key];
        let equal = if column.ty == ColumnType::ReferenceList {
            expected == actual
        } else {
            let typed = |v: &Value| -> TypedValue {
                value::from_json(column.ty, None, v)
                    .unwrap_or_else(|err| panic!("{} : {step} : `{key}` = {v} : {err}", table.name))
            };
            typed(expected) == typed(actual)
        };
        assert!(
            equal,
            "{} : {step} : `{key}` attendu {expected}, obtenu {actual}",
            table.name
        );
    }
}
