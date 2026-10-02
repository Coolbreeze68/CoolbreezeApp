//! Opérations d'une table, indépendantes du transport.
//!
//! [`Service`] est implémenté une fois, de façon générique, pour chaque entité
//! générée ; REST ([`crate::rest`]), GraphQL ([`crate::graphql`]) et CSV
//! ([`crate::csv_io`]) passent tous par lui : mêmes validations, règles
//! d'autorisation, hooks et calculs, quel que soit le point d'entrée.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::str::FromStr;

use async_trait::async_trait;
use forge_schema::spec::{Action, ColumnType, Table};
use forge_schema::value::TypedValue;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait,
    IntoActiveModel, ModelTrait, PaginatorTrait, PrimaryKeyTrait, QueryFilter, QuerySelect,
    TransactionTrait,
};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::aggregate::AggregateQuery;
use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::{Error, RowError};
use crate::hooks::{HookContext, Hooks};
use crate::payload::{self, Mode, Payload};
use crate::query::ListQuery;
use crate::rules::{self, Scope};
use crate::{compute, links, values};

/// Taille des lots lus pour un export.
const EXPORT_CHUNK: u64 = 500;

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

/// Page d'une liste : enregistrements, numéro de page et total.
#[derive(Debug, Serialize)]
pub(crate) struct Listing {
    pub data: Vec<JsonValue>,
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
}

/// Ligne d'un import : `id` renseigné → modification, sinon création. `body`
/// porte déjà l'erreur si la ligne n'a pas pu être lue.
#[derive(Debug)]
pub(crate) struct ImportRow {
    pub line: u64,
    pub id: Option<i64>,
    pub body: Result<JsonValue, Error>,
}

/// Bilan d'un import réussi.
#[derive(Debug, Default, Serialize)]
pub(crate) struct ImportReport {
    pub created: u64,
    pub updated: u64,
}

/// Opérations d'une table pour l'utilisateur `user`, règles d'autorisation comprises.
#[async_trait]
pub(crate) trait Service: Send + Sync {
    /// Nom de la table.
    fn name(&self) -> &str;

    async fn list(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &ListQuery,
    ) -> Result<Listing, Error>;

    /// Tous les enregistrements lisibles correspondant aux filtres (pagination ignorée).
    async fn all(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &ListQuery,
    ) -> Result<Vec<JsonValue>, Error>;

    /// `404` si l'enregistrement n'existe pas ou est hors du périmètre de lecture.
    async fn read(&self, state: &AppState, user: &CurrentUser, id: i64)
    -> Result<JsonValue, Error>;

    /// Enregistrements lisibles parmi `ids` (les autres sont omis).
    async fn read_many(
        &self,
        state: &AppState,
        user: &CurrentUser,
        ids: &[i64],
    ) -> Result<Vec<JsonValue>, Error>;

    async fn aggregate(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &AggregateQuery,
    ) -> Result<JsonValue, Error>;

    async fn create(
        &self,
        state: &AppState,
        user: &CurrentUser,
        body: JsonValue,
    ) -> Result<JsonValue, Error>;

    async fn update(
        &self,
        state: &AppState,
        user: &CurrentUser,
        id: i64,
        body: JsonValue,
    ) -> Result<JsonValue, Error>;

    async fn delete(&self, state: &AppState, user: &CurrentUser, id: i64) -> Result<(), Error>;

    /// Tout ou rien : chaque ligne s'exécute comme une création ou une
    /// modification (hooks et règles compris) ; à la moindre erreur, rien n'est
    /// enregistré et toutes les erreurs sont rapportées ([`Error::Import`]).
    async fn import(
        &self,
        state: &AppState,
        user: &CurrentUser,
        rows: Vec<ImportRow>,
    ) -> Result<ImportReport, Error>;
}

/// Implémentation de [`Service`] pour l'entité `E` et ses hooks `H`.
pub(crate) struct Resource<E, H> {
    table: String,
    hooks: H,
    entity: PhantomData<fn() -> E>,
}

impl<E: ForgeEntity, H: Hooks<E>> Resource<E, H> {
    pub(crate) fn new(hooks: H) -> Self {
        Self {
            table: E::default().table_name().to_owned(),
            hooks,
            entity: PhantomData,
        }
    }

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
        normalize_decimals(table, &mut records);
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

    /// Création dans la transaction `txn` ; `allowed` : portée de création.
    async fn insert(
        &self,
        state: &AppState,
        txn: &DatabaseTransaction,
        user: &CurrentUser,
        allowed: &Scope,
        Payload { values, links }: Payload,
    ) -> Result<E::Model, Error> {
        let table = self.table(state);
        let ctx = HookContext {
            txn,
            model: &state.model,
            user,
        };
        let mut record = <E::ActiveModel as ActiveModelTrait>::default();
        assign::<E>(&mut record, values)?;
        stamp::<E>(&mut record, Some(user.id))?;
        self.hooks.before_create(&ctx, &mut record).await?;
        self.hooks.validate(&ctx, &record).await?;
        let model = record.insert(txn).await?;
        let id = id_of::<E>(&model);
        // La condition de création porte sur l'enregistrement tel qu'il serait créé.
        if !rules::allows::<E>(txn, allowed, id).await? {
            return Err(out_of_scope(&table.name));
        }
        self.save_links(state, txn, id, links).await?;
        let changes = compute::neighborhood(txn, &state.model, &table.name, id, false).await?;
        compute::propagate(txn, &state.model, &state.functions, changes).await?;
        // Relu : les formules persistées viennent d'être calculées.
        let model = self.find(txn, id, &Scope::All).await?;
        self.hooks.after_create(&ctx, &model).await?;
        Ok(model)
    }

    /// Modification dans la transaction `txn`.
    async fn modify(
        &self,
        state: &AppState,
        txn: &DatabaseTransaction,
        user: &CurrentUser,
        (readable, allowed): (&Scope, &Scope),
        id: i64,
        Payload { values, links }: Payload,
    ) -> Result<E::Model, Error> {
        let table = self.table(state);
        let existing = self.find(txn, id, readable).await?;
        // Modifier exige le droit sur l'enregistrement, avant et après modification.
        if !rules::allows::<E>(txn, allowed, id).await? {
            return Err(out_of_scope(&table.name));
        }
        let ctx = HookContext {
            txn,
            model: &state.model,
            user,
        };
        // Voisinage avant modification : références et liens qui vont peut-être changer.
        let mut changes = compute::neighborhood(txn, &state.model, &table.name, id, false).await?;

        let mut record = existing.into_active_model();
        assign::<E>(&mut record, values)?;
        stamp::<E>(&mut record, None)?;
        self.hooks.before_update(&ctx, &mut record).await?;
        self.hooks.validate(&ctx, &record).await?;
        let model = record.update(txn).await?;
        if !rules::allows::<E>(txn, allowed, id).await? {
            return Err(out_of_scope(&table.name));
        }
        self.save_links(state, txn, id, links).await?;
        changes.merge(compute::neighborhood(txn, &state.model, &table.name, id, false).await?);
        compute::propagate(txn, &state.model, &state.functions, changes).await?;
        let model = if model_has_persisted(table) {
            self.find(txn, id, &Scope::All).await?
        } else {
            model
        };
        self.hooks.after_update(&ctx, &model).await?;
        Ok(model)
    }

    /// Une ligne d'import, dans sa propre transaction imbriquée (point de
    /// sauvegarde) : son échec n'empêche pas de vérifier les suivantes.
    /// Retourne `true` pour une création.
    async fn import_row(
        &self,
        state: &AppState,
        txn: &DatabaseTransaction,
        user: &CurrentUser,
        scopes: &ImportScopes,
        row: ImportRow,
    ) -> Result<bool, Error> {
        let body = row.body?;
        let savepoint = txn.begin().await?;
        let result = self
            .write_row(state, &savepoint, user, scopes, row.id, body)
            .await;
        match result {
            Ok(_) => savepoint.commit().await?,
            Err(_) => savepoint.rollback().await?,
        }
        result
    }

    async fn write_row(
        &self,
        state: &AppState,
        txn: &DatabaseTransaction,
        user: &CurrentUser,
        scopes: &ImportScopes,
        id: Option<i64>,
        body: JsonValue,
    ) -> Result<bool, Error> {
        let table = self.table(state);
        match id {
            None => {
                let allowed = scopes
                    .create
                    .as_ref()
                    .ok_or_else(|| denied(table, "créer"))?;
                let payload = payload::parse(table, body, Mode::Create)?;
                self.insert(state, txn, user, allowed, payload).await?;
                Ok(true)
            }
            Some(id) => {
                let allowed = scopes
                    .update
                    .as_ref()
                    .ok_or_else(|| denied(table, "modifier"))?;
                let payload = payload::parse(table, body, Mode::Update)?;
                self.modify(state, txn, user, (&scopes.read, allowed), id, payload)
                    .await?;
                Ok(false)
            }
        }
    }
}

/// Portées calculées une fois pour tout l'import (`None` : action interdite).
struct ImportScopes {
    read: Scope,
    create: Option<Scope>,
    update: Option<Scope>,
}

/// Portée d'une action, ou `None` si l'utilisateur n'y a pas droit.
async fn optional_scope(
    state: &AppState,
    table: &Table,
    action: Action,
    user: &CurrentUser,
) -> Result<Option<Scope>, Error> {
    match rules::scope(state, table, action, user).await {
        Ok(scope) => Ok(Some(scope)),
        Err(Error::Forbidden(_)) => Ok(None),
        Err(err) => Err(err),
    }
}

fn denied(table: &Table, action: &str) -> Error {
    Error::Forbidden(format!(
        "vous n'avez pas le droit de {action} dans `{}`",
        table.name
    ))
}

#[async_trait]
impl<E: ForgeEntity, H: Hooks<E>> Service for Resource<E, H> {
    fn name(&self) -> &str {
        &self.table
    }

    async fn list(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &ListQuery,
    ) -> Result<Listing, Error> {
        let table = self.table(state);
        let scope = rules::scope(state, table, Action::Read, user).await?;
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
        Ok(Listing {
            data: self.to_json(state, &state.db, models).await?,
            page: query.page,
            per_page: query.per_page,
            total,
        })
    }

    async fn all(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &ListQuery,
    ) -> Result<Vec<JsonValue>, Error> {
        let table = self.table(state);
        let scope = rules::scope(state, table, Action::Read, user).await?;
        let mut select = query.apply(table, E::find());
        if let Some(condition) = scope.condition() {
            select = select.filter(condition);
        }
        let mut pages = select.paginate(&state.db, EXPORT_CHUNK);
        let mut records = Vec::new();
        while let Some(models) = pages.fetch_and_next().await? {
            records.extend(self.to_json(state, &state.db, models).await?);
        }
        Ok(records)
    }

    async fn read(
        &self,
        state: &AppState,
        user: &CurrentUser,
        id: i64,
    ) -> Result<JsonValue, Error> {
        let readable = rules::scope(state, self.table(state), Action::Read, user).await?;
        let model = self.find(&state.db, id, &readable).await?;
        self.one_json(state, &state.db, model).await
    }

    async fn read_many(
        &self,
        state: &AppState,
        user: &CurrentUser,
        ids: &[i64],
    ) -> Result<Vec<JsonValue>, Error> {
        let readable = rules::scope(state, self.table(state), Action::Read, user).await?;
        let mut select = E::find().filter(column_of::<E>("id").is_in(ids.iter().copied()));
        if let Some(condition) = readable.condition() {
            select = select.filter(condition);
        }
        let models = select.all(&state.db).await?;
        self.to_json(state, &state.db, models).await
    }

    async fn aggregate(
        &self,
        state: &AppState,
        user: &CurrentUser,
        query: &AggregateQuery,
    ) -> Result<JsonValue, Error> {
        let table = self.table(state);
        let scope = rules::scope(state, table, Action::Read, user).await?;
        query.run::<E>(&state.db, table, &scope).await
    }

    async fn create(
        &self,
        state: &AppState,
        user: &CurrentUser,
        body: JsonValue,
    ) -> Result<JsonValue, Error> {
        let table = self.table(state);
        // Portées calculées avant la transaction : elles lisent la base hors transaction.
        let allowed = rules::scope(state, table, Action::Create, user).await?;
        let payload = payload::parse(table, body, Mode::Create)?;
        let txn = state.db.begin().await?;
        let model = self.insert(state, &txn, user, &allowed, payload).await?;
        let json = self.one_json(state, &txn, model).await?;
        txn.commit().await?;
        Ok(json)
    }

    async fn update(
        &self,
        state: &AppState,
        user: &CurrentUser,
        id: i64,
        body: JsonValue,
    ) -> Result<JsonValue, Error> {
        let table = self.table(state);
        let readable = rules::scope(state, table, Action::Read, user).await?;
        let allowed = rules::scope(state, table, Action::Update, user).await?;
        let payload = payload::parse(table, body, Mode::Update)?;
        let txn = state.db.begin().await?;
        let model = self
            .modify(state, &txn, user, (&readable, &allowed), id, payload)
            .await?;
        let json = self.one_json(state, &txn, model).await?;
        txn.commit().await?;
        Ok(json)
    }

    async fn delete(&self, state: &AppState, user: &CurrentUser, id: i64) -> Result<(), Error> {
        let table = self.table(state);
        let readable = rules::scope(state, table, Action::Read, user).await?;
        let allowed = rules::scope(state, table, Action::Delete, user).await?;
        let txn = state.db.begin().await?;
        let model = self.find(&txn, id, &readable).await?;
        if !rules::allows::<E>(&txn, &allowed, id).await? {
            return Err(out_of_scope(&table.name));
        }
        let ctx = HookContext {
            txn: &txn,
            model: &state.model,
            user,
        };
        self.hooks.before_delete(&ctx, &model).await?;
        // Capturé avant suppression : ce qui référence l'enregistrement va changer.
        let changes = compute::neighborhood(&txn, &state.model, &table.name, id, true).await?;
        E::delete_by_id(id).exec(&txn).await?;
        compute::propagate(&txn, &state.model, &state.functions, changes).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn import(
        &self,
        state: &AppState,
        user: &CurrentUser,
        rows: Vec<ImportRow>,
    ) -> Result<ImportReport, Error> {
        let table = self.table(state);
        let scopes = ImportScopes {
            read: rules::scope(state, table, Action::Read, user).await?,
            create: optional_scope(state, table, Action::Create, user).await?,
            update: optional_scope(state, table, Action::Update, user).await?,
        };
        let txn = state.db.begin().await?;
        let mut report = ImportReport::default();
        let mut errors = Vec::new();
        for row in rows {
            let line = row.line;
            match self.import_row(state, &txn, user, &scopes, row).await {
                Ok(true) => report.created += 1,
                Ok(false) => report.updated += 1,
                // Une erreur interne interrompt l'import (la transaction est annulée).
                Err(err) if err.status().is_server_error() => return Err(err),
                Err(err) => errors.push(RowError::new(line, &err)),
            }
        }
        if !errors.is_empty() {
            txn.rollback().await?;
            return Err(Error::Import(errors));
        }
        txn.commit().await?;
        Ok(report)
    }
}

/// Décimaux sans zéros superflus : `"500"` quelle que soit la base
/// (PostgreSQL et MySQL renvoient `"500.0000"`).
fn normalize_decimals(table: &Table, records: &mut [JsonValue]) {
    let decimals: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.is_stored() && c.ty == ColumnType::Decimal)
        .map(|c| c.name.as_str())
        .collect();
    for record in records {
        for name in &decimals {
            if let Some(decimal) = record[*name]
                .as_str()
                .and_then(|s| Decimal::from_str(s).ok())
            {
                record[*name] = json!(decimal.normalize().to_string());
            }
        }
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
