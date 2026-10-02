//! Endpoints CRUD génériques d'une table.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `GET` | `/api/<table>` | liste paginée, triée, filtrée (voir [`crate::query`]) |
//! | `POST` | `/api/<table>` | création → `201` |
//! | `GET` | `/api/<table>/aggregate` | agrégats (voir [`crate::aggregate`]) |
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
use forge_schema::spec::{Action, ColumnType, Table};
use forge_schema::value::TypedValue;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, EntityTrait, IntoActiveModel, ModelTrait, PaginatorTrait,
    PrimaryKeyTrait, QueryFilter, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::aggregate::AggregateQuery;
use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::Error;
use crate::hooks::{HookContext, Hooks};
use crate::payload::{self, Mode, Payload};
use crate::query::ListQuery;
use crate::rules::{self, Scope};
use crate::{compute, links, values};

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
    let (r1, r2, r3, r4, r5, r6) = (
        resource.clone(),
        resource.clone(),
        resource.clone(),
        resource.clone(),
        resource.clone(),
        resource,
    );
    let router = Router::new()
        .route(
            &format!("/api/{table}"),
            get(move |state, user, query| list(r1, state, user, query))
                .post(move |state, user, body| create(r2, state, user, body)),
        )
        .route(
            &format!("/api/{table}/aggregate"),
            get(move |state, user, query| aggregate(r6, state, user, query)),
        )
        .route(
            &format!("/api/{table}/{{id}}"),
            get(move |state, user, id| read(r3, state, user, id))
                .patch(move |state, user, id, body| update(r4, state, user, id, body))
                .delete(move |state, user, id| delete(r5, state, user, id)),
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

    /// Sérialise des enregistrements, avec leurs `reference_list`, lookups et
    /// formules non persistées.
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
        let table = self.table(state);
        compute::complete(db, &state.model, &state.functions, table, &mut records).await?;
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

    /// Enregistrement `id`, s'il existe et est lisible : sinon 404, sans révéler
    /// l'existence d'un enregistrement hors du périmètre de lecture.
    async fn find(
        &self,
        db: &impl ConnectionTrait,
        id: i64,
        readable: &Scope,
    ) -> Result<E::Model, Error> {
        let mut select = E::find_by_id(id);
        if let Some(condition) = readable.condition() {
            select = select.filter(condition);
        }
        select.one(db).await?.ok_or(Error::NotFound)
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

/// Horodatage ; à la création (`owner` renseigné), date de création et propriétaire.
fn stamp<E: ForgeEntity>(record: &mut E::ActiveModel, owner: Option<i64>) -> Result<(), Error> {
    let now = TypedValue::Datetime(chrono::Utc::now());
    let mut stamps = vec![("updated_at".to_owned(), ColumnType::Datetime, now.clone())];
    if let Some(owner) = owner {
        stamps.push(("created_at".to_owned(), ColumnType::Datetime, now));
        stamps.push((
            "owner".to_owned(),
            ColumnType::Reference,
            TypedValue::Integer(owner),
        ));
    }
    assign::<E>(record, stamps)
}

/// La table a-t-elle des formules persistées (à relire après écriture) ?
fn model_has_persisted(table: &Table) -> bool {
    table
        .columns
        .iter()
        .any(|c| c.formula.is_some() && c.persist)
}

/// Refus d'une modification qui sortirait l'enregistrement du périmètre autorisé.
fn out_of_scope(table: &str) -> Error {
    Error::Forbidden(format!(
        "enregistrement de `{table}` hors de votre périmètre"
    ))
}

async fn list<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, Error> {
    let table = resource.table(&state);
    let scope = rules::scope(&state, table, Action::Read, &user).await?;
    let query = ListQuery::parse(table, raw.as_deref())?;
    let mut select = query.apply(table, E::find());
    if let Some(condition) = scope.condition() {
        select = select.filter(condition);
    }
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

async fn aggregate<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, Error> {
    let table = resource.table(&state);
    let scope = rules::scope(&state, table, Action::Read, &user).await?;
    let query = AggregateQuery::parse(table, raw.as_deref())?;
    Ok(Json(query.run::<E>(&state.db, table, &scope).await?))
}

async fn read<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<JsonValue>, Error> {
    let readable = rules::scope(&state, resource.table(&state), Action::Read, &user).await?;
    let model = resource.find(&state.db, id, &readable).await?;
    Ok(Json(resource.one_json(&state, &state.db, model).await?))
}

async fn create<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<JsonValue>,
) -> Result<(StatusCode, Json<JsonValue>), Error> {
    let table = resource.table(&state);
    let allowed = rules::scope(&state, table, Action::Create, &user).await?;
    let Payload { values, links } = payload::parse(table, body, Mode::Create)?;
    let txn = state.db.begin().await?;
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
        user: &user,
    };

    let mut record = <E::ActiveModel as ActiveModelTrait>::default();
    assign::<E>(&mut record, values)?;
    stamp::<E>(&mut record, Some(user.id))?;
    resource.hooks.before_create(&ctx, &mut record).await?;
    resource.hooks.validate(&ctx, &record).await?;
    let model = record.insert(&txn).await?;
    let id = id_of::<E>(&model);
    // La condition de création porte sur l'enregistrement tel qu'il serait créé.
    if !rules::allows::<E>(&txn, &allowed, id).await? {
        return Err(out_of_scope(&table.name));
    }
    resource.save_links(&state, &txn, id, links).await?;
    let changes = compute::neighborhood(&txn, &state.model, &table.name, id, false).await?;
    compute::propagate(&txn, &state.model, &state.functions, changes).await?;
    // Relu : les formules persistées viennent d'être calculées.
    let model = resource.find(&txn, id, &Scope::All).await?;
    resource.hooks.after_create(&ctx, &model).await?;
    let json = resource.one_json(&state, &txn, model).await?;
    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(json)))
}

async fn update<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(body): Json<JsonValue>,
) -> Result<Json<JsonValue>, Error> {
    let table = resource.table(&state);
    // Portées calculées avant la transaction : elles lisent la base hors transaction.
    let readable = rules::scope(&state, table, Action::Read, &user).await?;
    let allowed = rules::scope(&state, table, Action::Update, &user).await?;
    let Payload { values, links } = payload::parse(table, body, Mode::Update)?;
    let txn = state.db.begin().await?;
    let existing = resource.find(&txn, id, &readable).await?;
    // Modifier exige le droit sur l'enregistrement, avant et après modification.
    if !rules::allows::<E>(&txn, &allowed, id).await? {
        return Err(out_of_scope(&table.name));
    }
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
        user: &user,
    };
    // Voisinage avant modification : références et liens qui vont peut-être changer.
    let mut changes = compute::neighborhood(&txn, &state.model, &table.name, id, false).await?;

    let mut record = existing.into_active_model();
    assign::<E>(&mut record, values)?;
    stamp::<E>(&mut record, None)?;
    resource.hooks.before_update(&ctx, &mut record).await?;
    resource.hooks.validate(&ctx, &record).await?;
    let model = record.update(&txn).await?;
    if !rules::allows::<E>(&txn, &allowed, id).await? {
        return Err(out_of_scope(&table.name));
    }
    resource.save_links(&state, &txn, id, links).await?;
    changes.merge(compute::neighborhood(&txn, &state.model, &table.name, id, false).await?);
    compute::propagate(&txn, &state.model, &state.functions, changes).await?;
    let model = if model_has_persisted(table) {
        resource.find(&txn, id, &Scope::All).await?
    } else {
        model
    };
    resource.hooks.after_update(&ctx, &model).await?;
    let json = resource.one_json(&state, &txn, model).await?;
    txn.commit().await?;
    Ok(Json(json))
}

async fn delete<E: ForgeEntity, H: Hooks<E>>(
    resource: Arc<Resource<E, H>>,
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, Error> {
    let table = resource.table(&state);
    let readable = rules::scope(&state, table, Action::Read, &user).await?;
    let allowed = rules::scope(&state, table, Action::Delete, &user).await?;
    let txn = state.db.begin().await?;
    let model = resource.find(&txn, id, &readable).await?;
    if !rules::allows::<E>(&txn, &allowed, id).await? {
        return Err(out_of_scope(&table.name));
    }
    let ctx = HookContext {
        txn: &txn,
        model: &state.model,
        user: &user,
    };
    resource.hooks.before_delete(&ctx, &model).await?;
    // Capturé avant suppression : ce qui référence l'enregistrement va changer.
    let changes = compute::neighborhood(&txn, &state.model, &table.name, id, true).await?;
    E::delete_by_id(id).exec(&txn).await?;
    compute::propagate(&txn, &state.model, &state.functions, changes).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
