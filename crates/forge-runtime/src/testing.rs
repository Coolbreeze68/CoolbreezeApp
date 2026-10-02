//! Outils de test des applications générées (feature `testing`).
//!
//! ```ignore
//! let db = forge_runtime::testing::database::<Migrator>().await;
//! forge_runtime::testing::check_resources(app()?, db).await;
//!
//! // Requêtes libres, pour tester votre propre code :
//! let client = TestClient::new(app()?, database::<Migrator>().await).await;
//! let (status, body) = client.request(Method::POST, "/api/tag", Some(json!({ "nom": "x" }))).await;
//! ```
//!
//! La base de test vient de `TEST_DATABASE_URL` (SQLite en mémoire par défaut).
//! Attention : sur PostgreSQL et MySQL, toutes ses tables sont supprimées puis recréées.

use std::future::Future;
use std::pin::Pin;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
pub use axum::http::{Method, StatusCode};
use forge_schema::Model;
use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::{self, TypedValue};
use http_body_util::BodyExt;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use serde_json::{Map, Value, json};
use tower::ServiceExt;

use crate::app::App;
use crate::columns;

/// Connexion à une base de test vierge, migrations appliquées.
///
/// # Panics
///
/// Si la connexion ou les migrations échouent.
pub async fn database<M: MigratorTrait>() -> DatabaseConnection {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let in_memory = url.starts_with("sqlite::memory:");
    let mut options = ConnectOptions::new(url);
    if in_memory {
        // Chaque connexion SQLite en mémoire est une base distincte.
        options.max_connections(1);
    }
    let db = Database::connect(options)
        .await
        .expect("connexion à la base de test");
    let migrated = if in_memory {
        M::up(&db, None).await
    } else {
        M::fresh(&db).await
    };
    migrated.expect("migrations de la base de test");
    db
}

/// Vérifie le cycle CRUD complet de chaque table du schéma, via l'API HTTP.
///
/// # Panics
///
/// À la première vérification en échec, avec la table et l'étape concernées.
pub async fn check_resources(app: App, db: DatabaseConnection) {
    let model = app.schema().clone();
    let mut checker = Checker {
        client: TestClient::new(app, db).await,
        counter: 0,
    };
    for table in model.tables() {
        checker.check_table(&model, table).await;
    }
}

/// Client HTTP en mémoire : les requêtes sont traitées par le routeur, sans réseau.
#[derive(Debug, Clone)]
pub struct TestClient {
    router: Router,
}

impl TestClient {
    /// # Panics
    ///
    /// Si l'application ne démarre pas.
    pub async fn new(app: App, db: DatabaseConnection) -> Self {
        Self {
            router: app
                .into_router(db)
                .await
                .expect("démarrage de l'application"),
        }
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
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .expect("requête valide");
        let response = self.router.clone().oneshot(request).await.expect("réponse");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("corps de réponse")
            .to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
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
        let id = created["id"].as_i64().expect("identifiant");
        let item = format!("{collection}/{id}");

        let (status, read) = self.send(Method::GET, &item, None).await;
        assert_eq!(status, StatusCode::OK, "{name} : lecture : {read}");
        assert_eq!(read, created, "{name} : lecture différente de la création");

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
