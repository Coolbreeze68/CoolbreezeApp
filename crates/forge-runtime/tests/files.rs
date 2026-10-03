//! Modèles de champ : validation des valeurs, fichiers téléversés (rattachement,
//! URL signées, remplacement et suppression).

use forge_runtime::migration::Plan;
use forge_runtime::testing::{Method, StatusCode, TEST_PNG, TestClient, check_resources, database};
use forge_runtime::{App, Hooks};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{big_integer_null, string_null};
use serde_json::{Value, json};

mod fiche {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize)]
    #[sea_orm(table_name = "fiche")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub nom: Option<String>,
        pub courriel: Option<String>,
        pub couleur: Option<String>,
        pub note: Option<i64>,
        pub logo: Option<String>,
        pub contrat: Option<String>,
        pub owner: Option<i64>,
        pub created_at: DateTimeUtc,
        pub updated_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Default)]
struct FicheHooks;

impl Hooks<fiche::Entity> for FicheHooks {}

struct CreateFiche;

impl MigrationName for CreateFiche {
    fn name(&self) -> &'static str {
        "m0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateFiche {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .table("fiche", |t| {
                for name in ["nom", "courriel", "couleur", "logo", "contrat"] {
                    t.col(string_null(name));
                }
                t.col(big_integer_null("note"));
            })
            .apply(manager)
            .await
    }
}

struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = forge_runtime::migration::system();
        migrations.push(Box::new(CreateFiche));
        migrations
    }
}

fn app() -> App {
    App::new(
        &json!({
            "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
            "roles": ["admin", "vendeur"],
            "tables": [{
                "name": "fiche",
                "columns": [
                    { "name": "nom", "type": "string", "title_field": true },
                    { "name": "courriel", "type": "email" },
                    { "name": "couleur", "type": "color" },
                    { "name": "note", "type": "rating", "max": 3 },
                    { "name": "logo", "type": "image" },
                    { "name": "contrat", "type": "file", "accept": [".pdf"], "max_size": 1 }
                ],
                "rules": [{ "roles": ["vendeur"], "actions": ["*"] }]
            }]
        })
        .to_string(),
    )
    .unwrap()
    .resource::<fiche::Entity, FicheHooks>()
}

async fn upload_logo(client: &TestClient) -> Value {
    let (status, file) = client
        .upload(
            ("fiche", "logo"),
            "logo.png",
            "image/png",
            TEST_PNG.to_vec(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{file}");
    file
}

#[tokio::test]
async fn generated_checks_cover_field_models() {
    check_resources(app(), database::<Migrator>().await).await;
}

#[tokio::test]
async fn values_are_checked_and_normalized() {
    let admin = TestClient::new(app(), database::<Migrator>().await).await;
    let (status, body) = admin
        .request(
            Method::POST,
            "/api/fiche",
            Some(json!({ "courriel": "pas-un-courriel", "couleur": "bleu", "note": 4 })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields = &body["error"]["fields"];
    for field in ["courriel", "couleur", "note"] {
        assert!(fields[field].is_array(), "{field} : {body}");
    }

    let (status, fiche) = admin
        .request(
            Method::POST,
            "/api/fiche",
            Some(json!({ "courriel": "a@b.fr", "couleur": "#AABBCC", "note": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{fiche}");
    assert_eq!(fiche["couleur"], json!("#aabbcc"));
}

#[tokio::test]
async fn files_are_attached_served_replaced_and_removed() {
    let admin = TestClient::new(app(), database::<Migrator>().await).await;
    let first = upload_logo(&admin).await;
    assert_eq!(first["content_type"], json!("image/png"));

    let (status, fiche) = admin
        .request(
            Method::POST,
            "/api/fiche",
            Some(json!({ "nom": "Acme", "logo": first["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{fiche}");
    assert_eq!(fiche["logo"]["id"], first["id"]);
    assert_eq!(fiche["logo"]["name"], json!("logo.png"));
    let url = fiche["logo"]["url"].as_str().unwrap().to_owned();

    // L'URL signée se lit sans jeton ; altérée, elle est refusée.
    let anonymous = admin.anonymous();
    let (headers, _) = anonymous.request_headers(Method::GET, &url).await;
    assert_eq!(headers["content-type"], "image/png");
    let (status, _) = anonymous
        .request_text(Method::GET, &format!("{url}x"), "text/plain", "")
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Un fichier ne sert qu'une fois.
    let (status, body) = admin
        .request(
            Method::POST,
            "/api/fiche",
            Some(json!({ "logo": first["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // Remplacé : l'ancien fichier disparaît ; la description relue est acceptée.
    let second = upload_logo(&admin).await;
    let id = fiche["id"].as_i64().unwrap();
    let (status, fiche) = admin
        .request(
            Method::PATCH,
            &format!("/api/fiche/{id}"),
            Some(json!({ "nom": "Acme SA", "logo": second })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{fiche}");
    assert_eq!(fiche["logo"]["id"], second["id"]);
    let (status, _) = anonymous
        .request_text(Method::GET, &url, "text/plain", "")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Supprimé avec l'enregistrement.
    let url = fiche["logo"]["url"].as_str().unwrap().to_owned();
    let (status, _) = admin
        .request(Method::DELETE, &format!("/api/fiche/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = anonymous
        .request_text(Method::GET, &url, "text/plain", "")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn uploads_are_checked() {
    let admin = TestClient::new(app(), database::<Migrator>().await).await;
    let (status, body) = admin
        .upload(("fiche", "logo"), "x.svg", "image/png", b"<svg/>".to_vec())
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, body) = admin
        .upload(
            ("fiche", "contrat"),
            "x.txt",
            "text/plain",
            b"texte".to_vec(),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, body) = admin
        .upload(
            ("fiche", "contrat"),
            "gros.pdf",
            "application/pdf",
            vec![0; 1024 * 1024 + 1],
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, contract) = admin
        .upload(
            ("fiche", "contrat"),
            "c.pdf",
            "application/pdf",
            b"%PDF".to_vec(),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{contract}");
    let (status, _) = admin
        .upload(("fiche", "nom"), "x.png", "image/png", TEST_PNG.to_vec())
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Le fichier d'un autre utilisateur ne peut pas être rattaché.
    let vendeur = admin.as_new_user("vendeur@exemple.fr", &["vendeur"]).await;
    let (status, body) = vendeur
        .request(
            Method::POST,
            "/api/fiche",
            Some(json!({ "contrat": contract["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, fiche) = vendeur
        .request(Method::POST, "/api/fiche", Some(json!({ "nom": "x" })))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{fiche}");
    let own = upload_logo(&vendeur).await;
    let (status, fiche) = vendeur
        .request(
            Method::PATCH,
            &format!("/api/fiche/{}", fiche["id"]),
            Some(json!({ "logo": own["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{fiche}");
}
