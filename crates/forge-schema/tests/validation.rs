//! Chaque règle de validation est couverte par un cas minimal : un schéma
//! valide de base, modifié pour introduire une seule erreur.

use forge_schema::{ColumnRef, Model, RelationKind, SchemaError};
use serde_json::{Value, json};

/// Schéma minimal valide : `client` ← `commande` (N→1), `produit` (N↔N).
fn base() -> Value {
    json!({
        "app": { "name": "demo", "default_locale": "fr", "locales": ["fr", "en"] },
        "roles": ["admin", "vendeur"],
        "parameters": [{ "name": "tva", "type": "decimal", "default": 20 }],
        "tables": [
            {
                "name": "client",
                "columns": [
                    { "name": "nom", "type": "string", "required": true, "title_field": true },
                    { "name": "categorie", "type": "enum", "values": ["pme", "grand_compte"] }
                ]
            },
            {
                "name": "commande",
                "columns": [
                    { "name": "client", "type": "reference", "target": "client", "inverse": "commandes" },
                    { "name": "montant", "type": "decimal" },
                    { "name": "livraison", "type": "date" },
                    { "name": "produits", "type": "reference_list", "target": "produit" }
                ]
            },
            {
                "name": "produit",
                "columns": [{ "name": "libelle", "type": "string" }, { "name": "prix", "type": "decimal" }]
            }
        ]
    })
}

fn load(schema: &Value) -> Result<Model, SchemaError> {
    Model::from_json(&schema.to_string())
}

fn issues(schema: &Value) -> Vec<String> {
    match load(schema) {
        Ok(_) => Vec::new(),
        Err(err) => err.issues.iter().map(ToString::to_string).collect(),
    }
}

/// Vérifie qu'une erreur porte sur `path` et contient `fragment`.
#[track_caller]
fn assert_issue(schema: &Value, path: &str, fragment: &str) {
    let issues = issues(schema);
    assert!(
        issues
            .iter()
            .any(|i| i.starts_with(&format!("{path}: ")) && i.contains(fragment)),
        "attendu `{path}: …{fragment}…`, obtenu : {issues:#?}"
    );
}

/// Ajoute une colonne à la table `index` et retourne le schéma.
fn with_column(index: usize, column: Value) -> Value {
    let mut schema = base();
    schema["tables"][index]["columns"]
        .as_array_mut()
        .unwrap()
        .push(column);
    schema
}

#[test]
fn base_is_valid() {
    assert_eq!(issues(&base()), Vec::<String>::new());
}

#[test]
fn crm_example_is_valid() {
    let src = include_str!("../../../examples/crm/forge.json");
    let model = Model::from_json(src).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(model.tables().len(), 5);
}

#[test]
fn json_schema_file_is_up_to_date() {
    let committed: Value =
        serde_json::from_str(include_str!("../../../forge.schema.json")).unwrap();
    assert_eq!(
        committed,
        forge_schema::json_schema(),
        "forge.schema.json est obsolète : `cargo run -p forge-cli -- schema > forge.schema.json`"
    );
}

// ------------------------------------------------------------ structure JSON

#[test]
fn json_errors_have_paths() {
    let mut schema = base();
    schema["tables"][1]["columns"][1]["type"] = json!("money");
    assert_issue(
        &schema,
        "tables[1].columns[1].type",
        "unknown variant `money`",
    );

    let mut schema = base();
    schema["tables"][0]["columns"][0]["requird"] = json!(true);
    assert_issue(
        &schema,
        "tables[0].columns[0].requird",
        "unknown field `requird`",
    );

    assert!(issues(&json!("pas un objet"))[0].contains("JSON invalide"));
    let err = Model::from_json("{ \"app\": ").unwrap_err();
    assert!(err.issues[0].message.contains("line 1"));
}

#[test]
fn collects_all_errors() {
    let mut schema = base();
    schema["app"]["name"] = json!("Demo");
    schema["tables"][0]["columns"][0]["name"] = json!("id");
    schema["tables"][1]["columns"][1]["formula"] = json!("inconnu");
    assert_eq!(issues(&schema).len(), 3, "{:#?}", issues(&schema));
}

// ------------------------------------------------------------ app, rôles, paramètres

#[test]
fn app_checks() {
    let mut schema = base();
    schema["app"]["default_locale"] = json!("de");
    assert_issue(&schema, "app.default_locale", "doit figurer");

    let mut schema = base();
    schema["app"]["locales"] = json!(["fr", "fr", "FR"]);
    assert_issue(&schema, "app.locales[1]", "en double");
    assert_issue(&schema, "app.locales[2]", "code de langue");

    let mut schema = base();
    schema["tables"][0]["label"] = json!({ "fr": "Client", "de": "Kunde" });
    assert_issue(&schema, "tables[0].label.de", "absente de `app.locales`");
}

#[test]
fn role_checks() {
    let mut schema = base();
    schema["roles"] = json!(["vendeur", "vendeur"]);
    assert_issue(&schema, "roles", "`admin` est obligatoire");
    assert_issue(&schema, "roles[1]", "en double");
}

#[test]
fn parameter_checks() {
    let mut schema = base();
    schema["parameters"] = json!([
        { "name": "tva", "type": "decimal", "default": "vingt" },
        { "name": "tva", "type": "reference" }
    ]);
    assert_issue(&schema, "parameters[0].default", "attendu un nombre");
    assert_issue(&schema, "parameters[1].name", "en double");
    assert_issue(&schema, "parameters[1].type", "non autorisé");
}

// ------------------------------------------------------------ tables et colonnes

#[test]
fn table_names() {
    let mut schema = base();
    schema["tables"][0]["name"] = json!("users");
    assert_issue(&schema, "tables[0].name", "table système");

    let mut schema = base();
    schema["tables"][2]["name"] = json!("client");
    assert_issue(&schema, "tables[2].name", "en double");

    let mut schema = base();
    schema["tables"][0]["columns"] = json!([]);
    assert_issue(&schema, "tables[0].columns", "au moins une colonne");
}

#[test]
fn column_names() {
    assert_issue(
        &with_column(0, json!({ "name": "created_at", "type": "datetime" })),
        "tables[0].columns[2].name",
        "ajoutée automatiquement",
    );
    assert_issue(
        &with_column(0, json!({ "name": "type", "type": "string" })),
        "tables[0].columns[2].name",
        "mot réservé",
    );
    assert_issue(
        &with_column(0, json!({ "name": "nom", "type": "string" })),
        "tables[0].columns[2].name",
        "en double",
    );
}

#[test]
fn type_specific_options() {
    assert_issue(
        &with_column(0, json!({ "name": "x", "type": "reference" })),
        "tables[0].columns[2]",
        "exige l'option `target`",
    );
    assert_issue(
        &with_column(0, json!({ "name": "x", "type": "string", "values": ["a"] })),
        "tables[0].columns[2].values",
        "option inutile",
    );
    assert_issue(
        &with_column(
            0,
            json!({ "name": "x", "type": "enum", "values": ["a", "a", "B"] }),
        ),
        "tables[0].columns[2].values[1]",
        "en double",
    );
    assert_issue(
        &with_column(0, json!({ "name": "x", "type": "string", "inverse": "y" })),
        "tables[0].columns[2].inverse",
        "réservé",
    );
}

#[test]
fn incompatible_options() {
    let cases = [
        (
            json!({ "name": "x", "type": "decimal", "persist": true }),
            "persist",
            "nécessite",
        ),
        (
            json!({ "name": "x", "type": "enum", "values": ["a"], "formula": "1" }),
            "formula",
            "ne peut pas être calculée",
        ),
        (
            json!({ "name": "x", "type": "decimal", "formula": "1", "required": true }),
            "required",
            "calculée",
        ),
        (
            json!({ "name": "x", "type": "text", "unique": true }),
            "unique",
            "indexable",
        ),
        (
            json!({ "name": "x", "type": "decimal", "formula": "1", "unique": true }),
            "unique",
            "indexable",
        ),
        (
            json!({ "name": "x", "type": "string", "hidden": true, "title_field": true }),
            "title_field",
            "hidden",
        ),
        (
            json!({ "name": "x", "type": "integer", "default": 1.5 }),
            "default",
            "entier",
        ),
        (
            json!({ "name": "x", "type": "date", "default": "31/01/2026" }),
            "default",
            "AAAA-MM-JJ",
        ),
        (
            json!({ "name": "x", "type": "enum", "values": ["a"], "default": "b" }),
            "default",
            "valeurs",
        ),
        (
            json!({ "name": "x", "type": "reference", "target": "produit", "default": 1 }),
            "default",
            "relation",
        ),
    ];
    for (column, option, fragment) in cases {
        assert_issue(
            &with_column(0, column),
            &format!("tables[0].columns[2].{option}"),
            fragment,
        );
    }
    assert!(issues(&with_column(0, json!({ "name": "x", "type": "decimal", "formula": "1", "persist": true, "unique": true }))).is_empty());
}

// ------------------------------------------------------------ relations

#[test]
fn relations_are_resolved() {
    let model = load(&base()).unwrap();
    let relations = model.relations();
    assert_eq!(relations.len(), 2);
    assert_eq!(relations[0].source, ColumnRef::new("commande", "client"));
    assert_eq!(relations[0].inverse, "commandes");
    assert_eq!(relations[0].kind, RelationKind::ManyToOne);
    // Nom inverse par défaut : la table source.
    assert_eq!(relations[1].inverse, "commande");
    assert_eq!(relations[1].kind, RelationKind::ManyToMany);
}

#[test]
fn relation_errors() {
    assert_issue(
        &with_column(
            1,
            json!({ "name": "x", "type": "reference", "target": "fournisseur" }),
        ),
        "tables[1].columns[4].target",
        "table `fournisseur` inconnue",
    );
    // Deux références vers `client` sans `inverse` : collision sur le nom par défaut.
    let mut schema = base();
    schema["tables"][1]["columns"][0]
        .as_object_mut()
        .unwrap()
        .remove("inverse");
    schema["tables"][1]["columns"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "payeur", "type": "reference", "target": "client" }));
    assert_issue(
        &schema,
        "tables[1].columns[4]",
        "déjà une relation inverse `commande`",
    );

    assert_issue(
        &with_column(
            1,
            json!({ "name": "x", "type": "reference", "target": "client", "inverse": "nom" }),
        ),
        "tables[1].columns[4].inverse",
        "déjà une colonne `nom`",
    );
}

// ------------------------------------------------------------ formules

fn with_formula(table: usize, formula: &str) -> Value {
    with_column(
        table,
        json!({ "name": "calcul", "type": "decimal", "formula": formula }),
    )
}

#[test]
fn valid_formulas() {
    let formulas = [
        (1, "montant * (1 + $param.tva / 100)"),
        (
            1,
            "IF(livraison == NULL, 0, DAYS_BETWEEN(TODAY(), livraison))",
        ),
        (1, "CONCAT(client.nom, \" \", client.categorie)"),
        (1, "SUM(produits.prix) + COUNT(produits)"),
        (0, "SUM(commandes.montant) / COUNT(commandes)"),
        (0, "MAX(commandes.livraison)"),
        (1, "id + owner"),
    ];
    for (table, formula) in formulas {
        assert!(
            issues(&with_formula(table, formula)).is_empty(),
            "{formula} : {:#?}",
            issues(&with_formula(table, formula))
        );
    }
}

#[test]
fn formula_errors() {
    let cases = [
        (1, "montant *", "syntaxe"),
        (
            1,
            "montnt * 2",
            "`montnt` n'est ni une colonne ni une relation de la table `commande`",
        ),
        (1, "montant.x", "n'est pas une référence"),
        (1, "client.inconnu", "de la table `client`"),
        (1, "produits.prix", "plusieurs valeurs"),
        (0, "commandes.montant", "plusieurs valeurs"),
        (1, "SUM(montant)", "pas une relation « plusieurs »"),
        (0, "SUM(commandes)", "attend une colonne"),
        (0, "SUM(commandes.montant * 2)", "attend une relation"),
        (0, "SUM(SUM(commandes.montant))", "attend une relation"),
        (1, "TVA(montant)", "fonction `TVA` inconnue"),
        (1, "ROUND()", "1 à 2 argument(s), 0 fourni(s)"),
        (1, "$user.id", "que dans les conditions de règles"),
        (1, "$param.remise", "non déclaré"),
    ];
    for (table, formula, fragment) in cases {
        let path = format!(
            "tables[{table}].columns[{}].formula",
            if table == 0 { 2 } else { 4 }
        );
        assert_issue(&with_formula(table, formula), &path, fragment);
    }
}

#[test]
fn formula_error_reports_position() {
    assert_issue(
        &with_formula(1, "montant + montnt"),
        "tables[1].columns[4].formula",
        "(position 10)",
    );
}

#[test]
fn persisted_formula_cannot_be_volatile() {
    let schema = with_column(
        1,
        json!({ "name": "delai", "type": "integer", "formula": "DAYS_BETWEEN(TODAY(), livraison)", "persist": true }),
    );
    assert_issue(
        &schema,
        "tables[1].columns[4].formula",
        "`TODAY` change avec le temps",
    );
}

#[test]
fn computed_order_follows_dependencies() {
    let mut schema = base();
    let columns = schema["tables"][1]["columns"].as_array_mut().unwrap();
    columns.push(json!({ "name": "ttc", "type": "decimal", "formula": "ht * 1.2" }));
    columns.push(json!({ "name": "ht", "type": "decimal", "formula": "montant" }));
    schema["tables"][0]["columns"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "total", "type": "decimal", "formula": "SUM(commandes.ttc)" }));

    let model = load(&schema).unwrap();
    let order: Vec<String> = model
        .computed_order()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(order, ["commande.ht", "commande.ttc", "client.total"]);
    assert!(model.formula(&ColumnRef::new("client", "total")).is_some());
}

#[test]
fn circular_dependencies() {
    let mut schema = base();
    let columns = schema["tables"][1]["columns"].as_array_mut().unwrap();
    columns.push(json!({ "name": "a", "type": "decimal", "formula": "b + 1" }));
    columns.push(json!({ "name": "b", "type": "decimal", "formula": "client.total" }));
    schema["tables"][0]["columns"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "name": "total", "type": "decimal", "formula": "SUM(commandes.a)" }));
    assert_issue(
        &schema,
        "tables[0].columns[2].formula",
        "dépendance circulaire : client.total → commande.a → commande.b → client.total",
    );
}

// ------------------------------------------------------------ lookups

#[test]
fn lookups() {
    let schema = with_column(
        1,
        json!({ "name": "categorie", "type": "lookup", "path": "client.categorie" }),
    );
    let model = load(&schema).unwrap();
    assert_eq!(
        model.lookup_path(&ColumnRef::new("commande", "categorie")),
        Some(&["client".to_owned(), "categorie".to_owned()][..])
    );

    let cases = [
        ("montant", "chemin attendu"),
        ("client.nom + 1", "chemin attendu"),
        ("produits.prix", "une seule valeur"),
        ("client.inconnu", "de la table `client`"),
    ];
    for (path, fragment) in cases {
        let schema = with_column(1, json!({ "name": "x", "type": "lookup", "path": path }));
        assert_issue(&schema, "tables[1].columns[4].path", fragment);
    }
}

// ------------------------------------------------------------ vues

#[test]
fn views() {
    let mut schema = base();
    schema["tables"][1]["views"] = json!({
        "calendar": { "start": "livraison" },
        "stats": { "fields": ["montant"], "group_by": "client" }
    });
    assert!(issues(&schema).is_empty(), "{:#?}", issues(&schema));

    // Un lookup prend le type de la colonne visée.
    let mut schema = with_column(
        1,
        json!({ "name": "categorie", "type": "lookup", "path": "client.categorie" }),
    );
    schema["tables"][1]["views"] =
        json!({ "stats": { "fields": ["montant"], "group_by": "categorie" } });
    assert!(issues(&schema).is_empty(), "{:#?}", issues(&schema));

    let mut schema = base();
    schema["tables"][1]["views"] = json!({
        "calendar": { "start": "montant", "end": "inconnue", "duration": "livraison" },
        "stats": { "fields": [], "group_by": "montant" }
    });
    assert_issue(
        &schema,
        "tables[1].views.calendar.start",
        "attendu : date ou datetime",
    );
    assert_issue(
        &schema,
        "tables[1].views.calendar.end",
        "colonne `inconnue` inconnue",
    );
    assert_issue(
        &schema,
        "tables[1].views.calendar.duration",
        "attendu : duration",
    );
    assert_issue(&schema, "tables[1].views.calendar", "exclusifs");
    assert_issue(
        &schema,
        "tables[1].views.stats.fields",
        "au moins une colonne",
    );
    assert_issue(&schema, "tables[1].views.stats.group_by", "attendu : enum");
}

// ------------------------------------------------------------ règles

#[test]
fn rules() {
    let mut schema = base();
    schema["tables"][1]["rules"] = json!([
        { "roles": ["admin"], "actions": ["*"] },
        { "roles": ["vendeur"], "actions": ["update"], "when": "owner == $user.id AND montant < 1000" }
    ]);
    let model = load(&schema).unwrap();
    assert!(model.condition("commande", 1).is_some());
    assert!(model.condition("commande", 0).is_none());

    let cases = [
        (
            json!({ "roles": ["inconnu"], "actions": ["read"] }),
            "roles[0]",
            "non déclaré",
        ),
        (
            json!({ "roles": [], "actions": ["read"] }),
            "roles",
            "au moins un rôle",
        ),
        (
            json!({ "roles": ["admin"], "actions": [] }),
            "actions",
            "au moins une action",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["*", "read"] }),
            "actions",
            "couvre déjà",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["read"], "when": "owner ==" }),
            "when",
            "syntaxe",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["read"], "when": "client.nom == \"x\"" }),
            "when",
            "colonnes stockées",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["read"], "when": "produits == 1" }),
            "when",
            "colonnes stockées",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["read"], "when": "$user.nom == \"x\"" }),
            "when",
            "champs disponibles",
        ),
        (
            json!({ "roles": ["admin"], "actions": ["read"], "when": "ROUND(montant) > 1" }),
            "when",
            "fonctions",
        ),
    ];
    for (rule, path, fragment) in cases {
        let mut schema = base();
        schema["tables"][1]["rules"] = json!([rule]);
        assert_issue(&schema, &format!("tables[1].rules[0].{path}"), fragment);
    }

    let mut schema = base();
    schema["tables"][1]["rules"] = json!([{ "roles": ["admin"], "actions": ["delete", "lire"] }]);
    assert_issue(
        &schema,
        "tables[1].rules[0].actions[1]",
        "unknown variant `lire`",
    );
}

#[test]
fn rule_condition_cannot_use_unpersisted_formula() {
    let mut schema = with_formula(1, "montant * 2");
    schema["tables"][1]["rules"] =
        json!([{ "roles": ["admin"], "actions": ["read"], "when": "calcul > 10" }]);
    assert_issue(&schema, "tables[1].rules[0].when", "colonnes stockées");

    schema["tables"][1]["columns"][4]["persist"] = json!(true);
    assert!(issues(&schema).is_empty(), "{:#?}", issues(&schema));
}

#[test]
fn join_tables() {
    let model = load(&base()).unwrap();
    assert_eq!(
        model.relations()[1].join_table().as_deref(),
        Some("commande_produits")
    );
    assert_eq!(model.relations()[0].join_table(), None);

    let mut schema = base();
    schema["tables"].as_array_mut().unwrap().push(
        json!({ "name": "commande_produits", "columns": [{ "name": "x", "type": "string" }] }),
    );
    assert_issue(
        &schema,
        "tables[1].columns[3]",
        "table de jointure `commande_produits`",
    );
}

#[test]
fn required_reference_cycles() {
    let mut schema = base();
    schema["tables"][1]["columns"][0]["required"] = json!(true);
    schema["tables"][0]["columns"].as_array_mut().unwrap().push(
        json!({ "name": "derniere", "type": "reference", "target": "commande", "required": true, "inverse": "derniere_de" }),
    );
    assert_issue(
        &schema,
        "tables[0]",
        "références obligatoires circulaires (client → commande → client)",
    );

    // Une seule des deux références obligatoire : insertion possible.
    schema["tables"][0]["columns"][2]["required"] = json!(false);
    assert!(issues(&schema).is_empty(), "{:#?}", issues(&schema));

    let schema = with_column(
        0,
        json!({ "name": "parent", "type": "reference", "target": "client", "required": true }),
    );
    assert_issue(&schema, "tables[0]", "client → client");
}
