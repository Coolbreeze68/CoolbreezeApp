//! Santé, métriques, identifiant de requête et cache des lectures du CRM.
//! Test écrit à la main, jamais modifié par forge.

use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use serde_json::json;

#[tokio::test]
async fn sante_metriques_et_identifiant_de_requete() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let anonymous = admin.anonymous();
    let (status, health) = anonymous.request(Method::GET, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health, json!({ "status": "ok", "database": "ok" }));

    for _ in 0..2 {
        admin.request(Method::GET, "/api/tag", None).await;
    }
    let (status, metrics) = anonymous
        .request_text(Method::GET, "/metrics", "text/plain", "")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        metrics.contains(r#"http_requests_total{method="GET",path="/api/tag",status="200"}"#),
        "{metrics}"
    );
    assert!(
        metrics.contains("http_request_duration_seconds_bucket"),
        "{metrics}"
    );
    assert!(
        metrics.contains(r#"forge_cache_requests_total{table="tag",result="hit"}"#),
        "deuxième lecture servie par le cache : {metrics}"
    );

    let (headers, _) = anonymous.request_headers(Method::GET, "/health").await;
    let id = headers.get("x-request-id").expect("identifiant généré");
    assert_eq!(id.len(), 36, "UUID");
}

#[tokio::test]
async fn le_cache_suit_les_ecritures() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let (_, acme) = admin
        .request(
            Method::POST,
            "/api/entreprise",
            Some(json!({ "nom": "Acme" })),
        )
        .await;
    let body = json!({ "titre": "Contrat", "entreprise": acme["id"], "montant": "1000" });
    let (_, opportunite) = admin
        .request(Method::POST, "/api/opportunite", Some(body))
        .await;
    let url = format!("/api/opportunite/{}", opportunite["id"]);
    assert_eq!(
        admin.request(Method::GET, &url, None).await.1["etape"],
        "prospect"
    );

    // Écriture d'un hook signalée par `ctx.modified` (src/custom/hooks/activite.rs).
    let activite = json!({
        "sujet": "Appel", "nature": "appel", "debut": "2026-03-01T10:00:00Z",
        "opportunite": opportunite["id"],
    });
    let (status, body) = admin
        .request(Method::POST, "/api/activite", Some(activite))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        admin.request(Method::GET, &url, None).await.1["etape"],
        "proposition"
    );

    // Formule persistée d'une autre table, recalculée par propagation.
    let entreprise = format!("/api/entreprise/{}", acme["id"]);
    assert_eq!(
        admin.request(Method::GET, &entreprise, None).await.1["pipeline"],
        "1000"
    );
    admin
        .request(Method::PATCH, &url, Some(json!({ "montant": "2500" })))
        .await;
    assert_eq!(
        admin.request(Method::GET, &entreprise, None).await.1["pipeline"],
        "2500"
    );

    // Paramètre lu par une formule.
    let ttc = |r: serde_json::Value| r["montant_ttc"].clone();
    assert_eq!(ttc(admin.request(Method::GET, &url, None).await.1), "3000");
    admin
        .request(
            Method::PUT,
            "/api/parameters/tva",
            Some(json!({ "value": 10 })),
        )
        .await;
    assert_eq!(ttc(admin.request(Method::GET, &url, None).await.1), "2750");
}
