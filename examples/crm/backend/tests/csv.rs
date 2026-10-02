//! Import et export CSV du CRM : tout ou rien, rapport d'erreurs par ligne,
//! filtres et règles appliqués. Test écrit à la main, jamais modifié par forge.

use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use serde_json::{Value, json};

async fn import(client: &TestClient, table: &str, csv: &str) -> (StatusCode, Value) {
    let (status, body) = client
        .request_text(
            Method::POST,
            &format!("/api/{table}/import"),
            "text/csv",
            csv,
        )
        .await;
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

async fn total(client: &TestClient, table: &str) -> Value {
    client
        .request(Method::GET, &format!("/api/{table}"), None)
        .await
        .1["total"]
        .clone()
}

#[tokio::test]
async fn import_tout_ou_rien() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;

    // Séparateur `;` détecté ; les formules persistées sont calculées.
    let (status, report) = import(
        &admin,
        "entreprise",
        "nom;secteur;chiffre_affaires\nAcme;industrie;1500,5\n",
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "virgule décimale refusée : {report}"
    );
    let lines = &report["error"]["lines"];
    assert_eq!(lines[0]["line"], json!(2), "{report}");
    assert!(
        lines[0]["fields"]["chiffre_affaires"].is_array(),
        "{report}"
    );

    let (status, report) = import(
        &admin,
        "entreprise",
        "nom;secteur;chiffre_affaires\nAcme;industrie;1500.5\n\"Beta; et fils\";services;\n",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report, json!({ "created": 2, "updated": 0 }));

    // Plusieurs lignes en erreur : toutes rapportées, rien n'est enregistré.
    let csv = "titre,entreprise,montant,etape\n\
               Valide,1,100,gagne\n\
               ,1,abc,inconnue\n\
               Orpheline,999,10,\n";
    let (status, report) = import(&admin, "opportunite", csv).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{report}");
    assert_eq!(report["error"]["code"], "import");
    let lines = report["error"]["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2, "{report}");
    assert_eq!(lines[0]["line"], json!(3));
    let fields = &lines[0]["fields"];
    assert!(
        fields["montant"].is_array() && fields["etape"].is_array(),
        "{report}"
    );
    assert_eq!(
        (lines[1]["line"].clone(), lines[1]["code"].clone()),
        (json!(4), json!("conflict"))
    );
    assert_eq!(
        total(&admin, "opportunite").await,
        json!(0),
        "rien n'est enregistré"
    );

    // Valeur unique déjà prise : l'erreur n'empêche pas de vérifier la suite.
    let (status, report) = import(&admin, "entreprise", "nom\nGamma\nAcme\nDelta\n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{report}");
    assert_eq!(
        report["error"]["lines"].as_array().map(Vec::len),
        Some(1),
        "{report}"
    );
    assert_eq!(report["error"]["lines"][0]["line"], json!(3));
    assert_eq!(total(&admin, "entreprise").await, json!(2));

    let (status, report) = import(&admin, "entreprise", "nom,inconnue\nX,1\n").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        report["error"]["lines"][0]["line"],
        json!(1),
        "en-tête : {report}"
    );
}

#[tokio::test]
async fn export_filtre_et_regles() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let csv = "nom,secteur,chiffre_affaires\nAcme,industrie,1500\nBeta,services,\n";
    assert_eq!(import(&admin, "entreprise", csv).await.0, StatusCode::OK);

    let (status, export) = admin
        .request_text(
            Method::GET,
            "/api/entreprise/export?secteur=industrie&delimiter=;",
            "text/plain",
            "",
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{export}");
    let lines: Vec<_> = export.lines().collect();
    assert_eq!(lines.len(), 2, "{export}");
    assert!(lines[0].starts_with("id;nom;secteur;"), "{export}");
    assert!(lines[1].contains(";Acme;industrie;"), "{export}");

    // Réimport modifié : une ligne avec `id` modifie l'enregistrement.
    let modified = export.replace(";Acme;", ";Acme SA;");
    let (status, report) = import(&admin, "entreprise", &modified).await;
    assert_eq!(
        (status, report),
        (StatusCode::OK, json!({ "created": 0, "updated": 1 }))
    );

    // Règles : un commercial ne modifie pas les contacts d'un autre.
    assert_eq!(
        import(&admin, "contact", "nom\nMartin\n").await.0,
        StatusCode::OK
    );
    let commercial = admin.as_new_user("vente@crm.test", &["commercial"]).await;
    let (_, contacts) = commercial
        .request_text(Method::GET, "/api/contact/export", "text/plain", "")
        .await;
    let (status, report) = import(
        &commercial,
        "contact",
        &contacts.replace("Martin", "Durand"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{report}");
    assert_eq!(report["error"]["lines"][0]["code"], "forbidden", "{report}");
    let lecteur = admin.as_new_user("lea@crm.test", &["lecteur"]).await;
    let (status, report) = import(&lecteur, "tag", "nom\nurgent\n").await;
    assert_eq!(report["error"]["lines"][0]["code"], "forbidden", "{report}");
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
