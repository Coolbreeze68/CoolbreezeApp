//! Agrégats d'une table, calculés en SQL.
//!
//! ```text
//! GET /api/opportunite/aggregate?fields=montant,montant_pondere&group_by=etape&date_cloture[gte]=2026-01-01
//! ```
//!
//! ```json
//! { "fields": ["montant"], "group_by": "etape",
//!   "groups": [ { "key": "gagne", "count": 2, "montant": { "sum": "3000", "avg": "1500", "min": "1000", "max": "2000" } } ],
//!   "total": { "count": 5, "montant": { … } } }
//! ```
//!
//! `fields` : colonnes numériques stockées (formules persistées comprises) ;
//! `group_by` (facultatif) : colonne stockée. Les filtres et la recherche de
//! liste s'appliquent, ainsi que le périmètre de lecture de l'utilisateur.

use forge_schema::spec::{ColumnType, Table};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use sea_orm::sea_query::{Expr, Func, SimpleExpr};
use sea_orm::{
    EntityTrait, FromQueryResult, QueryFilter, QueryOrder, QueryResult, QuerySelect, QueryTrait,
    Select,
};
use serde_json::{Map, Value as JsonValue, json};

use crate::columns;
use crate::error::{Error, FieldErrors};
use crate::query::{ListQuery, pairs};
use crate::resource::column_of;
use crate::rules::Scope;

const FUNCTIONS: [&str; 4] = ["sum", "avg", "min", "max"];

/// Paramètres d'agrégation validés.
#[derive(Debug)]
pub(crate) struct AggregateQuery {
    fields: Vec<String>,
    group_by: Option<(String, ColumnType)>,
    list: ListQuery,
}

impl AggregateQuery {
    pub(crate) fn parse(table: &Table, raw: Option<&str>) -> Result<Self, Error> {
        Self::from_pairs(table, pairs(raw)?)
    }

    /// Comme [`Self::parse`], sur des paires déjà décodées.
    pub(crate) fn from_pairs(table: &Table, pairs: Vec<(String, String)>) -> Result<Self, Error> {
        let (mut fields, mut group_by, mut rest) = (None, None, Vec::new());
        for (key, value) in pairs {
            match key.as_str() {
                "fields" => fields = Some(value),
                "group_by" => group_by = Some(value),
                _ => rest.push((key, value)),
            }
        }
        let mut errors = FieldErrors::new();
        let mut error = |field: &str, message: String| {
            errors.entry(field.to_owned()).or_default().push(message);
        };

        let fields: Vec<String> = fields
            .unwrap_or_default()
            .split(',')
            .filter(|f| !f.is_empty())
            .map(str::to_owned)
            .collect();
        if fields.is_empty() {
            error(
                "fields",
                "au moins une colonne numérique attendue (`fields=montant`)".into(),
            );
        }
        for field in &fields {
            let numeric = columns::queryable(table, field).is_some_and(|q| {
                matches!(
                    q.ty,
                    ColumnType::Integer | ColumnType::Decimal | ColumnType::Duration
                )
            });
            if !numeric {
                error(
                    "fields",
                    format!("`{field}` n'est pas une colonne numérique stockée"),
                );
            }
        }
        let group_by = group_by.and_then(|name| match columns::queryable(table, &name) {
            Some(q) if q.ty != ColumnType::Text => Some((name, q.ty)),
            _ => {
                error("group_by", format!("regroupement impossible sur `{name}`"));
                None
            }
        });
        let list = ListQuery::from_pairs(table, rest);
        if let Err(Error::Validation(list_errors)) = &list {
            for (field, messages) in list_errors {
                errors
                    .entry(field.clone())
                    .or_default()
                    .extend(messages.iter().cloned());
            }
        }
        if !errors.is_empty() {
            return Err(Error::Validation(errors));
        }
        Ok(Self {
            fields,
            group_by,
            list: list?,
        })
    }

    /// Requêtes de regroupement et de total, filtres et périmètre appliqués.
    pub(crate) async fn run<E: EntityTrait>(
        &self,
        db: &impl sea_orm::ConnectionTrait,
        table: &Table,
        scope: &Scope,
    ) -> Result<JsonValue, Error> {
        let base = || {
            let mut select = self.list.filter(table, E::find());
            if let Some(condition) = scope.condition() {
                select = select.filter(condition);
            }
            self.measures(select.select_only())
        };
        let backend = db.get_database_backend();
        let total = match db.query_one_raw(base().build(backend)).await? {
            Some(row) => self.format(&row),
            None => Map::new(),
        };
        let mut groups = Vec::new();
        if let Some((name, ty)) = &self.group_by {
            let column = column_of::<E>(name);
            let select = base()
                .column_as(column, "key")
                .group_by(column)
                .order_by_asc(column);
            for row in db.query_all_raw(select.build(backend)).await? {
                // La clé est une vraie colonne : son type est connu de la base.
                let key_value = JsonValue::from_query_result(&row, "")?;
                let mut group = self.format(&row);
                group.insert("key".into(), key(&key_value["key"], *ty));
                groups.push(JsonValue::Object(group));
            }
        }
        Ok(json!({
            "fields": self.fields,
            "group_by": self.group_by.as_ref().map(|(name, _)| name),
            "groups": groups,
            "total": JsonValue::Object(total),
        }))
    }

    fn measures<E: EntityTrait>(&self, mut select: Select<E>) -> Select<E> {
        let count: SimpleExpr = Func::count(Expr::col(column_of::<E>("id"))).into();
        select = select.column_as(count, "count");
        for field in &self.fields {
            let column = || Expr::col(column_of::<E>(field));
            let measures: [(&str, SimpleExpr); 4] = [
                ("sum", Func::sum(column()).into()),
                ("avg", Func::avg(column()).into()),
                ("min", Func::min(column()).into()),
                ("max", Func::max(column()).into()),
            ];
            for (function, expr) in measures {
                select = select.column_as(expr, format!("{field}__{function}"));
            }
        }
        select
    }

    /// Met en forme une ligne : nombre d'enregistrements et mesures par colonne.
    fn format(&self, row: &QueryResult) -> Map<String, JsonValue> {
        let mut result = Map::new();
        let count = decimal(row, "count").and_then(|d| d.to_i64()).unwrap_or(0);
        result.insert("count".into(), json!(count));
        for field in &self.fields {
            let measures: Map<String, JsonValue> = FUNCTIONS
                .iter()
                .map(|f| {
                    let value =
                        decimal(row, &format!("{field}__{f}")).map_or(JsonValue::Null, |d| {
                            json!(
                                d.round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero)
                                    .normalize()
                                    .to_string()
                            )
                        });
                    ((*f).to_owned(), value)
                })
                .collect();
            result.insert(field.clone(), JsonValue::Object(measures));
        }
        result
    }
}

/// Mesure renvoyée par la base : son type dépend du moteur et de la fonction
/// (`SUM` d'entiers en décimal sous PostgreSQL, `MIN` en entier, `AVG` en
/// flottant sous SQLite…), d'où les lectures successives. `None` si NULL.
fn decimal(row: &QueryResult, column: &str) -> Option<Decimal> {
    if let Ok(value) = row.try_get::<Option<Decimal>>("", column) {
        return value;
    }
    if let Ok(value) = row.try_get::<Option<i64>>("", column) {
        return value.map(Decimal::from);
    }
    let value = row.try_get::<Option<f64>>("", column).ok()??;
    Decimal::from_f64_retain(value)
}

/// Clé de regroupement ; MySQL renvoie les booléens sous forme d'entiers.
fn key(value: &JsonValue, ty: ColumnType) -> JsonValue {
    match (ty, value) {
        (ColumnType::Boolean, JsonValue::Number(n)) => json!(n.as_i64() != Some(0)),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        serde_json::from_value(json!({
            "name": "opportunite",
            "columns": [
                { "name": "titre", "type": "string" },
                { "name": "notes", "type": "text" },
                { "name": "montant", "type": "decimal" },
                { "name": "ttc", "type": "decimal", "formula": "montant" },
                { "name": "etape", "type": "enum", "values": ["a", "b"] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn parameters_are_validated() {
        let query =
            AggregateQuery::parse(&table(), Some("fields=montant&group_by=etape&etape=a")).unwrap();
        assert_eq!(query.fields, ["montant"]);
        let Err(Error::Validation(errors)) =
            AggregateQuery::parse(&table(), Some("fields=titre,ttc&group_by=notes&inconnu=1"))
        else {
            panic!("validation attendue");
        };
        assert_eq!(
            errors.keys().collect::<Vec<_>>(),
            ["fields", "group_by", "inconnu"]
        );
        assert_eq!(errors["fields"].len(), 2);
        assert!(
            AggregateQuery::parse(&table(), None).is_err(),
            "fields obligatoire"
        );
    }

    #[test]
    fn boolean_keys_from_mysql() {
        assert_eq!(key(&json!(1), ColumnType::Boolean), json!(true));
        assert_eq!(key(&json!(0), ColumnType::Boolean), json!(false));
        assert_eq!(key(&json!("a"), ColumnType::Enum), json!("a"));
    }
}
