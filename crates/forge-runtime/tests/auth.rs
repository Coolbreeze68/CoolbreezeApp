//! Authentification et gestion des comptes, sur une application minimale
//! (une table `note`, déclarée ici comme le ferait le code généré).

use forge_runtime::migration::Plan;
use forge_runtime::testing::{ADMIN_EMAIL, Method, PASSWORD, StatusCode, TestClient, database};
use forge_runtime::{App, Hooks};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::string_null;
use serde_json::{Value, json};

mod note {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize)]
    #[sea_orm(table_name = "note")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub titre: Option<String>,
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

fn app() -> App {
    App::new(
        &json!({
            "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
            "roles": ["admin", "commercial"],
            "parameters": [{ "name": "tva", "type": "decimal", "default": 20 }],
            "tables": [{ "name": "note", "columns": [{ "name": "titre", "type": "string" }] }]
        })
        .to_string(),
    )
    .unwrap()
    .resource::<note::Entity, NoteHooks>()
}

async fn admin() -> TestClient {
    TestClient::new(app(), database::<Migrator>().await).await
}

async fn session(client: &TestClient, email: &str, password: &str) -> (StatusCode, Value) {
    client
        .anonymous()
        .request(
            Method::POST,
            "/api/auth/login",
            Some(json!({ "email": email, "password": password })),
        )
        .await
}

#[tokio::test]
async fn login_me_and_access_control() {
    let admin = admin().await;
    let (status, me) = admin.request(Method::GET, "/api/auth/me", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["email"], ADMIN_EMAIL);
    assert_eq!(me["roles"], json!(["admin"]));
    assert!(me.get("password_hash").is_none());

    assert_eq!(
        session(&admin, ADMIN_EMAIL, "mauvais").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session(&admin, "inconnu@test.local", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = admin
        .anonymous()
        .request(Method::GET, "/api/auth/me", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
    assert_eq!(
        admin
            .anonymous()
            .request(Method::GET, "/api/parameters", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Comptes et paramètres : écriture réservée aux administrateurs.
    let user = admin.as_new_user("vente@test.local", &["commercial"]).await;
    assert_eq!(
        user.request(Method::GET, "/api/users", None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        user.request(Method::GET, "/api/parameters", None).await.0,
        StatusCode::OK
    );
    let change = Some(json!({ "value": 5.5 }));
    assert_eq!(
        user.request(Method::PUT, "/api/parameters/tva", change.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin
            .request(Method::PUT, "/api/parameters/tva", change)
            .await
            .0,
        StatusCode::OK
    );
    let (_, users) = admin.request(Method::GET, "/api/users?q=VENTE", None).await;
    assert_eq!(users["total"], 1);
}

#[tokio::test]
async fn refresh_tokens_rotate_and_can_be_revoked() {
    let admin = admin().await;
    let (status, first) = session(&admin, ADMIN_EMAIL, PASSWORD).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["token_type"], "Bearer");
    let refresh = |token: &Value| Some(json!({ "refresh_token": token }));

    let (status, second) = admin
        .anonymous()
        .request(
            Method::POST,
            "/api/auth/refresh",
            refresh(&first["refresh_token"]),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(second["refresh_token"], first["refresh_token"]);
    // Un jeton déjà utilisé est refusé.
    let reused = admin
        .anonymous()
        .request(
            Method::POST,
            "/api/auth/refresh",
            refresh(&first["refresh_token"]),
        )
        .await;
    assert_eq!(reused.0, StatusCode::UNAUTHORIZED);

    let (status, _) = admin
        .request(
            Method::POST,
            "/api/auth/logout",
            refresh(&second["refresh_token"]),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let after_logout = admin
        .anonymous()
        .request(
            Method::POST,
            "/api/auth/refresh",
            refresh(&second["refresh_token"]),
        )
        .await;
    assert_eq!(after_logout.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn password_change_and_deactivation() {
    let admin = admin().await;
    let user = admin.as_new_user("vente@test.local", &["commercial"]).await;

    let wrong = json!({ "current_password": "faux", "new_password": "nouveau-secret" });
    let (status, body) = user
        .request(Method::PUT, "/api/auth/password", Some(wrong))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let change = json!({ "current_password": PASSWORD, "new_password": "nouveau-secret" });
    assert_eq!(
        user.request(Method::PUT, "/api/auth/password", Some(change))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        session(&admin, "vente@test.local", PASSWORD).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session(&admin, "vente@test.local", "nouveau-secret")
            .await
            .0,
        StatusCode::OK
    );

    let (_, me) = user.request(Method::GET, "/api/auth/me", None).await;
    let account = format!("/api/users/{}", me["id"]);
    let (status, body) = admin
        .request(Method::PATCH, &account, Some(json!({ "active": false })))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        session(&admin, "vente@test.local", "nouveau-secret")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        admin.request(Method::DELETE, &account, None).await.0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn accounts_are_validated_and_admins_protected() {
    let admin = admin().await;
    let invalid = json!({ "email": "x", "password": "court", "roles": ["pirate"] });
    let (status, body) = admin
        .request(Method::POST, "/api/users", Some(invalid))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"]["fields"].as_object().unwrap().len(),
        3,
        "{body}"
    );
    let duplicate = json!({ "email": ADMIN_EMAIL.to_uppercase(), "password": PASSWORD });
    assert_eq!(
        admin
            .request(Method::POST, "/api/users", Some(duplicate))
            .await
            .0,
        StatusCode::CONFLICT
    );

    let (_, me) = admin.request(Method::GET, "/api/auth/me", None).await;
    let account = format!("/api/users/{}", me["id"]);
    assert_eq!(
        admin.request(Method::DELETE, &account, None).await.0,
        StatusCode::CONFLICT
    );
    let demote = json!({ "roles": ["commercial"] });
    assert_eq!(
        admin.request(Method::PATCH, &account, Some(demote)).await.0,
        StatusCode::CONFLICT
    );
}
