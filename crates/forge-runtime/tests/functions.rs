//! Fonctions personnalisées : déclarées dans le schéma, implémentées en Rust
//! par `App::function`, utilisées à la lecture comme dans les formules persistées.

use forge_runtime::formula::Value;
use forge_runtime::migration::Plan;
use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use forge_runtime::{App, AuthConfig, Hooks};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{big_integer_null, string_null};
use serde_json::json;

mod note {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize)]
    #[sea_orm(table_name = "note")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub titre: Option<String>,
        pub mots: Option<i64>,
        pub owner: Option<i64>,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Default)]
struct NoteHooks;

impl Hooks<note::Entity> for NoteHooks {}

struct CreateNote;

impl MigrationName for CreateNote {
    fn name(&self) -> &'static str {
        "m0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateNote {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .table("note", |t| {
                t.col(string_null("titre"));
                t.col(big_integer_null("mots"));
            })
            .apply(manager)
            .await
    }
}

struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = forge_runtime::migration::system();
        migrations.push(Box::new(CreateNote));
        migrations
    }
}

/// `INITIALES` (à la lecture) et `MOTS` (persistée), déclarées mais pas encore implémentées.
fn app() -> App {
    App::new(
        &json!({
            "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
            "roles": ["admin"],
            "functions": [
                { "name": "INITIALES", "args": ["text"], "returns": "text" },
                { "name": "MOTS", "args": ["text"], "returns": "number" }
            ],
            "tables": [{ "name": "note", "columns": [
                { "name": "titre", "type": "string" },
                { "name": "initiales", "type": "string", "formula": "INITIALES(titre)" },
                { "name": "mots", "type": "integer", "formula": "MOTS(titre)", "persist": true }
            ] }]
        })
        .to_string(),
    )
    .unwrap()
    .resource::<note::Entity, NoteHooks>()
}

fn text(args: &[Value]) -> Option<&str> {
    match args {
        [Value::Text(text)] => Some(text),
        _ => None,
    }
}

#[tokio::test]
async fn missing_or_unknown_implementations_prevent_startup() {
    let db = database::<Migrator>().await;
    let app = app()
        .function("INITIALES", |_| Ok(Value::Null))
        .function("INCONNUE", |_| Ok(Value::Null));
    let Err(error) = app
        .into_router(db.connection().clone(), AuthConfig::new("secret"))
        .await
    else {
        panic!("démarrage refusé attendu");
    };
    let message = error.to_string();
    assert!(
        message.contains("`MOTS` déclarée mais non implémentée"),
        "{message}"
    );
    assert!(message.contains("INCONNUE"), "{message}");
}

#[tokio::test]
async fn custom_functions_are_evaluated() {
    let app = app()
        .function("INITIALES", |args| {
            Ok(text(args).map_or(Value::Null, |t| {
                Value::Text(
                    t.split_whitespace()
                        .filter_map(|w| w.chars().next())
                        .collect(),
                )
            }))
        })
        .function("MOTS", |args| {
            Ok(text(args).map_or(Value::Null, |t| {
                Value::Number(t.split_whitespace().count().into())
            }))
        });
    let admin = TestClient::new(app, database::<Migrator>().await).await;

    let (status, note) = admin
        .request(
            Method::POST,
            "/api/note",
            Some(json!({ "titre": "forge en Rust" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{note}");
    assert_eq!(note["initiales"], json!("feR"));
    assert_eq!(note["mots"], json!(3));

    let (status, note) = admin
        .request(
            Method::PATCH,
            &format!("/api/note/{}", note["id"]),
            Some(json!({ "titre": "un" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{note}");
    assert_eq!(
        (note["initiales"].clone(), note["mots"].clone()),
        (json!("u"), json!(1))
    );
}
