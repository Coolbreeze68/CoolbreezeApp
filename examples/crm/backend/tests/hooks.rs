//! Tests du code personnalisé (`src/custom/`) : écrits à la main, jamais modifiés par forge.

use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use serde_json::json;

#[tokio::test]
async fn une_opportunite_doit_avoir_un_montant_positif() {
    let client = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let (status, entreprise) = client
        .request(
            Method::POST,
            "/api/entreprise",
            Some(json!({ "nom": "Acme" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let opportunite =
        json!({ "titre": "Contrat", "entreprise": entreprise["id"], "montant": "-10" });
    let (status, body) = client
        .request(Method::POST, "/api/opportunite", Some(opportunite))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"]["fields"]["montant"][0],
        "le montant doit être strictement positif"
    );
}
