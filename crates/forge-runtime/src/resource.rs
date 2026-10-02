//! Endpoints CRUD génériques d'une table.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `GET` | `/api/<table>` | liste paginée, triée, filtrée (voir [`crate::query`]) |
//! | `POST` | `/api/<table>` | création → `201` |
//! | `GET` | `/api/<table>/{id}` | lecture |
//! | `PATCH` | `/api/<table>/{id}` | modification partielle |
//! | `DELETE` | `/api/<table>/{id}` | suppression → `204` |

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::TypedValue;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, EntityTrait, IntoActiveModel, ModelTrait, PaginatorTrait,
    PrimaryKeyTrait, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::app::AppState;
use crate::error::Error;
use crate::hooks::{HookContext, Hooks};
use crate::payload::{self, Mode, Payload};
use crate::query::ListQuery;
use crate::{links, values};

/// Contraintes communes aux entités générées par forge (clé primaire `id: i64`).
pub trait ForgeEntity:
    EntityTrait<
        Model: Serialize + IntoActiveModel<Self::ActiveModel> + Send + Sync,
        ActiveModel: Send + Sync,
        PrimaryKey: PrimaryKeyTrait<ValueType = i64>,
    >
{
}

impl<E> ForgeEntity for E where
    E: EntityTrait<
            Model: Serialize + IntoActiveModel<E::ActiveModel> + Send + Sync,
            ActiveModel: Send + Sync,
            PrimaryKey: PrimaryKeyTrait<ValueType = i64>,
        >
{
}

/// Colonne d'entité par son nom. Le nom a été validé contre le schéma, à partir
/// duquel les entités sont générées : un échec signale du code non régénéré.
pub(crate) fn column_of<E: EntityTrait>(name: &str) -> E::Column {
    E::Column::from_str(name).unwrap_or_else(|_| {
        panic!("colonne `{name}` absente de l'entité générée : lancez `forge generate`")
    })
}

struct Resource<E, H> {
    table: String,
    hooks: H,
    entity: PhantomData<fn() -> E>,
}

/// Routes CRUD de l'entité `E`.
pub(crate) fn router<E: ForgeEntity, H: Hooks<E>>(hooks: H) -> (String, Router<AppState>) {
    let table = E::default().table_name().to_owned();
    let resource = Arc::new(Resource {
        table: table.clone(),
        hooks,
        entity: PhantomData::<fn() -> E>,
    });
    let (r1, r2, r3, r4, r5) = (
        resource.clone(),
        resource.clone(),
        resource.clone(),
        resource.clone(),
        resource,
    );
    let router = Router::new()
        .route(
            &format!("/api/{table}"),
            get(move |state, query| list(r1, state, query))
                .post(move |state, body| create(r2, state, body)),
        )
        .route(
            &format!("/api/{table}/{{id}}"),
            get(move |state, id| read(r3, state, id))
                .patch(move |state, id, body| update(r4, state, id, body))
                .delete(move |state, id| delete(r5, state, id)),
        );
    (table, router)
}

impl<E: ForgeEntity, H: Hooks<E>> Resource<E, H> {
    fn table<'a>(&self, state: &'a AppState) -> &'a Table {
        state
            .model
            .table(&self.table)
            .expect("table enregistrée à partir du schéma")
    }

    /// Colonnes `reference_list` de la table et leur table de jointure.
    fn join_tables(&self, state: &AppState) -> Vec<(String, String)> {
        state
            .model
            .relations()
            .iter()
            .filter(|r| r.source.table == self.table)
            .filter_map(|r| Some((r.source.column.clone(), r.join_table()?)))
            .collect()
    }

    /// Sérialise des enregistrements et y ajoute leurs `reference_list`.
    async fn to_json(
        &self,
        state: &AppState,
        db: &impl ConnectionTrait,
        models: Vec<E::Model>,
    ) -> Result<Vec<JsonValue>, Error> {
        let ids: Vec<i64> = models.iter().map(id_of::<E>).collect();
        let mut records = models
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| Error::Config(format!("sérialisation : {err}")))?;
        for (column, join) in self.join_tables(state) {
            let mut linked = links::load(db, &join, &ids).await?;
            for (record, id) in records.iter_mut().zip(&ids) {
                record[&column] = json!(linked.remove(id).unwrap_or_default());
            }
        }
        Ok(records)
    }

    async fn one_json(
        &self,
        state: &AppState,
        db: &impl ConnectionTrait,
        model: E::Model,
    ) -> Result<JsonValue, Error> {
        let mut records = self.to_json(state, db, vec![model]).await?;
        Ok(records.remove(0))
    }

    async fn find(&self, db: &impl ConnectionTrait, id: i64) -> Result<E::Model, Error> {
        E::find_by_id(id).one(db).await?.ok_or(Error::NotFound)
    }

    async fn save_links(
        &self,
        state: &AppState,
        db: &impl ConnectionTrait,
        id: i64,
        links: Vec<(String, Vec<i64>)>,
    ) -> Result<(), Error> {
        let joins: BTreeMap<_, _> = self.join_tables(state).into_iter().collect();
        for (column, targets) in links {
            links::replace(db, &joins[&column], id, &targets).await?;
        }
        Ok(())
    }
}

fn id_of<E: ForgeEntity>(model: &E::Model) -> i64 {
    match model.get(column_of::<E>("id")) {
        sea_orm::Value::BigInt(Some(id)) => id,
        other => panic!("identifiant inattendu : {other:?}"),
    }
}

/// Affecte des valeurs validées à un `ActiveModel`.
fn assign<E: ForgeEntity>(
    record: &mut E::ActiveModel,
    values: Vec<(String, ColumnType, TypedValue)>,
) -> Result<(), Error> {
    for (name, ty, value) in values {
        record.try_set(column_of::<E>(&name), values::to_db(ty, value))?;
    }
    Ok(())
}

fn touch<E: ForgeEntity>(record: &mut E::ActiveModel, created: bool) -> Result<(), Error> {
    let now = TypedValue::Datetime(chrono::Utc::now());
    let mut stamps = vec![("updated_at".to_owned(), ColumnType::Datetime, now.clone())];
    if created {
        stamps.push(("created_at".to_owned(), ColumnType::Datetime, now));
    }
    assign::<E>(record, stamps)
}

async fn list<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, Error> {
    let table = resource.table(&state);
    let query = ListQuery::parse(table, raw.as_deref())?;
    let select = query.apply(table, E::find());
    let total = select.clone().count(&state.db).await?;
    let models = select
        .offset((query.page - 1) * query.per_page)
        .limit(query.per_page)
        .all(&state.db)
        .await?;
    let data = resource.to_json(&state, &state.db, models).await?;
    Ok(Json(json!({
        "data": data,
        "page": query.page,
        "per_page": query.per_page,
        "total": total,
    })))
}

async fn read<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<JsonValue>, Error> {
    let model = resource.find(&state.db, id).await?;
    Ok(Json(resource.one_json(&state, &state.db, model).await?))
}

async fn create<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    Json(body): Json<JsonValue>,
) -> Result<(StatusCode, Json<JsonValue>), Error> {
    let Payload { values, links } = payload::parse(resource.table(&state), body, Mode::Create)?;
    let txn = state.db.begin().await?;
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
    };

    let mut record = <E::ActiveModel as ActiveModelTrait>::default();
    assign::<E>(&mut record, values)?;
    touch::<E>(&mut record, true)?;
    resource.hooks.before_create(&ctx, &mut record).await?;
    resource.hooks.validate(&ctx, &record).await?;
    let model = record.insert(&txn).await?;
    resource
        .save_links(&state, &txn, id_of::<E>(&model), links)
        .await?;
    resource.hooks.after_create(&ctx, &model).await?;
    let json = resource.one_json(&state, &txn, model).await?;
    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(json)))
}

async fn update<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<JsonValue>,
) -> Result<Json<JsonValue>, Error> {
    let Payload { values, links } = payload::parse(resource.table(&state), body, Mode::Update)?;
    let txn = state.db.begin().await?;
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
    };

    let mut record = resource.find(&txn, id).await?.into_active_model();
    assign::<E>(&mut record, values)?;
    touch::<E>(&mut record, false)?;
    resource.hooks.before_update(&ctx, &mut record).await?;
    resource.hooks.validate(&ctx, &record).await?;
    let model = record.update(&txn).await?;
    resource.save_links(&state, &txn, id, links).await?;
    resource.hooks.after_update(&ctx, &model).await?;
    let json = resource.one_json(&state, &txn, model).await?;
    txn.commit().await?;
    Ok(Json(json))
}

async fn delete<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, Error> {
    let txn = state.db.begin().await?;
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
    };
    let model = resource.find(&txn, id).await?;
    resource.hooks.before_delete(&ctx, &model).await?;
    E::delete_by_id(id).exec(&txn).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
