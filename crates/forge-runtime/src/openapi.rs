//! Documentation OpenAPI 3.1, produite au démarrage à partir du schéma.
//!
//! | Chemin | Contenu |
//! |---|---|
//! | `/openapi.json` | le document OpenAPI |
//! | `/docs` | Swagger UI (bouton « Authorize » : jeton d'accès de `/api/auth/login`) |
//!
//! Le document est écrit en JSON, puis chargé dans le modèle d'`utoipa` qui le
//! sert avec Swagger UI : les routes étant dynamiques, il n'y a pas de types Rust
//! à annoter.

use axum::Router;
use forge_schema::graphql as names;
use forge_schema::spec::{Column, ColumnType, Label, Table};
use forge_schema::{ColumnRef, Model};
use serde_json::{Map, Value as JsonValue, json};
use utoipa::openapi::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::app::AppState;
use crate::compute;
use crate::error::Error;
use crate::query::{DEFAULT_PER_PAGE, MAX_PER_PAGE};

/// Routes `/docs` et `/openapi.json`.
pub(crate) fn router(model: &Model) -> Result<Router<AppState>, Error> {
    let document: OpenApi = serde_json::from_value(document(model))
        .map_err(|err| Error::Config(format!("document OpenAPI : {err}")))?;
    Ok(SwaggerUi::new("/docs")
        .url("/openapi.json", document)
        .into())
}

/// Document OpenAPI complet.
pub(crate) fn document(model: &Model) -> JsonValue {
    let spec = model.spec();
    let locale = &spec.app.default_locale;
    let mut paths = Map::new();
    let mut schemas = Map::new();
    let mut tags = Vec::new();

    for table in model.tables() {
        let name = &table.name;
        let tag = label(table.label.as_ref(), locale).unwrap_or_else(|| name.clone());
        tags.push(json!({ "name": tag }));
        schemas.insert(names::object(name), record_schema(model, table));
        schemas.insert(names::input(name), input_schema(table));
        schemas.insert(
            names::page(name),
            page_schema(&format!("#/components/schemas/{}", names::object(name))),
        );
        for (path, item) in table_paths(table, &tag) {
            paths.insert(path, item);
        }
    }
    schemas.insert("Error".into(), error_schema());
    for (path, item) in system_paths() {
        paths.insert(path, item);
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": spec.app.name,
            "version": "1.0.0",
            "description": "API générée par forge. Toutes les routes exigent un jeton d'accès \
                (`Authorization: Bearer …`), sauf la connexion et le rafraîchissement.",
        },
        "tags": tags,
        "paths": paths,
        "components": {
            "schemas": schemas,
            "securitySchemes": {
                "bearer": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" },
            },
        },
        "security": [{ "bearer": [] }],
    })
}

fn label(label: Option<&Label>, locale: &str) -> Option<String> {
    match label? {
        Label::Plain(text) => Some(text.clone()),
        Label::Localized(map) => map.get(locale).cloned(),
    }
}

fn schema_ref(name: &str) -> JsonValue {
    json!({ "$ref": format!("#/components/schemas/{name}") })
}

/// Schéma JSON d'une valeur de type `ty`.
fn value_schema(ty: ColumnType, values: Option<&[String]>) -> JsonValue {
    match ty {
        ColumnType::String => json!({ "type": "string", "maxLength": 255 }),
        // Un lookup prend le type de la colonne visée (voir `column_schema`).
        ColumnType::Text | ColumnType::Lookup => json!({ "type": "string" }),
        ColumnType::Integer => json!({ "type": "integer", "format": "int64" }),
        ColumnType::Duration => {
            json!({ "type": "integer", "format": "int64", "minimum": 0, "description": "Durée en secondes." })
        }
        ColumnType::Decimal => json!({
            "type": "string", "format": "decimal", "examples": ["12.50"],
            "description": "Décimal exact, en texte (un nombre est accepté en entrée).",
        }),
        ColumnType::Boolean => json!({ "type": "boolean" }),
        ColumnType::Date => json!({ "type": "string", "format": "date" }),
        ColumnType::Datetime => json!({ "type": "string", "format": "date-time" }),
        ColumnType::Enum => json!({ "type": "string", "enum": values.unwrap_or_default() }),
        ColumnType::Reference => {
            json!({ "type": "integer", "format": "int64", "description": "Identifiant de l'enregistrement cible." })
        }
        ColumnType::ReferenceList => json!({
            "type": "array", "items": { "type": "integer", "format": "int64" },
            "description": "Identifiants des enregistrements liés.",
        }),
    }
}

/// Autorise `null` (OpenAPI 3.1 : `type` devient une liste).
fn nullable(mut schema: JsonValue) -> JsonValue {
    if let Some(ty) = schema.get("type").cloned() {
        schema["type"] = json!([ty, "null"]);
    }
    schema
}

fn column_schema(model: &Model, table: &Table, column: &Column) -> JsonValue {
    let ty = compute::result_type(model, &ColumnRef::new(&table.name, &column.name));
    let mut schema = value_schema(ty, column.values.as_deref());
    if let Some(text) = label(column.label.as_ref(), &model.spec().app.default_locale) {
        schema["title"] = json!(text);
    }
    if column.is_computed() {
        schema["readOnly"] = json!(true);
    }
    if column.required || column.ty == ColumnType::ReferenceList {
        schema
    } else {
        nullable(schema)
    }
}

fn record_schema(model: &Model, table: &Table) -> JsonValue {
    let id = json!({ "type": "integer", "format": "int64", "readOnly": true });
    let stamp = json!({ "type": "string", "format": "date-time", "readOnly": true });
    let mut properties = Map::new();
    properties.insert("id".into(), id);
    let mut required = vec!["id".to_owned()];
    for column in &table.columns {
        properties.insert(column.name.clone(), column_schema(model, table, column));
        if column.required {
            required.push(column.name.clone());
        }
    }
    properties.insert(
        "owner".into(),
        json!({ "type": ["integer", "null"], "format": "int64", "readOnly": true, "description": "Créateur." }),
    );
    properties.insert("created_at".into(), stamp.clone());
    properties.insert("updated_at".into(), stamp);
    required.extend(["created_at".to_owned(), "updated_at".to_owned()]);
    json!({ "type": "object", "properties": properties, "required": required })
}

/// Corps de création et de modification : les champs absents gardent leur
/// valeur (modification) ou prennent leur valeur par défaut (création).
fn input_schema(table: &Table) -> JsonValue {
    let mut properties = Map::new();
    let mut mandatory = Vec::new();
    for column in table.columns.iter().filter(|c| !c.is_computed()) {
        let mut schema = value_schema(column.ty, column.values.as_deref());
        if let Some(default) = &column.default {
            schema["default"] = default.clone();
        }
        if !column.required {
            schema = nullable(schema);
        } else if column.default.is_none() {
            mandatory.push(column.name.as_str());
        }
        properties.insert(column.name.clone(), schema);
    }
    let description = if mandatory.is_empty() {
        "Aucun champ obligatoire.".to_owned()
    } else {
        format!("Obligatoires à la création : {}.", mandatory.join(", "))
    };
    json!({
        "type": "object", "properties": properties,
        "additionalProperties": false, "description": description,
    })
}

fn page_schema(item: &str) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "data": { "type": "array", "items": { "$ref": item } },
            "page": { "type": "integer" },
            "per_page": { "type": "integer" },
            "total": { "type": "integer" },
        },
        "required": ["data", "page", "per_page", "total"],
    })
}

fn error_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": { "error": {
            "type": "object",
            "properties": {
                "code": { "type": "string", "examples": ["validation"] },
                "message": { "type": "string" },
                "fields": {
                    "type": "object",
                    "additionalProperties": { "type": "array", "items": { "type": "string" } },
                    "description": "Erreurs par champ (`validation`).",
                },
                "lines": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "Erreurs par ligne (`import`) : `line`, `code`, `message`, `fields`.",
                },
            },
            "required": ["code", "message"],
        } },
    })
}

// ------------------------------------------------------------------ chemins

fn json_body(schema: &JsonValue) -> JsonValue {
    json!({ "required": true, "content": { "application/json": { "schema": schema } } })
}

fn json_response(description: &str, schema: &JsonValue) -> JsonValue {
    json!({ "description": description, "content": { "application/json": { "schema": schema } } })
}

fn csv_content() -> JsonValue {
    json!({ "text/csv": { "schema": { "type": "string" } } })
}

/// Réponse de succès `status`, suivie des réponses d'erreur `errors`.
fn responses(status: &str, success: JsonValue, errors: &[(&str, &str)]) -> JsonValue {
    let mut responses = Map::from_iter([(status.to_owned(), success)]);
    for (code, description) in errors {
        let error = json_response(description, &schema_ref("Error"));
        responses.insert((*code).to_owned(), error);
    }
    JsonValue::Object(responses)
}

const UNAUTHORIZED: (&str, &str) = ("401", "Identifiants ou jeton invalides");
const DENIED: (&str, &str) = ("403", "Action non autorisée");
const MISSING: (&str, &str) = ("404", "Introuvable ou hors de votre périmètre");
const INVALID: (&str, &str) = ("422", "Données invalides (détail par champ)");
const CONFLICT: (&str, &str) = ("409", "Valeur unique déjà prise ou référence invalide");

/// Opération : étiquette, résumé et détails (`parameters`, `requestBody`, `responses`…).
fn operation(tag: &str, summary: &str, details: JsonValue) -> JsonValue {
    let mut operation = json!({ "tags": [tag], "summary": summary });
    if let (Some(target), JsonValue::Object(details)) = (operation.as_object_mut(), details) {
        target.extend(details);
    }
    operation
}

fn query_parameter(name: &str, schema: &JsonValue, description: &str) -> JsonValue {
    json!({ "name": name, "in": "query", "required": false, "schema": schema, "description": description })
}

fn id_parameter() -> JsonValue {
    json!({ "name": "id", "in": "path", "required": true, "schema": { "type": "integer", "format": "int64" } })
}

/// Recherche et filtres de liste, suivis de `extra`.
fn with_filters(extra: Vec<JsonValue>) -> Vec<JsonValue> {
    let mut parameters = extra;
    parameters.push(query_parameter(
        "q",
        &json!({ "type": "string" }),
        "Recherche (contient, sans casse) dans les colonnes texte.",
    ));
    parameters.push(json!({
        "name": "filtres", "in": "query", "required": false, "style": "form", "explode": true,
        "schema": { "type": "object", "additionalProperties": { "type": "string" } },
        "description": "Filtres par colonne stockée : `etape=gagne`, `montant[gte]=1000`. \
            Opérateurs : `eq`, `ne`, `lt`, `lte`, `gt`, `gte`, `like`, `in` (`a,b`), \
            `null` (`true`/`false`).",
    }));
    parameters
}

fn table_paths(table: &Table, tag: &str) -> Vec<(String, JsonValue)> {
    let name = &table.name;
    vec![
        (format!("/api/{name}"), collection_path(name, tag)),
        (format!("/api/{name}/aggregate"), aggregate_path(name, tag)),
        (format!("/api/{name}/export"), export_path(name, tag)),
        (format!("/api/{name}/import"), import_path(name, tag)),
        (format!("/api/{name}/{{id}}"), item_path(name, tag)),
    ]
}

fn collection_path(name: &str, tag: &str) -> JsonValue {
    let parameters = with_filters(vec![
        query_parameter(
            "page",
            &json!({ "type": "integer", "minimum": 1, "default": 1 }),
            "Page",
        ),
        query_parameter(
            "per_page",
            &json!({ "type": "integer", "minimum": 1, "maximum": MAX_PER_PAGE, "default": DEFAULT_PER_PAGE }),
            "Enregistrements par page",
        ),
        query_parameter(
            "sort",
            &json!({ "type": "string" }),
            "Tri : `-montant,titre` (`-` : décroissant).",
        ),
    ]);
    let page = json_response("Page d'enregistrements", &schema_ref(&names::page(name)));
    let created = json_response("Enregistrement créé", &schema_ref(&names::object(name)));
    json!({
        "get": operation(tag, &format!("Liste de `{name}`"), json!({
            "parameters": parameters,
            "responses": responses("200", page, &[DENIED, INVALID]),
        })),
        "post": operation(tag, &format!("Crée un enregistrement de `{name}`"), json!({
            "requestBody": json_body(&schema_ref(&names::input(name))),
            "responses": responses("201", created, &[DENIED, CONFLICT, INVALID]),
        })),
    })
}

fn aggregate_path(name: &str, tag: &str) -> JsonValue {
    let parameters = with_filters(vec![
        json!({
            "name": "fields", "in": "query", "required": true, "schema": { "type": "string" },
            "description": "Colonnes numériques stockées, séparées par des virgules.",
        }),
        query_parameter(
            "group_by",
            &json!({ "type": "string" }),
            "Colonne de regroupement.",
        ),
    ]);
    let result = json_response("Agrégats", &json!({ "type": "object" }));
    json!({
        "get": operation(tag, &format!("Agrégats de `{name}`"), json!({
            "description": "`count` et, par colonne de `fields`, `sum`, `avg`, `min`, `max`, \
                au total et par groupe.",
            "parameters": parameters,
            "responses": responses("200", result, &[DENIED, INVALID]),
        })),
    })
}

fn export_path(name: &str, tag: &str) -> JsonValue {
    let parameters = with_filters(vec![query_parameter(
        "delimiter",
        &json!({ "type": "string", "enum": [",", ";"], "default": "," }),
        "Séparateur de colonnes.",
    )]);
    let file = json!({ "description": "Fichier CSV", "content": csv_content() });
    json!({
        "get": operation(tag, &format!("Export CSV de `{name}`"), json!({
            "parameters": parameters,
            "responses": responses("200", file, &[DENIED, INVALID]),
        })),
    })
}

fn import_path(name: &str, tag: &str) -> JsonValue {
    let report = json_response(
        "Bilan",
        &json!({
            "type": "object",
            "properties": { "created": { "type": "integer" }, "updated": { "type": "integer" } },
        }),
    );
    json!({
        "post": operation(tag, &format!("Import CSV dans `{name}`"), json!({
            "description": "Tout ou rien. Une ligne avec `id` modifie, sans `id` crée ; les \
                colonnes calculées et système sont ignorées. En cas d'erreur, rien n'est \
                enregistré et `error.lines` détaille chaque ligne.",
            "requestBody": { "required": true, "content": csv_content() },
            "responses": responses("200", report, &[DENIED, INVALID]),
        })),
    })
}

fn item_path(name: &str, tag: &str) -> JsonValue {
    let record = schema_ref(&names::object(name));
    json!({
        "parameters": [id_parameter()],
        "get": operation(tag, &format!("Lit un enregistrement de `{name}`"), json!({
            "responses": responses("200", json_response("Enregistrement", &record), &[DENIED, MISSING]),
        })),
        "patch": operation(tag, &format!("Modifie un enregistrement de `{name}`"), json!({
            "requestBody": json_body(&schema_ref(&names::input(name))),
            "responses": responses(
                "200",
                json_response("Enregistrement modifié", &record),
                &[DENIED, MISSING, CONFLICT, INVALID],
            ),
        })),
        "delete": operation(tag, &format!("Supprime un enregistrement de `{name}`"), json!({
            "responses": responses(
                "204",
                json!({ "description": "Supprimé" }),
                &[DENIED, MISSING, ("409", "Enregistrement encore référencé")],
            ),
        })),
    })
}

/// Authentification, comptes, paramètres et GraphQL.
fn system_paths() -> Vec<(String, JsonValue)> {
    let mut paths = auth_paths();
    paths.extend(admin_paths());
    paths
}

fn auth_paths() -> Vec<(String, JsonValue)> {
    let credentials = json!({
        "type": "object",
        "properties": { "email": { "type": "string" }, "password": { "type": "string" } },
        "required": ["email", "password"],
    });
    let refresh = json!({
        "type": "object", "properties": { "refresh_token": { "type": "string" } },
        "required": ["refresh_token"],
    });
    let password = json!({
        "type": "object",
        "properties": {
            "current_password": { "type": "string" },
            "new_password": { "type": "string", "minLength": 8 },
        },
        "required": ["current_password", "new_password"],
    });
    let session = json_response(
        "Session",
        &json!({
            "type": "object",
            "properties": {
                "access_token": { "type": "string" }, "refresh_token": { "type": "string" },
                "token_type": { "type": "string" }, "expires_in": { "type": "integer" },
                "user": { "type": "object" },
            },
        }),
    );
    let public = |summary: &str, body: &JsonValue| {
        let mut op = operation(
            "auth",
            summary,
            json!({
                "requestBody": json_body(body),
                "responses": responses("200", session.clone(), &[UNAUTHORIZED]),
            }),
        );
        op["security"] = json!([]);
        op
    };
    vec![
        (
            "/api/auth/login".into(),
            json!({ "post": public("Connexion", &credentials) }),
        ),
        (
            "/api/auth/refresh".into(),
            json!({ "post": public(
            "Nouveau jeton d'accès (le jeton de rafraîchissement est remplacé)", &refresh,
        ) }),
        ),
        (
            "/api/auth/logout".into(),
            json!({ "post": operation("auth", "Déconnexion (révoque le jeton de rafraîchissement)", json!({
            "requestBody": json_body(&refresh),
            "responses": { "204": { "description": "Déconnecté" } },
        })) }),
        ),
        (
            "/api/auth/me".into(),
            json!({ "get": operation("auth", "Compte connecté", json!({
            "responses": { "200": json_response("Compte", &json!({ "type": "object" })) },
        })) }),
        ),
        (
            "/api/auth/password".into(),
            json!({ "put": operation("auth", "Change son mot de passe", json!({
            "requestBody": json_body(&password),
            "responses": responses("204", json!({ "description": "Modifié" }), &[INVALID]),
        })) }),
        ),
    ]
}

fn admin_paths() -> Vec<(String, JsonValue)> {
    let account = || json_response("Compte", &json!({ "type": "object" }));
    let parameter = || json_response("Paramètre", &json!({ "type": "object" }));
    let value = json!({
        "type": "object",
        "properties": { "value": {
            "type": ["string", "number", "integer", "boolean", "null"],
            "description": "Valeur, du type du paramètre.",
        } },
        "required": ["value"],
    });
    let graphql = json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" }, "variables": { "type": "object" },
            "operationName": { "type": "string" },
        },
        "required": ["query"],
    });
    let users_parameters = [
        query_parameter("page", &json!({ "type": "integer" }), "Page"),
        query_parameter(
            "per_page",
            &json!({ "type": "integer" }),
            "Comptes par page",
        ),
        query_parameter(
            "q",
            &json!({ "type": "string" }),
            "Recherche dans l'email et le nom",
        ),
    ];
    let page = json_response("Page de comptes", &json!({ "type": "object" }));
    vec![
        (
            "/api/users".into(),
            json!({
                "get": operation("users", "Comptes (administrateur)", json!({
                    "parameters": users_parameters,
                    "responses": responses("200", page, &[DENIED]),
                })),
                "post": operation("users", "Crée un compte (administrateur)", json!({
                    "requestBody": json_body(&account_schema()),
                    "responses": responses("201", account(), &[DENIED, CONFLICT, INVALID]),
                })),
            }),
        ),
        (
            "/api/users/{id}".into(),
            json!({
                "parameters": [id_parameter()],
                "get": operation("users", "Lit un compte (administrateur)", json!({
                    "responses": responses("200", account(), &[DENIED, MISSING]),
                })),
                "patch": operation("users", "Modifie un compte (administrateur)", json!({
                    "requestBody": json_body(&account_schema()),
                    "responses": responses("200", account(), &[DENIED, MISSING, INVALID]),
                })),
                "delete": operation("users", "Supprime un compte (administrateur)", json!({
                    "responses": responses("204", json!({ "description": "Supprimé" }), &[DENIED, MISSING]),
                })),
            }),
        ),
        (
            "/api/parameters".into(),
            json!({ "get": operation("parameters", "Paramètres et leurs valeurs", json!({
            "responses": { "200": json_response("Paramètres", &json!({ "type": "array", "items": { "type": "object" } })) },
        })) }),
        ),
        (
            "/api/parameters/{name}".into(),
            json!({
                "parameters": [{ "name": "name", "in": "path", "required": true, "schema": { "type": "string" } }],
                "get": operation("parameters", "Lit un paramètre", json!({
                    "responses": responses("200", parameter(), &[MISSING]),
                })),
                "put": operation("parameters", "Modifie un paramètre (administrateur)", json!({
                    "requestBody": json_body(&value),
                    "responses": responses("200", parameter(), &[DENIED, MISSING, INVALID]),
                })),
            }),
        ),
        (
            "/api/graphql".into(),
            json!({ "post": operation("graphql", "Requête GraphQL (éditeur interactif : /graphql)", json!({
            "requestBody": json_body(&graphql),
            "responses": { "200": json_response("Réponse GraphQL (`data`, `errors`)", &json!({ "type": "object" })) },
        })) }),
        ),
    ]
}

fn account_schema() -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "email": { "type": "string" }, "password": { "type": "string", "minLength": 8 },
            "display_name": { "type": ["string", "null"] }, "active": { "type": "boolean" },
            "roles": { "type": "array", "items": { "type": "string" } },
        },
        "additionalProperties": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model::from_json(
            &json!({
                "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
                "roles": ["admin"],
                "tables": [{ "name": "note", "label": "Note", "columns": [
                    { "name": "titre", "type": "string", "required": true },
                    { "name": "montant", "type": "decimal" },
                    { "name": "double", "type": "decimal", "formula": "montant * 2" },
                    { "name": "etape", "type": "enum", "values": ["a", "b"] }
                ] }]
            })
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn document_is_valid_for_utoipa() {
        let document = document(&model());
        let parsed: OpenApi = serde_json::from_value(document.clone()).unwrap();
        assert!(parsed.paths.paths.contains_key("/api/note/{id}"));

        let note = &document["components"]["schemas"]["Note"];
        assert_eq!(
            note["properties"]["montant"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(note["properties"]["double"]["readOnly"], json!(true));
        assert_eq!(note["properties"]["etape"]["enum"], json!(["a", "b"]));
        let input = &document["components"]["schemas"]["NoteInput"];
        assert!(
            input["properties"].get("double").is_none(),
            "colonne calculée"
        );
        assert_eq!(input["description"], "Obligatoires à la création : titre.");
        assert_eq!(
            document["paths"]["/api/auth/login"]["post"]["security"],
            json!([])
        );
    }
}
