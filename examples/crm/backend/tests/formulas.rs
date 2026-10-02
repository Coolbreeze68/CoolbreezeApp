//! Formules et lookups du CRM : valeurs calculées à la lecture, formules
//! persistées recalculées à l'écriture (y compris entre tables et quand un
//! paramètre change). Test écrit à la main, jamais modifié par forge.

use forge_runtime::testing::{Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use rust_decimal::Decimal;
use serde_json::{Value, json};

/// Décimal d'une réponse (texte ou nombre selon la base).
fn dec(value: &Value) -> Decimal {
    match value {
        Value::String(s) => s.parse().unwrap(),
        Value::Number(n) => n.to_string().parse().unwrap(),
        other => panic!("décimal attendu : {other}"),
    }
}

async fn create(client: &TestClient, table: &str, body: Value) -> Value {
    let (status, record) = client
        .request(Method::POST, &format!("/api/{table}"), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{table} : {record}");
    record
}

async fn get(client: &TestClient, table: &str, id: &Value) -> Value {
    let (status, record) = client
        .request(Method::GET, &format!("/api/{table}/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    record
}

async fn patch(client: &TestClient, table: &str, id: &Value, body: Value) -> Value {
    let (status, record) = client
        .request(Method::PATCH, &format!("/api/{table}/{id}"), Some(body))
        .await;
    assert_eq!(status, StatusCode::OK, "{record}");
    record
}

#[tokio::test]
async fn formules_lookups_et_propagation() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let acme = create(
        &admin,
        "entreprise",
        json!({ "nom": "Acme", "secteur": "industrie" }),
    )
    .await;
    let beta = create(
        &admin,
        "entreprise",
        json!({ "nom": "Beta", "secteur": "services" }),
    )
    .await;

    // Formules de la ligne : persistée (pondéré, TTC) et calculée à la lecture.
    let in_ten_days = (chrono::Utc::now().date_naive() + chrono::Duration::days(10)).to_string();
    let o1 = create(
        &admin,
        "opportunite",
        json!({ "titre": "A", "entreprise": acme["id"], "montant": 1000, "date_cloture": in_ten_days }),
    )
    .await;
    assert_eq!(
        dec(&o1["montant_pondere"]),
        Decimal::from(500),
        "50 % par défaut"
    );
    assert_eq!(dec(&o1["montant_ttc"]), Decimal::from(1200), "TVA à 20 %");
    assert_eq!(o1["jours_restants"], json!(10));
    assert_eq!(o1["secteur"], json!("industrie"), "lookup");
    let o2 = create(
        &admin,
        "opportunite",
        json!({ "titre": "B", "entreprise": acme["id"], "montant": 2000, "probabilite": 30 }),
    )
    .await;
    assert_eq!(dec(&o2["montant_pondere"]), Decimal::from(600));
    assert_eq!(o2["jours_restants"], Value::Null, "pas de date de clôture");

    // Agrégats sur la relation inverse : persisté (pipeline) et à la lecture.
    let entreprise = get(&admin, "entreprise", &acme["id"]).await;
    assert_eq!(dec(&entreprise["pipeline"]), Decimal::from(3000));
    assert_eq!(dec(&entreprise["pipeline_pondere"]), Decimal::from(1100));
    assert_eq!(entreprise["nb_contacts"], json!(0));
    let contact = create(
        &admin,
        "contact",
        json!({ "nom": "Martin", "entreprise": acme["id"] }),
    )
    .await;
    assert_eq!(contact["secteur"], json!("industrie"));
    assert_eq!(
        get(&admin, "entreprise", &acme["id"]).await["nb_contacts"],
        json!(1)
    );

    // Modification, déplacement et suppression propagés aux entreprises concernées.
    patch(&admin, "opportunite", &o2["id"], json!({ "montant": 4000 })).await;
    assert_eq!(
        dec(&get(&admin, "entreprise", &acme["id"]).await["pipeline"]),
        Decimal::from(5000)
    );
    patch(
        &admin,
        "opportunite",
        &o2["id"],
        json!({ "entreprise": beta["id"] }),
    )
    .await;
    assert_eq!(
        dec(&get(&admin, "entreprise", &acme["id"]).await["pipeline"]),
        Decimal::from(1000)
    );
    assert_eq!(
        dec(&get(&admin, "entreprise", &beta["id"]).await["pipeline"]),
        Decimal::from(4000)
    );
    let (status, _) = admin
        .request(
            Method::DELETE,
            &format!("/api/opportunite/{}", o1["id"]),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        dec(&get(&admin, "entreprise", &acme["id"]).await["pipeline"]),
        Decimal::ZERO
    );

    // Agrégat sur une relation N↔N, vue depuis la cible.
    let tag = create(&admin, "tag", json!({ "nom": "urgent" })).await;
    patch(
        &admin,
        "opportunite",
        &o2["id"],
        json!({ "tags": [tag["id"]] }),
    )
    .await;
    assert_eq!(
        get(&admin, "tag", &tag["id"]).await["nb_opportunites"],
        json!(1)
    );
    patch(&admin, "opportunite", &o2["id"], json!({ "tags": [] })).await;
    assert_eq!(
        get(&admin, "tag", &tag["id"]).await["nb_opportunites"],
        json!(0)
    );

    // Un paramètre modifié recalcule les formules persistées qui le lisent.
    let (status, _) = admin
        .request(
            Method::PUT,
            "/api/parameters/tva",
            Some(json!({ "value": 10 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        dec(&get(&admin, "opportunite", &o2["id"]).await["montant_ttc"]),
        Decimal::from(4400)
    );
}

#[tokio::test]
async fn agregats_par_etape() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let acme = create(&admin, "entreprise", json!({ "nom": "Acme" })).await;
    for (montant, etape, probabilite) in [
        (1000, "gagne", 100),
        (3000, "gagne", 100),
        (500, "perdu", 0),
        (800, "prospect", 50),
    ] {
        let body = json!({ "titre": "x", "entreprise": acme["id"], "montant": montant, "etape": etape, "probabilite": probabilite });
        create(&admin, "opportunite", body).await;
    }

    let (status, stats) = admin
        .request(
            Method::GET,
            "/api/opportunite/aggregate?fields=montant,montant_pondere&group_by=etape",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["total"]["count"], json!(4));
    assert_eq!(dec(&stats["total"]["montant"]["sum"]), Decimal::from(5300));
    assert_eq!(
        dec(&stats["total"]["montant_pondere"]["sum"]),
        Decimal::from(4400)
    );
    let groups = stats["groups"].as_array().unwrap();
    let keys: Vec<_> = groups.iter().map(|g| g["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["gagne", "perdu", "prospect"]);
    assert_eq!(groups[0]["count"], json!(2));
    assert_eq!(dec(&groups[0]["montant"]["avg"]), Decimal::from(2000));
    assert_eq!(dec(&groups[0]["montant"]["min"]), Decimal::from(1000));
    assert_eq!(dec(&groups[0]["montant"]["max"]), Decimal::from(3000));

    // Les filtres de liste s'appliquent ; les paramètres invalides sont refusés.
    let (_, filtered) = admin
        .request(
            Method::GET,
            "/api/opportunite/aggregate?fields=montant&montant[gte]=900",
            None,
        )
        .await;
    assert_eq!(filtered["total"]["count"], json!(2));
    assert_eq!(filtered["groups"], json!([]));
    let (status, body) = admin
        .request(
            Method::GET,
            "/api/opportunite/aggregate?fields=titre,montant_ttc&group_by=notes_internes",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body["error"]["fields"]["fields"].as_array().unwrap().len(),
        1,
        "{body}"
    );
}
