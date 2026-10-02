//! Paramètres de liste : pagination, tri, recherche et filtres.
//!
//! ```text
//! GET /api/opportunite?page=2&per_page=50&sort=-montant,titre&q=dupont
//!                     &etape=gagne&montant[gte]=1000&date_cloture[null]=false
//! ```
//!
//! Opérateurs : `eq` (défaut), `ne`, `lt`, `lte`, `gt`, `gte`,
//! `like` (contient, sans tenir compte de la casse), `in` (liste séparée par des virgules),
//! `null` (`true` ou `false`).

use forge_schema::spec::{ColumnType, Table};
use forge_schema::value::{self, TypedValue};
use sea_orm::sea_query::{Expr, ExprTrait, Func, LikeExpr, SimpleExpr};
use sea_orm::{ColumnTrait, Condition, EntityTrait, Order, QueryFilter, QueryOrder, Select};

use crate::columns::{self, Queryable};
use crate::error::{Error, FieldErrors};
use crate::resource::column_of;
use crate::values;

pub const DEFAULT_PER_PAGE: u64 = 25;
pub const MAX_PER_PAGE: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Like,
    In,
    Null,
}

impl Op {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "eq" => Self::Eq,
            "ne" => Self::Ne,
            "lt" => Self::Lt,
            "lte" => Self::Lte,
            "gt" => Self::Gt,
            "gte" => Self::Gte,
            "like" => Self::Like,
            "in" => Self::In,
            "null" => Self::Null,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Filter {
    column: String,
    ty: ColumnType,
    op: Op,
    values: Vec<TypedValue>,
}

/// Paramètres de liste validés pour une table.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListQuery {
    pub page: u64,
    pub per_page: u64,
    sort: Vec<(String, Order)>,
    search: Option<String>,
    filters: Vec<Filter>,
}

impl ListQuery {
    /// Analyse une chaîne de requête (`a=1&b[gte]=2`). Toutes les erreurs sont rapportées.
    pub(crate) fn parse(table: &Table, raw: Option<&str>) -> Result<Self, Error> {
        Self::from_pairs(table, pairs(raw)?)
    }

    /// Comme [`Self::parse`], sur des paires déjà décodées.
    pub(crate) fn from_pairs(table: &Table, pairs: Vec<(String, String)>) -> Result<Self, Error> {
        let mut query = Self {
            page: 1,
            per_page: DEFAULT_PER_PAGE,
            sort: Vec::new(),
            search: None,
            filters: Vec::new(),
        };
        let mut errors = FieldErrors::new();
        let mut error = |field: &str, message: String| {
            errors.entry(field.to_owned()).or_default().push(message);
        };

        for (key, raw_value) in pairs {
            match key.as_str() {
                "page" => match raw_value.parse::<u64>() {
                    Ok(page) if page >= 1 => query.page = page,
                    _ => error("page", "entier supérieur ou égal à 1 attendu".into()),
                },
                "per_page" => match raw_value.parse::<u64>() {
                    Ok(n) if (1..=MAX_PER_PAGE).contains(&n) => query.per_page = n,
                    _ => error(
                        "per_page",
                        format!("entier entre 1 et {MAX_PER_PAGE} attendu"),
                    ),
                },
                "sort" => {
                    for item in raw_value.split(',').filter(|s| !s.is_empty()) {
                        let (name, order) = match item.strip_prefix('-') {
                            Some(name) => (name, Order::Desc),
                            None => (item, Order::Asc),
                        };
                        if columns::queryable(table, name).is_some() {
                            query.sort.push((name.to_owned(), order));
                        } else {
                            error("sort", format!("tri impossible sur `{name}`"));
                        }
                    }
                }
                "q" => query.search = Some(raw_value).filter(|s| !s.trim().is_empty()),
                _ => match parse_filter(table, &key, &raw_value) {
                    Ok(filter) => query.filters.push(filter),
                    Err(message) => error(&key, message),
                },
            }
        }

        if errors.is_empty() {
            Ok(query)
        } else {
            Err(Error::Validation(errors))
        }
    }

    /// Applique filtres, recherche et tri à une requête sea-orm.
    pub(crate) fn apply<E: EntityTrait>(&self, table: &Table, select: Select<E>) -> Select<E> {
        let mut select = self.filter(table, select);
        for (name, order) in &self.sort {
            select = select.order_by(column_of::<E>(name), order.clone());
        }
        // L'identifiant en dernier critère garantit une pagination stable.
        select.order_by(column_of::<E>("id"), Order::Asc)
    }

    /// Filtres et recherche, sans tri.
    pub(crate) fn filter<E: EntityTrait>(&self, table: &Table, mut select: Select<E>) -> Select<E> {
        for filter in &self.filters {
            select = select.filter(filter.condition::<E>());
        }
        if let Some(search) = &self.search {
            let pattern = like_pattern(search);
            let mut any = Condition::any();
            for column in table
                .columns
                .iter()
                .filter(|c| c.is_stored() && matches!(c.ty, ColumnType::String | ColumnType::Text))
            {
                any = any.add(lower(column_of::<E>(&column.name)).like(pattern.clone()));
            }
            select = select.filter(any);
        }
        select
    }
}

/// Décode une chaîne de requête en paires clé-valeur.
pub(crate) fn pairs(raw: Option<&str>) -> Result<Vec<(String, String)>, Error> {
    serde_urlencoded::from_str(raw.unwrap_or_default())
        .map_err(|err| Error::BadRequest(err.to_string()))
}

fn parse_filter(table: &Table, key: &str, raw: &str) -> Result<Filter, String> {
    let (name, op) = match key.split_once('[') {
        Some((name, rest)) => {
            let op = rest
                .strip_suffix(']')
                .and_then(Op::parse)
                .ok_or_else(|| format!("opérateur inconnu dans `{key}`"))?;
            (name, op)
        }
        None => (key, Op::Eq),
    };
    let Queryable { ty, values } =
        columns::queryable(table, name).ok_or_else(|| format!("filtre impossible sur `{name}`"))?;

    let typed = match op {
        Op::Null => match raw {
            "true" => vec![TypedValue::Boolean(true)],
            "false" => vec![TypedValue::Boolean(false)],
            _ => return Err("`true` ou `false` attendu".into()),
        },
        Op::Like if !matches!(ty, ColumnType::String | ColumnType::Text | ColumnType::Enum) => {
            return Err("`like` est réservé aux colonnes texte".into());
        }
        Op::Like => vec![TypedValue::String(raw.to_owned())],
        Op::In => raw
            .split(',')
            .map(|item| value::from_text(ty, values, item))
            .collect::<Result<_, _>>()?,
        _ => vec![value::from_text(ty, values, raw)?],
    };
    Ok(Filter {
        column: name.to_owned(),
        ty,
        op,
        values: typed,
    })
}

impl Filter {
    fn condition<E: EntityTrait>(&self) -> SimpleExpr {
        let column = column_of::<E>(&self.column);
        let mut values = self
            .values
            .iter()
            .map(|v| values::to_db(self.ty, v.clone()));
        match self.op {
            Op::Null if self.values[0] == TypedValue::Boolean(true) => column.is_null(),
            Op::Null => column.is_not_null(),
            Op::Like => {
                let TypedValue::String(text) = &self.values[0] else {
                    unreachable!("validé par parse_filter");
                };
                lower(column).like(like_pattern(text))
            }
            Op::In => column.is_in(values),
            op => {
                let value = values.next().expect("une valeur par filtre");
                match op {
                    Op::Ne => column.ne(value),
                    Op::Lt => column.lt(value),
                    Op::Lte => column.lte(value),
                    Op::Gt => column.gt(value),
                    Op::Gte => column.gte(value),
                    _ => column.eq(value),
                }
            }
        }
    }
}

fn lower(column: impl ColumnTrait) -> SimpleExpr {
    Func::lower(Expr::col(column.as_column_ref())).into()
}

/// Motif `LIKE` « contient », insensible à la casse, avec échappement de `%` et `_`.
fn like_pattern(text: &str) -> LikeExpr {
    let escaped = text
        .to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    LikeExpr::new(format!("%{escaped}%")).escape('\\')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn table() -> Table {
        serde_json::from_value(json!({
            "name": "opportunite",
            "columns": [
                { "name": "titre", "type": "string" },
                { "name": "montant", "type": "decimal" },
                { "name": "etape", "type": "enum", "values": ["a", "b"] },
                { "name": "ttc", "type": "decimal", "formula": "montant" },
                { "name": "tags", "type": "reference_list", "target": "tag" }
            ]
        }))
        .unwrap()
    }

    fn fields(raw: &str) -> Vec<String> {
        match ListQuery::parse(&table(), Some(raw)) {
            Err(Error::Validation(fields)) => fields.into_keys().collect(),
            other => panic!("erreur de validation attendue : {other:?}"),
        }
    }

    #[test]
    fn defaults() {
        let query = ListQuery::parse(&table(), None).unwrap();
        assert_eq!((query.page, query.per_page), (1, DEFAULT_PER_PAGE));
    }

    #[test]
    fn parses_everything() {
        let query = ListQuery::parse(
            &table(),
            Some("page=2&per_page=10&sort=-montant,created_at&q=x&etape[in]=a,b&montant[gte]=10.5&owner[null]=true"),
        )
        .unwrap();
        assert_eq!((query.page, query.per_page), (2, 10));
        assert_eq!(
            query.sort,
            vec![
                ("montant".into(), Order::Desc),
                ("created_at".into(), Order::Asc)
            ]
        );
        assert_eq!(query.search.as_deref(), Some("x"));
        assert_eq!(query.filters.len(), 3);
        assert_eq!(query.filters[0].values.len(), 2);
    }

    #[test]
    fn rejects_invalid_parameters() {
        assert_eq!(fields("page=0&per_page=500"), ["page", "per_page"]);
        assert_eq!(fields("sort=ttc"), ["sort"]);
        assert_eq!(fields("tags=1&inconnu=2"), ["inconnu", "tags"]);
        assert_eq!(fields("montant[gt]=abc&etape=z"), ["etape", "montant[gt]"]);
        assert_eq!(
            fields("montant[like]=1&titre[foo]=1"),
            ["montant[like]", "titre[foo]"]
        );
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        let condition = Expr::col("titre").like(like_pattern("50%_A"));
        let sql = sea_orm::sea_query::Query::select()
            .expr(condition)
            .to_string(sea_orm::sea_query::SqliteQueryBuilder);
        assert_eq!(sql, r#"SELECT "titre" LIKE '%50\%\_a%' ESCAPE '\'"#);
    }
}
