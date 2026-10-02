//! Graphe d'enregistrements chargé par lots, sur lequel les formules s'évaluent.
//!
//! Pour un ensemble d'identifiants, chaque étape d'un chemin coûte une requête
//! (par paquet de [`CHUNK`] identifiants), quel que soit le nombre d'enregistrements :
//! pas de requête par ligne. L'évaluation elle-même se fait ensuite en mémoire.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::Pin;

use forge_formula::{Env, FunctionRegistry, Value, VarScope};
use forge_schema::spec::ColumnType;
use forge_schema::{ColumnRef, Model};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::prelude::{Date, DateTimeUtc};
use sea_orm::sea_query::{Alias, Expr, ExprTrait, Query};
use sea_orm::{ConnectionTrait, QueryResult};

use super::paths::{self, Hop, Path};
use crate::error::Error;
use crate::{links, parameters};

/// Nombre maximal d'identifiants par clause `IN`.
const CHUNK: usize = 500;

pub(crate) type Row = BTreeMap<String, Value>;
type Boxed<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// Type de stockage d'une colonne (système comprise) ; `None` si elle n'est pas stockée.
pub(crate) fn stored_type(model: &Model, table: &str, column: &str) -> Option<ColumnType> {
    match column {
        "id" | "owner" => Some(ColumnType::Reference),
        "created_at" | "updated_at" => Some(ColumnType::Datetime),
        _ => model
            .table(table)?
            .columns
            .iter()
            .find(|c| c.name == column && c.is_stored())
            .map(|c| c.ty),
    }
}

fn stored_columns(model: &Model, table: &str) -> Vec<(String, ColumnType)> {
    let system = [
        ("id", ColumnType::Reference),
        ("owner", ColumnType::Reference),
        ("created_at", ColumnType::Datetime),
        ("updated_at", ColumnType::Datetime),
    ];
    let mut columns: Vec<_> = system.iter().map(|(n, t)| ((*n).to_owned(), *t)).collect();
    if let Some(table) = model.table(table) {
        columns.extend(
            table
                .columns
                .iter()
                .filter(|c| c.is_stored())
                .map(|c| (c.name.clone(), c.ty)),
        );
    }
    columns
}

/// Lit une valeur de ligne selon le type de la colonne.
fn read(row: &QueryResult, column: &str, ty: ColumnType) -> Result<Value, Error> {
    Ok(match ty {
        ColumnType::String | ColumnType::Text | ColumnType::Enum => row
            .try_get::<Option<String>>("", column)?
            .map_or(Value::Null, Value::Text),
        ColumnType::Integer | ColumnType::Duration | ColumnType::Reference => row
            .try_get::<Option<i64>>("", column)?
            .map_or(Value::Null, Value::from),
        ColumnType::Decimal => row
            .try_get::<Option<Decimal>>("", column)?
            .map_or(Value::Null, Value::Number),
        ColumnType::Boolean => row
            .try_get::<Option<bool>>("", column)?
            .map_or(Value::Null, Value::Boolean),
        ColumnType::Date => row
            .try_get::<Option<Date>>("", column)?
            .map_or(Value::Null, Value::Date),
        ColumnType::Datetime => row
            .try_get::<Option<DateTimeUtc>>("", column)?
            .map_or(Value::Null, Value::DateTime),
        ColumnType::ReferenceList | ColumnType::Lookup => Value::Null,
    })
}

fn as_id(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.to_i64(),
        _ => None,
    }
}

/// Exécute `select` par paquets d'identifiants (`column IN …`).
async fn select_in(
    db: &impl ConnectionTrait,
    ids: &[i64],
    build: impl Fn(&[i64]) -> sea_orm::sea_query::SelectStatement,
) -> Result<Vec<QueryResult>, Error> {
    let mut rows = Vec::new();
    for chunk in ids.chunks(CHUNK) {
        rows.extend(db.query_all(&build(chunk)).await?);
    }
    Ok(rows)
}

/// Erreur d'évaluation d'une formule, pour un enregistrement.
#[derive(Debug, Clone)]
pub(crate) struct Failure {
    pub column: ColumnRef,
    pub id: i64,
    pub message: String,
}

pub(crate) struct Graph<'a, C> {
    db: &'a C,
    model: &'a Model,
    functions: &'a FunctionRegistry,
    parameters: BTreeMap<String, Value>,
    rows: HashMap<String, HashMap<i64, Row>>,
    /// Liens « plusieurs » chargés : clé de l'étape → source → cibles.
    links: HashMap<(String, String), HashMap<i64, Vec<i64>>>,
    pub failures: Vec<Failure>,
}

impl<'a, C: ConnectionTrait> Graph<'a, C> {
    pub(crate) async fn new(
        db: &'a C,
        model: &'a Model,
        functions: &'a FunctionRegistry,
    ) -> Result<Self, Error> {
        let parameters = parameters::values(db, &model.spec().parameters)
            .await?
            .into_iter()
            .map(|(name, (ty, json))| {
                let value = forge_schema::value::from_json(ty, None, &json)
                    .map_or(Value::Null, super::typed_to_value);
                (name, value)
            })
            .collect();
        Ok(Self {
            db,
            model,
            functions,
            parameters,
            rows: HashMap::new(),
            links: HashMap::new(),
            failures: Vec::new(),
        })
    }

    pub(crate) fn value(&self, table: &str, id: i64, column: &str) -> Value {
        self.rows
            .get(table)
            .and_then(|rows| rows.get(&id))
            .and_then(|row| row.get(column))
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub(crate) fn contains(&self, table: &str, id: i64) -> bool {
        self.rows
            .get(table)
            .is_some_and(|rows| rows.contains_key(&id))
    }

    /// Charge les colonnes stockées des enregistrements absents du graphe.
    pub(crate) async fn load(&mut self, table: &str, ids: &[i64]) -> Result<(), Error> {
        let known = self.rows.entry(table.to_owned()).or_default();
        let mut missing: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|id| !known.contains_key(id))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(());
        }
        let columns = stored_columns(self.model, table);
        let rows = select_in(self.db, &missing, |chunk| {
            Query::select()
                .columns(columns.iter().map(|(name, _)| Alias::new(name)))
                .from(Alias::new(table))
                .and_where(Expr::col(Alias::new("id")).is_in(chunk.iter().copied()))
                .to_owned()
        })
        .await?;
        let known = self.rows.entry(table.to_owned()).or_default();
        for row in rows {
            let mut values = Row::new();
            for (name, ty) in &columns {
                values.insert(name.clone(), read(&row, name, *ty)?);
            }
            if let Some(id) = values.get("id").and_then(as_id) {
                known.insert(id, values);
            }
        }
        Ok(())
    }

    /// Suit une étape depuis `ids` ; retourne les identifiants atteints (chargés).
    async fn follow(&mut self, hop: &Hop, ids: &[i64]) -> Result<Vec<i64>, Error> {
        let mut targets = Vec::new();
        if let Hop::Reference { from, column, .. } = hop {
            self.load(from, ids).await?;
            targets.extend(
                ids.iter()
                    .filter_map(|id| as_id(&self.value(from, *id, column))),
            );
        } else {
            let key = hop.key();
            let known = self.links.entry(key.clone()).or_default();
            let missing: Vec<i64> = ids
                .iter()
                .copied()
                .filter(|id| !known.contains_key(id))
                .collect();
            if !missing.is_empty() {
                let (table, source, target) = match hop {
                    Hop::List { join, .. } => (join.as_str(), links::SOURCE, links::TARGET),
                    Hop::InverseList { join, .. } => (join.as_str(), links::TARGET, links::SOURCE),
                    Hop::Inverse { to, column, .. } => (to.as_str(), column.as_str(), "id"),
                    Hop::Reference { .. } => unreachable!("traité ci-dessus"),
                };
                let rows = select_in(self.db, &missing, |chunk| {
                    Query::select()
                        .columns([Alias::new(source), Alias::new(target)])
                        .from(Alias::new(table))
                        .and_where(Expr::col(Alias::new(source)).is_in(chunk.iter().copied()))
                        .to_owned()
                })
                .await?;
                let known = self.links.entry(key.clone()).or_default();
                for id in &missing {
                    known.entry(*id).or_default();
                }
                for row in rows {
                    let from: i64 = row.try_get("", source)?;
                    let to: i64 = row.try_get("", target)?;
                    known.entry(from).or_default().push(to);
                }
            }
            let known = &self.links[&key];
            targets.extend(
                ids.iter()
                    .flat_map(|id| known.get(id).into_iter().flatten().copied()),
            );
        }
        targets.sort_unstable();
        targets.dedup();
        self.load(hop.to(), &targets).await?;
        Ok(targets)
    }

    /// Charge tout ce qu'il faut pour lire `path` depuis `ids`.
    fn ensure_path<'b>(
        &'b mut self,
        table: &'b str,
        ids: &'b [i64],
        path: &'b Path,
    ) -> Boxed<'b, ()> {
        Box::pin(async move {
            let mut current = ids.to_vec();
            self.load(table, &current).await?;
            for hop in &path.hops {
                current = self.follow(hop, &current).await?;
            }
            if let Some(column) = &path.column {
                self.ensure_column(path.end_table(table), &current, column)
                    .await?;
            }
            Ok(())
        })
    }

    /// Garantit la valeur de `column` (stockée, lookup ou formule) pour `ids`.
    pub(crate) fn ensure_column<'b>(
        &'b mut self,
        table: &'b str,
        ids: &'b [i64],
        column: &'b str,
    ) -> Boxed<'b, ()> {
        Box::pin(async move {
            self.load(table, ids).await?;
            let pending: Vec<i64> = ids
                .iter()
                .copied()
                .filter(|id| {
                    self.rows[table]
                        .get(id)
                        .is_some_and(|row| !row.contains_key(column))
                })
                .collect();
            if pending.is_empty() {
                return Ok(());
            }
            let reference = ColumnRef::new(table, column);
            if let Some(segments) = self.model.lookup_path(&reference) {
                let path = paths::resolve(self.model, table, segments);
                self.ensure_path(table, &pending, &path).await?;
                for id in pending {
                    let value = self.walk(table, id, &path);
                    self.set(table, id, column, value);
                }
                Ok(())
            } else {
                self.evaluate(table, &pending, column).await
            }
        })
    }

    /// Évalue la formule de `column` pour `ids`, en remplaçant toute valeur présente.
    pub(crate) async fn evaluate(
        &mut self,
        table: &str,
        ids: &[i64],
        column: &str,
    ) -> Result<(), Error> {
        let reference = ColumnRef::new(table, column);
        let Some(expr) = self.model.formula(&reference) else {
            return Ok(());
        };
        for segments in paths::paths(expr) {
            let path = paths::resolve(self.model, table, &segments);
            self.ensure_path(table, ids, &path).await?;
        }
        let present: Vec<i64> = ids
            .iter()
            .copied()
            .filter(|id| self.contains(table, *id))
            .collect();
        for id in present {
            let env = RowEnv {
                graph: self,
                table,
                id,
            };
            let value = match forge_formula::evaluate(expr, &env, self.functions) {
                Ok(value) => value,
                Err(err) => {
                    self.failures.push(Failure {
                        column: reference.clone(),
                        id,
                        message: err.to_string(),
                    });
                    Value::Null
                }
            };
            self.set(table, id, column, value);
        }
        Ok(())
    }

    fn set(&mut self, table: &str, id: i64, column: &str, value: Value) {
        if let Some(row) = self.rows.get_mut(table).and_then(|rows| rows.get_mut(&id)) {
            row.insert(column.to_owned(), value);
        }
    }

    /// Valeur d'un chemin à valeur unique.
    fn walk(&self, table: &str, id: i64, path: &Path) -> Value {
        let mut current = (table, id);
        for hop in &path.hops {
            let Hop::Reference { column, to, .. } = hop else {
                return Value::Null;
            };
            match as_id(&self.value(current.0, current.1, column)) {
                Some(target) => current = (to, target),
                None => return Value::Null,
            }
        }
        path.column.as_ref().map_or(Value::Null, |column| {
            self.value(current.0, current.1, column)
        })
    }

    /// Valeurs d'un chemin traversant une relation « plusieurs ».
    fn collect(&self, table: &str, id: i64, path: &Path) -> Vec<Value> {
        let mut current = vec![id];
        for hop in &path.hops {
            current = if let Hop::Reference { from, column, .. } = hop {
                current
                    .iter()
                    .filter_map(|id| as_id(&self.value(from, *id, column)))
                    .collect()
            } else {
                let known = self.links.get(&hop.key());
                current
                    .iter()
                    .flat_map(|id| known.and_then(|k| k.get(id)).into_iter().flatten().copied())
                    .collect()
            };
        }
        let end = path.end_table(table);
        current
            .into_iter()
            .map(|id| match &path.column {
                Some(column) => self.value(end, id, column),
                None => Value::Boolean(true),
            })
            .collect()
    }
}

/// Environnement d'évaluation d'une formule pour un enregistrement.
struct RowEnv<'g, 'a, C> {
    graph: &'g Graph<'a, C>,
    table: &'g str,
    id: i64,
}

impl<C: ConnectionTrait> Env for RowEnv<'_, '_, C> {
    fn path(&self, segments: &[String]) -> Value {
        let path = paths::resolve(self.graph.model, self.table, segments);
        self.graph.walk(self.table, self.id, &path)
    }

    fn variable(&self, scope: VarScope, path: &[String]) -> Value {
        match scope {
            VarScope::Param => self
                .graph
                .parameters
                .get(&path[0])
                .cloned()
                .unwrap_or(Value::Null),
            // Les formules n'ont pas accès à l'utilisateur (refusé par le schéma).
            VarScope::User => Value::Null,
        }
    }

    fn collection(&self, segments: &[String]) -> Vec<Value> {
        let path = paths::resolve(self.graph.model, self.table, segments);
        self.graph.collect(self.table, self.id, &path)
    }
}
