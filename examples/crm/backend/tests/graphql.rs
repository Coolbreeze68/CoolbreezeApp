//! API GraphQL du CRM : relations résolues, filtres, agrégats, erreurs, règles
//! et résolveur personnalisé (`src/custom/graphql.rs`). Test écrit à la main,
//! jamais modifié par forge.

use forge_runtime::testing::{ADMIN_EMAIL, Method, StatusCode, TestClient, database};
use mini_crm::generated::{Migrator, app};
use serde_json::{Value, json};

/// Donnée d'une réponse sans erreur.
async fn data(client: &TestClient, query: &str, variables: Value) -> Value {
    let response = client.graphql(query, variables).await;
    assert!(response.get("errors").is_none(), "{response}");
    response["data"].clone()
}

#[tokio::test]
async fn relations_filtres_et_agregats() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let created = data(
        &admin,
        r#"mutation {
            acme: create_entreprise(data: { nom: "Acme", secteur: industrie }) { id }
            urgent: create_tag(data: { nom: "urgent" }) { id }
        }"#,
        json!({}),
    )
    .await;
    let (acme, urgent) = (&created["acme"]["id"], &created["urgent"]["id"]);

    let create = "mutation($data: OpportuniteInput!) { create_opportunite(data: $data) { id } }";
    for (titre, montant) in [("Contrat", "1000"), ("Extension", "250.50")] {
        let input = json!({ "titre": titre, "entreprise": acme, "montant": montant, "etape": "gagne", "tags": [urgent] });
        data(&admin, create, json!({ "data": input })).await;
    }

    // Références résolues, énumérations, décimaux en texte, colonnes calculées.
    let list = data(
        &admin,
        r#"{ opportunite_list(sort: "-montant", filter: { montant: { gte: "500" } }) {
            total
            data { titre etape montant montant_pondere secteur entreprise { nom secteur } tags { nom } }
        } }"#,
        json!({}),
    )
    .await;
    let page = &list["opportunite_list"];
    assert_eq!(page["total"], json!(1), "{page}");
    let opportunite = &page["data"][0];
    assert_eq!(opportunite["titre"], "Contrat");
    assert_eq!(opportunite["etape"], "gagne");
    assert_eq!(opportunite["montant_pondere"], "500");
    assert_eq!(opportunite["secteur"], "industrie", "lookup");
    assert_eq!(
        opportunite["entreprise"],
        json!({ "nom": "Acme", "secteur": "industrie" })
    );
    assert_eq!(opportunite["tags"], json!([{ "nom": "urgent" }]));

    let stats = data(
        &admin,
        r#"{ opportunite_aggregate(fields: ["montant"], group_by: "etape") }"#,
        json!({}),
    )
    .await;
    let total = &stats["opportunite_aggregate"]["total"];
    assert_eq!(total["count"], json!(2));
    assert_eq!(total["montant"]["sum"], "1250.5");

    // Modification partielle : seuls les champs fournis changent.
    let id = &data(
        &admin,
        "{ opportunite_list(sort: \"titre\") { data { id } } }",
        json!({}),
    )
    .await["opportunite_list"]["data"][0]["id"];
    let updated = data(
        &admin,
        "mutation($id: ID!) { update_opportunite(id: $id, data: { probabilite: 10 }) { titre probabilite montant_pondere } }",
        json!({ "id": id }),
    )
    .await;
    assert_eq!(
        updated["update_opportunite"],
        json!({ "titre": "Contrat", "probabilite": 10, "montant_pondere": "100" })
    );
    let missing = data(&admin, "{ opportunite(id: 999999) { id } }", json!({})).await;
    assert_eq!(missing["opportunite"], Value::Null);
}

#[tokio::test]
async fn erreurs_regles_et_extensions() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;

    // Erreurs de validation : code et détail par champ, comme en REST.
    let response = admin
        .graphql(
            "mutation { create_entreprise(data: { ville: \"Lyon\" }) { id } }",
            json!({}),
        )
        .await;
    let error = &response["errors"][0];
    assert_eq!(error["extensions"]["code"], "validation", "{response}");
    assert_eq!(
        error["extensions"]["fields"]["nom"],
        json!(["valeur obligatoire"])
    );

    // Règles : un commercial ne modifie pas le contact d'un autre.
    let contact = data(
        &admin,
        "mutation { create_contact(data: { nom: \"Martin\" }) { id } }",
        json!({}),
    )
    .await;
    let commercial = admin.as_new_user("vente@crm.test", &["commercial"]).await;
    let response = commercial
        .graphql(
            "mutation($id: ID!) { update_contact(id: $id, data: { nom: \"Durand\" }) { id } }",
            json!({ "id": contact["create_contact"]["id"] }),
        )
        .await;
    assert_eq!(
        response["errors"][0]["extensions"]["code"], "forbidden",
        "{response}"
    );

    // Paramètres : lisibles par tous, modifiables par l'administrateur.
    let response = commercial
        .graphql(
            "mutation { update_parameter(name: \"tva\", value: 5) }",
            json!({}),
        )
        .await;
    assert_eq!(
        response["errors"][0]["extensions"]["code"], "forbidden",
        "{response}"
    );
    let updated = data(
        &admin,
        "mutation { update_parameter(name: \"tva\", value: 5) }",
        json!({}),
    )
    .await;
    assert_eq!(updated["update_parameter"]["value"], json!(5));
    let params = data(&commercial, "{ parameters }", json!({})).await;
    assert!(
        params["parameters"]
            .as_array()
            .is_some_and(|p| !p.is_empty())
    );

    // Résolveur personnalisé, et accès refusé sans jeton.
    let hello = data(&admin, "{ bonjour }", json!({})).await;
    assert_eq!(hello["bonjour"], format!("Bonjour {ADMIN_EMAIL}"));
    let (status, _) = admin
        .anonymous()
        .request(
            Method::POST,
            "/api/graphql",
            Some(json!({ "query": "{ bonjour }" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn documentation_publique() {
    let admin = TestClient::new(app().unwrap(), database::<Migrator>().await).await;
    let anonymous = admin.anonymous();
    let (status, openapi) = anonymous.request(Method::GET, "/openapi.json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(openapi["paths"]["/api/opportunite/{id}"]["patch"].is_object());
    assert!(openapi["components"]["schemas"]["Opportunite"].is_object());
    let (status, page) = anonymous
        .request_text(Method::GET, "/docs/", "text/html", "")
        .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let (status, page) = anonymous
        .request_text(Method::GET, "/graphql", "text/html", "")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("graphiql"), "{page}");
}
