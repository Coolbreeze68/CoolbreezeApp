//! Endpoints REST d'une table.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `GET` | `/api/<table>` | liste paginée, triée, filtrée (voir [`crate::query`]) |
//! | `POST` | `/api/<table>` | création → `201` |
//! | `GET` | `/api/<table>/aggregate` | agrégats (voir [`crate::aggregate`]) |
//! | `GET` | `/api/<table>/export` | export CSV (voir [`crate::csv_io`]) |
//! | `POST` | `/api/<table>/import` | import CSV, tout ou rien |
//! | `GET` | `/api/<table>/{id}` | lecture |
//! | `PATCH` | `/api/<table>/{id}` | modification partielle |
//! | `DELETE` | `/api/<table>/{id}` | suppression → `204` |

use std::sync::Arc;

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value as JsonValue, json};

use crate::aggregate::AggregateQuery;
use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::csv_io;
use crate::error::Error;
use crate::query::ListQuery;
use crate::resource::Service;

type Shared = Arc<dyn Service>;

/// Routes de la table servie par `service`.
pub(crate) fn router(service: &Shared) -> Router<AppState> {
    let table = service.name().to_owned();
    // Chaque handler garde sa propre référence au service.
    let s = || service.clone();
    let (list_s, create_s, aggregate_s, export_s, import_s) = (s(), s(), s(), s(), s());
    let (read_s, update_s, delete_s) = (s(), s(), s());
    Router::new()
        .route(
            &format!("/api/{table}"),
            get(move |state, user, query| list(list_s, state, user, query))
                .post(move |state, user, body| create(create_s, state, user, body)),
        )
        .route(
            &format!("/api/{table}/aggregate"),
            get(move |state, user, query| aggregate(aggregate_s, state, user, query)),
        )
        .route(
            &format!("/api/{table}/export"),
            get(move |state, user, query| csv_io::export(export_s, state, user, query)),
        )
        .route(
            &format!("/api/{table}/import"),
            post(move |state, user, body| csv_io::import(import_s, state, user, body)),
        )
        .route(
            &format!("/api/{table}/{{id}}"),
            get(move |state, user, id| read(read_s, state, user, id))
                .patch(move |state, user, id, body| update(update_s, state, user, id, body))
                .delete(move |state, user, id| delete(delete_s, state, user, id)),
        )
}

async fn list(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, Error> {
    let query = ListQuery::parse(state.table(service.name()), raw.as_deref())?;
    Ok(Json(json!(service.list(&state, &user, &query).await?)))
}

async fn aggregate(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, Error> {
    let query = AggregateQuery::parse(state.table(service.name()), raw.as_deref())?;
    Ok(Json(service.aggregate(&state, &user, &query).await?))
}

async fn read(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<JsonValue>, Error> {
    Ok(Json(service.read(&state, &user, id).await?))
}

async fn create(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<JsonValue>,
) -> Result<(StatusCode, Json<JsonValue>), Error> {
    let record = service.create(&state, &user, body).await?;
    Ok((StatusCode::CREATED, Json(record)))
}

async fn update(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(body): Json<JsonValue>,
) -> Result<Json<JsonValue>, Error> {
    Ok(Json(service.update(&state, &user, id, body).await?))
}

async fn delete(
    service: Shared,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, Error> {
    service.delete(&state, &user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
