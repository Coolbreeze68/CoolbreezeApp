//! Colonnes calculées : formules et lookups.
//!
//! - **À la lecture** ([`complete`]) : lookups et formules non persistées sont
//!   évalués pour les enregistrements renvoyés, données chargées par lots.
//! - **À l'écriture** ([`propagate`]) : les formules persistées des
//!   enregistrements touchés, directement ou par une relation, sont recalculées et
//!   stockées, dans la transaction de la requête, dans l'ordre des dépendances.
//!
//! Une modification se décrit par des [`Changes`] : les enregistrements touchés
//! et leur voisinage ([`neighborhood`]), ou les paramètres modifiés.

mod graph;
mod paths;

use std::collections::{BTreeMap, BTreeSet};

use forge_formula::{FunctionRegistry, Value};
use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::TypedValue;
use forge_schema::{ColumnRef, Model, RelationKind};
use rust_decimal::RoundingStrategy;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::ConnectionTrait;
use sea_orm::sea_query::{Alias, Expr, ExprTrait, Query};
use serde_json::{Value as JsonValue, json};

use crate::error::{Error, FieldErrors};
use crate::{links, values};
use graph::Graph;
use paths::Hop;

/// Décimales conservées pour une valeur `decimal` (comme en base).
const DECIMAL_PLACES: u32 = 4;

pub(crate) fn typed_to_value(typed: TypedValue) -> Value {
    match typed {
        TypedValue::Null => Value::Null,
        TypedValue::String(s) => Value::Text(s),
        TypedValue::Integer(i) => Value::from(i),
        TypedValue::Decimal(d) => Value::Number(d),
        TypedValue::Boolean(b) => Value::Boolean(b),
        TypedValue::Date(d) => Value::Date(d),
        TypedValue::Datetime(d) => Value::DateTime(d),
    }
}

/// Valeur à stocker dans une colonne de type `ty` (arrondie pour les nombres).
fn value_to_typed(ty: ColumnType, value: Value) -> TypedValue {
    match (ty, value) {
        (ColumnType::Integer | ColumnType::Duration | ColumnType::Reference, Value::Number(n)) => n
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .map_or(TypedValue::Null, TypedValue::Integer),
        (_, Value::Number(n)) => TypedValue::Decimal(
            n.round_dp_with_strategy(DECIMAL_PLACES, RoundingStrategy::MidpointAwayFromZero),
        ),
        (_, Value::Text(s)) => TypedValue::String(s),
        (_, Value::Boolean(b)) => TypedValue::Boolean(b),
        (_, Value::Date(d)) => TypedValue::Date(d),
        (_, Value::DateTime(d)) => TypedValue::Datetime(d),
        (_, Value::Null) => TypedValue::Null,
    }
}

/// Représentation JSON, cohérente avec celle des colonnes stockées
/// (décimaux en texte, dates ISO).
fn value_to_json(ty: ColumnType, value: Value) -> JsonValue {
    match value_to_typed(ty, value) {
        TypedValue::Null => JsonValue::Null,
        TypedValue::String(s) => json!(s),
        TypedValue::Integer(i) => json!(i),
        TypedValue::Decimal(d) => json!(d.normalize().to_string()),
        TypedValue::Boolean(b) => json!(b),
        TypedValue::Date(d) => json!(d),
        TypedValue::Datetime(d) => json!(d),
    }
}

/// Type des valeurs d'une colonne calculée : celui de la colonne, ou pour un
/// lookup celui de la colonne visée.
pub(crate) fn result_type(model: &Model, column: &ColumnRef) -> ColumnType {
    let mut current = column.clone();
    loop {
        let Some(found) = model.column(&current) else {
            return graph::stored_type(model, &current.table, &current.column)
                .unwrap_or(ColumnType::Text);
        };
        if found.ty != ColumnType::Lookup {
            return found.ty;
        }
        match model.dependencies(&current).and_then(|d| d.first()) {
            Some(target) => current = target.clone(),
            None => return ColumnType::Text,
        }
    }
}

/// Ajoute aux enregistrements JSON de `table` leurs lookups et formules non persistées.
///
/// Une erreur d'évaluation (fonction personnalisée en échec…) donne `null` et
/// est journalisée : la lecture ne doit pas échouer pour autant.
pub(crate) async fn complete(
    db: &impl ConnectionTrait,
    model: &Model,
    functions: &FunctionRegistry,
    table: &Table,
    records: &mut [JsonValue],
) -> Result<(), Error> {
    let computed: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.is_computed() && !c.is_stored())
        .map(|c| c.name.as_str())
        .collect();
    let ids: Vec<i64> = records.iter().filter_map(|r| r["id"].as_i64()).collect();
    if computed.is_empty() || ids.is_empty() {
        return Ok(());
    }
    let mut graph = Graph::new(db, model, functions).await?;
    for column in &computed {
        graph.ensure_column(&table.name, &ids, column).await?;
    }
    for failure in &graph.failures {
        tracing::warn!(colonne = %failure.column, id = failure.id, "formule : {}", failure.message);
    }
    for record in records {
        let Some(id) = record["id"].as_i64() else {
            continue;
        };
        for column in &computed {
            let ty = result_type(model, &ColumnRef::new(&table.name, *column));
            record[*column] = value_to_json(ty, graph.value(&table.name, id, column));
        }
    }
    Ok(())
}

/// Enregistrements et paramètres modifiés.
#[derive(Debug, Clone, Default)]
pub(crate) struct Changes {
    rows: BTreeMap<String, BTreeSet<i64>>,
    parameters: BTreeSet<String>,
}

impl Changes {
    pub(crate) fn parameter(name: &str) -> Self {
        Self {
            parameters: BTreeSet::from([name.to_owned()]),
            ..Self::default()
        }
    }

    fn add(&mut self, table: &str, ids: impl IntoIterator<Item = i64>) {
        self.rows.entry(table.to_owned()).or_default().extend(ids);
    }

    pub(crate) fn merge(&mut self, other: Self) {
        for (table, ids) in other.rows {
            self.add(&table, ids);
        }
        self.parameters.extend(other.parameters);
    }
}

async fn ids(
    db: &impl ConnectionTrait,
    select: &sea_orm::sea_query::SelectStatement,
    column: &str,
) -> Result<Vec<i64>, Error> {
    let mut ids = Vec::new();
    for row in db.query_all(select).await? {
        if let Some(id) = row.try_get::<Option<i64>>("", column)? {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Un enregistrement et son voisinage : les enregistrements qu'il référence ou
/// lie, et avec `referrers` ceux qui le référencent ou le lient (à capturer avant
/// une suppression, qui les modifie).
pub(crate) async fn neighborhood(
    db: &impl ConnectionTrait,
    model: &Model,
    table: &str,
    id: i64,
    referrers: bool,
) -> Result<Changes, Error> {
    let mut changes = Changes::default();
    changes.add(table, [id]);
    for relation in model.relations() {
        let column = || Alias::new(&relation.source.column);
        if relation.source.table == table {
            let select = match relation.join_table() {
                None => Query::select()
                    .column(column())
                    .from(Alias::new(table))
                    .and_where(Expr::col(Alias::new("id")).eq(id))
                    .to_owned(),
                Some(join) => Query::select()
                    .column(Alias::new(links::TARGET))
                    .from(Alias::new(join))
                    .and_where(Expr::col(Alias::new(links::SOURCE)).eq(id))
                    .to_owned(),
            };
            let output = if relation.kind == RelationKind::ManyToOne {
                &relation.source.column
            } else {
                links::TARGET
            };
            changes.add(&relation.target, ids(db, &select, output).await?);
        }
        if referrers && relation.target == table {
            let (select, output) = match relation.join_table() {
                None => (
                    Query::select()
                        .column(Alias::new("id"))
                        .from(Alias::new(&relation.source.table))
                        .and_where(Expr::col(column()).eq(id))
                        .to_owned(),
                    "id",
                ),
                Some(join) => (
                    Query::select()
                        .column(Alias::new(links::SOURCE))
                        .from(Alias::new(join))
                        .and_where(Expr::col(Alias::new(links::TARGET)).eq(id))
                        .to_owned(),
                    links::SOURCE,
                ),
            };
            changes.add(&relation.source.table, ids(db, &select, output).await?);
        }
    }
    Ok(changes)
}

/// Recalcule et stocke les formules persistées touchées par `changes`, en
/// cascade, dans l'ordre des dépendances.
///
/// Une erreur d'évaluation annule l'écriture (erreur de validation sur la colonne).
pub(crate) async fn propagate(
    db: &impl ConnectionTrait,
    model: &Model,
    functions: &FunctionRegistry,
    mut changes: Changes,
) -> Result<(), Error> {
    let persisted: Vec<&ColumnRef> = model
        .computed_order()
        .iter()
        .filter(|c| {
            model
                .column(c)
                .is_some_and(|col| col.formula.is_some() && col.persist)
        })
        .collect();
    for column in persisted {
        let affected: Vec<i64> = affected(db, model, column, &changes)
            .await?
            .into_iter()
            .collect();
        if affected.is_empty() {
            continue;
        }
        let ty = model.column(column).expect("colonne persistée").ty;
        let mut graph = Graph::new(db, model, functions).await?;
        graph.load(&column.table, &affected).await?;
        let before: BTreeMap<i64, Value> = affected
            .iter()
            .map(|id| (*id, graph.value(&column.table, *id, &column.column)))
            .collect();
        graph
            .evaluate(&column.table, &affected, &column.column)
            .await?;
        if let Some(failure) = graph.failures.first() {
            return Err(Error::Validation(FieldErrors::from([(
                failure.column.column.clone(),
                vec![failure.message.clone()],
            )])));
        }

        let mut updated = Vec::new();
        for id in affected
            .into_iter()
            .filter(|id| graph.contains(&column.table, *id))
        {
            let typed = value_to_typed(ty, graph.value(&column.table, id, &column.column));
            if typed_to_value(typed.clone()) == before[&id] {
                continue;
            }
            let update = Query::update()
                .table(Alias::new(&column.table))
                .value(Alias::new(&column.column), values::to_db(ty, typed))
                .and_where(Expr::col(Alias::new("id")).eq(id))
                .to_owned();
            db.execute(&update).await?;
            updated.push(id);
        }
        changes.add(&column.table, updated);
    }
    Ok(())
}

/// Enregistrements de `column.table` dont la valeur de `column` peut changer.
async fn affected(
    db: &impl ConnectionTrait,
    model: &Model,
    column: &ColumnRef,
    changes: &Changes,
) -> Result<BTreeSet<i64>, Error> {
    let table = &column.table;
    if paths::parameters(model, column)
        .iter()
        .any(|p| changes.parameters.contains(p))
    {
        let all = Query::select()
            .column(Alias::new("id"))
            .from(Alias::new(table))
            .to_owned();
        return Ok(ids(db, &all, "id").await?.into_iter().collect());
    }
    let expr = model.formula(column).expect("formule persistée");
    let mut affected = changes.rows.get(table).cloned().unwrap_or_default();
    for segments in paths::paths(expr) {
        let path = paths::resolve(model, table, &segments);
        // Un changement sur une table du chemin remonte, étape par étape, jusqu'ici.
        for (position, hop) in path.hops.iter().enumerate() {
            let Some(modified) = changes.rows.get(hop.to()) else {
                continue;
            };
            let mut current: Vec<i64> = modified.iter().copied().collect();
            for back in path.hops[..=position].iter().rev() {
                current = reverse(db, back, &current).await?;
                if current.is_empty() {
                    break;
                }
            }
            affected.extend(current);
        }
    }
    Ok(affected)
}

/// Identifiants de `hop.from()` liés par `hop` aux identifiants `ids` de `hop.to()`.
async fn reverse(db: &impl ConnectionTrait, hop: &Hop, targets: &[i64]) -> Result<Vec<i64>, Error> {
    let (table, filter, output) = match hop {
        Hop::Reference { from, column, .. } => (from.as_str(), column.as_str(), "id"),
        Hop::List { join, .. } => (join.as_str(), links::TARGET, links::SOURCE),
        Hop::Inverse { to, column, .. } => (to.as_str(), "id", column.as_str()),
        Hop::InverseList { join, .. } => (join.as_str(), links::SOURCE, links::TARGET),
    };
    let mut found = Vec::new();
    for chunk in targets.chunks(500) {
        let select = Query::select()
            .column(Alias::new(output))
            .from(Alias::new(table))
            .and_where(Expr::col(Alias::new(filter)).is_in(chunk.iter().copied()))
            .to_owned();
        found.extend(ids(db, &select, output).await?);
    }
    found.sort_unstable();
    found.dedup();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn conversions_round_to_storage() {
        let n = |s: &str| Value::Number(s.parse::<Decimal>().unwrap());
        assert_eq!(
            value_to_typed(ColumnType::Integer, n("2.5")),
            TypedValue::Integer(3)
        );
        assert_eq!(
            value_to_typed(ColumnType::Decimal, n("1.23456")),
            TypedValue::Decimal("1.2346".parse().unwrap())
        );
        assert_eq!(
            value_to_json(ColumnType::Decimal, n("300.0000")),
            json!("300")
        );
        assert_eq!(value_to_json(ColumnType::Integer, n("41.6")), json!(42));
        assert_eq!(
            value_to_json(ColumnType::Date, Value::Null),
            JsonValue::Null
        );
    }
}
