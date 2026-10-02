//! Règles conditionnelles du CRM : un commercial ne modifie que ses contacts.
//! Test écrit à la main, jamais modifié par forge.

use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use serde_json::json;

#[tokio::test]
async fn un_commercial_ne_modifie_que_ses_contacts() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let alice = admin.as_new_user("alice@crm.test", &["commercial"]).await;
    let bruno = admin.as_new_user("bruno@crm.test", &["commercial"]).await;
    let lecteur = admin.as_new_user("lea@crm.test", &["lecteur"]).await;

    let (status, contact) = alice
        .request(
            Method::POST,
            "/api/contact",
            Some(json!({ "nom": "Martin" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{contact}");
    let (_, me) = alice.request(Method::GET, "/api/auth/me", None).await;
    assert_eq!(
        contact["owner"], me["id"],
        "le créateur devient propriétaire"
    );
    let url = format!("/api/contact/{}", contact["id"]);
    let rename = Some(json!({ "nom": "Martin-Durand" }));

    // Lecture ouverte à tous les commerciaux, modification réservée au propriétaire.
    assert_eq!(
        bruno.request(Method::GET, &url, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        bruno.request(Method::PATCH, &url, rename.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        bruno.request(Method::DELETE, &url, None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        alice.request(Method::PATCH, &url, rename).await.0,
        StatusCode::OK
    );

    // Le lecteur lit tout mais n'écrit rien.
    let (status, list) = lecteur.request(Method::GET, "/api/contact", None).await;
    assert_eq!((status, list["total"].clone()), (StatusCode::OK, json!(1)));
    let (status, _) = lecteur
        .request(
            Method::POST,
            "/api/contact",
            Some(json!({ "nom": "Dupont" })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    assert_eq!(
        alice.request(Method::DELETE, &url, None).await.0,
        StatusCode::NO_CONTENT
    );
}
