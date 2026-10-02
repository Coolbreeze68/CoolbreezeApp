//! Table système `parameters` : valeurs globales typées, déclarées dans le schéma.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `GET` | `/api/parameters` | tous les paramètres |
//! | `GET` | `/api/parameters/{name}` | un paramètre |
//! | `PUT` | `/api/parameters/{name}` | modifie la valeur : `{ "value": … }` (admin) |
//!
//! Les valeurs sont stockées en JSON texte.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use forge_schema::spec::{Label, Parameter};
use forge_schema::value;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, EntityTrait, TransactionTrait};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use std::collections::BTreeMap;

use forge_schema::spec::ColumnType;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::compute::{self, Changes};
use crate::error::Error;

pub(crate) mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "parameters")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub name: String,
        #[sea_orm(column_type = "Text", nullable)]
        pub value: Option<String>,
        pub updated_at: DateTimeUtc,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/parameters", get(list))
        .route("/api/parameters/{name}", get(read).put(write))
}

/// Crée les paramètres déclarés absents de la base, avec leur valeur par défaut.
pub(crate) async fn seed(db: &impl ConnectionTrait, declared: &[Parameter]) -> Result<(), Error> {
    for param in declared {
        if entity::Entity::find_by_id(&param.name)
            .one(db)
            .await?
            .is_none()
        {
            entity::ActiveModel {
                name: Set(param.name.clone()),
                value: Set(param.default.as_ref().map(JsonValue::to_string)),
                updated_at: Set(chrono::Utc::now()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// Valeurs actuelles des paramètres déclarés, avec leur type.
pub(crate) async fn values(
    db: &impl ConnectionTrait,
    declared: &[Parameter],
) -> Result<BTreeMap<String, (ColumnType, JsonValue)>, Error> {
    if declared.is_empty() {
        return Ok(BTreeMap::new());
    }
    let stored = entity::Entity::find().all(db).await?;
    Ok(declared
        .iter()
        .map(|p| {
            let value = stored
                .iter()
                .find(|m| m.name == p.name)
                .and_then(|m| m.value.as_deref())
                .and_then(|v| serde_json::from_str(v).ok())
                .unwrap_or(JsonValue::Null);
            (p.name.clone(), (p.ty, value))
        })
        .collect())
}

fn declared<'a>(state: &'a AppState, name: &str) -> Result<&'a Parameter, Error> {
    state
        .model
        .spec()
        .parameters
        .iter()
        .find(|p| p.name == name)
        .ok_or(Error::NotFound)
}

fn to_json(param: &Parameter, stored: Option<&entity::Model>) -> JsonValue {
    let value = stored
        .and_then(|m| m.value.as_deref())
        .and_then(|v| serde_json::from_str(v).ok())
        .unwrap_or(JsonValue::Null);
    let label = match &param.label {
        Some(Label::Plain(text)) => json!(text),
        Some(Label::Localized(map)) => json!(map),
        None => JsonValue::Null,
    };
    json!({ "name": param.name, "type": param.ty.name(), "label": label, "value": value })
}

/// Tous les paramètres déclarés, avec leur valeur (REST et GraphQL).
pub(crate) async fn all(state: &AppState) -> Result<JsonValue, Error> {
    let stored = entity::Entity::find().all(&state.db).await?;
    let params: Vec<_> = state
        .model
        .spec()
        .parameters
        .iter()
        .map(|p| to_json(p, stored.iter().find(|m| m.name == p.name)))
        .collect();
    Ok(json!(params))
}

/// Modifie un paramètre (administrateur) et recalcule les formules persistées
/// qui le lisent.
pub(crate) async fn set(
    state: &AppState,
    current: &CurrentUser,
    name: &str,
    value: JsonValue,
) -> Result<JsonValue, Error> {
    current.require_admin()?;
    let param = declared(state, name)?;
    value::from_json(param.ty, None, &value).map_err(|msg| Error::validation("value", msg))?;
    let stored = entity::ActiveModel {
        name: Set(name.to_owned()),
        value: Set(Some(value.to_string())),
        updated_at: Set(chrono::Utc::now()),
    };
    // Le paramètre existe toujours : `seed` le crée au démarrage.
    let txn = state.db.begin().await?;
    let stored = stored.update(&txn).await?;
    compute::propagate(
        &txn,
        &state.model,
        &state.functions,
        Changes::parameter(name),
    )
    .await?;
    txn.commit().await?;
    Ok(to_json(param, Some(&stored)))
}

async fn list(State(state): State<AppState>) -> Result<Json<JsonValue>, Error> {
    Ok(Json(all(&state).await?))
}

async fn read(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<JsonValue>, Error> {
    let param = declared(&state, &name)?;
    let stored = entity::Entity::find_by_id(&name).one(&state.db).await?;
    Ok(Json(to_json(param, stored.as_ref())))
}

#[derive(Deserialize)]
struct Update {
    value: JsonValue,
}

async fn write(
    State(state): State<AppState>,
    current: CurrentUser,
    Path(name): Path<String>,
    Json(body): Json<Update>,
) -> Result<Json<JsonValue>, Error> {
    Ok(Json(set(&state, &current, &name, body.value).await?))
}
