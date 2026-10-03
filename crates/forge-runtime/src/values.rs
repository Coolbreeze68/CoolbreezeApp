//! Conversion des valeurs typées du schéma en valeurs sea-orm.

use forge_schema::spec::ColumnType;
use forge_schema::value::TypedValue;
use sea_orm::Value;
use sea_orm::prelude::{Date, DateTimeUtc, Decimal};

/// Convertit une valeur typée en [`Value`] du type exact de la colonne générée,
/// y compris pour `NULL` (sea-orm exige la bonne variante).
pub(crate) fn to_db(ty: ColumnType, value: TypedValue) -> Value {
    match (ty, value) {
        (_, TypedValue::String(s)) => s.into(),
        (_, TypedValue::Integer(i)) => i.into(),
        (_, TypedValue::Decimal(d)) => d.into(),
        (_, TypedValue::Boolean(b)) => b.into(),
        (_, TypedValue::Date(d)) => d.into(),
        (_, TypedValue::Datetime(d)) => d.into(),
        (ty, TypedValue::Null) => null(ty),
    }
}

fn null(ty: ColumnType) -> Value {
    match ty {
        ColumnType::String | ColumnType::Text | ColumnType::Enum => Option::<String>::None.into(),
        ColumnType::Integer
        | ColumnType::Duration
        | ColumnType::Reference
        | ColumnType::ReferenceList
        | ColumnType::Lookup => Option::<i64>::None.into(),
        ColumnType::Decimal => Option::<Decimal>::None.into(),
        ColumnType::Boolean => Option::<bool>::None.into(),
        ColumnType::Date => Option::<Date>::None.into(),
        ColumnType::Datetime => Option::<DateTimeUtc>::None.into(),
        model => null(model.base()),
    }
}
