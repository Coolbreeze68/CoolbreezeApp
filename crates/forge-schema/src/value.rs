//! Valeurs typées : conversion depuis JSON ou depuis du texte (paramètres d'URL).
//!
//! Source unique des règles de typage, partagée par la validation du schéma
//! (valeurs par défaut) et par le runtime (corps de requêtes, filtres, paramètres).

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value as Json;

use crate::spec::ColumnType;

/// Valeur conforme à un [`ColumnType`] de colonne stockée.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    Null,
    /// `string`, `text` et `enum`.
    String(String),
    /// `integer`, `duration` (secondes) et `reference` (identifiant).
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
}

/// Convertit une valeur JSON ; `values` liste les valeurs d'un `enum`.
pub fn from_json(
    ty: ColumnType,
    values: Option<&[String]>,
    json: &Json,
) -> Result<TypedValue, String> {
    let typed = match (ty, json) {
        (_, Json::Null) => Some(TypedValue::Null),
        (ColumnType::String | ColumnType::Text | ColumnType::Enum, Json::String(s)) => {
            Some(TypedValue::String(s.clone()))
        }
        (ColumnType::Integer | ColumnType::Reference, Json::Number(n)) => {
            n.as_i64().map(TypedValue::Integer)
        }
        (ColumnType::Duration, Json::Number(n)) => n
            .as_u64()
            .and_then(|n| i64::try_from(n).ok())
            .map(TypedValue::Integer),
        (ColumnType::Decimal, Json::Number(n)) => Decimal::from_str(&n.to_string())
            .or_else(|_| Decimal::from_scientific(&n.to_string()))
            .ok()
            .map(TypedValue::Decimal),
        (ColumnType::Boolean, Json::Bool(b)) => Some(TypedValue::Boolean(*b)),
        // Les décimaux, dates et dates-heures s'écrivent aussi sous forme de texte.
        (ColumnType::Decimal | ColumnType::Date | ColumnType::Datetime, Json::String(s)) => {
            return from_text(ty, values, s);
        }
        _ => None,
    };
    let typed = typed.ok_or_else(|| invalid(ty, &json.to_string()))?;
    check_enum(values, &typed)?;
    Ok(typed)
}

/// Convertit un texte, par exemple un paramètre d'URL (`?montant[gte]=1000`).
pub fn from_text(
    ty: ColumnType,
    values: Option<&[String]>,
    text: &str,
) -> Result<TypedValue, String> {
    let typed = match ty {
        ColumnType::String | ColumnType::Text | ColumnType::Enum => {
            Some(TypedValue::String(text.to_owned()))
        }
        ColumnType::Integer | ColumnType::Reference => text.parse().ok().map(TypedValue::Integer),
        ColumnType::Duration => text
            .parse::<u32>()
            .ok()
            .map(|n| TypedValue::Integer(i64::from(n))),
        ColumnType::Decimal => Decimal::from_str(text).ok().map(TypedValue::Decimal),
        ColumnType::Boolean => match text {
            "true" => Some(TypedValue::Boolean(true)),
            "false" => Some(TypedValue::Boolean(false)),
            _ => None,
        },
        ColumnType::Date => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .map(TypedValue::Date),
        ColumnType::Datetime => DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|d| TypedValue::Datetime(d.with_timezone(&Utc))),
        ColumnType::ReferenceList | ColumnType::Lookup => None,
    };
    let typed = typed.ok_or_else(|| invalid(ty, &format!("\"{text}\"")))?;
    check_enum(values, &typed)?;
    Ok(typed)
}

fn check_enum(values: Option<&[String]>, typed: &TypedValue) -> Result<(), String> {
    match (values, typed) {
        (Some(values), TypedValue::String(s)) if !values.contains(s) => Err(format!(
            "`{s}` ne fait pas partie des valeurs ({})",
            values.join(", ")
        )),
        _ => Ok(()),
    }
}

fn invalid(ty: ColumnType, shown: &str) -> String {
    let expected = match ty {
        ColumnType::Integer => "un entier",
        ColumnType::Reference => "un identifiant entier",
        ColumnType::Decimal => "un nombre",
        ColumnType::Boolean => "true ou false",
        ColumnType::Duration => "un nombre entier positif de secondes",
        ColumnType::Date => "une date `AAAA-MM-JJ`",
        ColumnType::Datetime => "une date-heure RFC 3339 (`2026-01-31T09:00:00Z`)",
        ColumnType::ReferenceList | ColumnType::Lookup => "aucune valeur directe",
        ColumnType::String | ColumnType::Text | ColumnType::Enum => "un texte",
    };
    format!("valeur {shown} invalide : attendu {expected}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_values() {
        assert_eq!(
            from_json(ColumnType::Integer, None, &json!(3)),
            Ok(TypedValue::Integer(3))
        );
        assert_eq!(
            from_json(ColumnType::Decimal, None, &json!(12.5)),
            Ok(TypedValue::Decimal(Decimal::new(125, 1)))
        );
        assert_eq!(
            from_json(ColumnType::Decimal, None, &json!("12.50")),
            Ok(TypedValue::Decimal(Decimal::new(1250, 2)))
        );
        assert_eq!(
            from_json(ColumnType::Date, None, &json!(null)),
            Ok(TypedValue::Null)
        );
        assert!(from_json(ColumnType::Integer, None, &json!(1.5)).is_err());
        assert!(from_json(ColumnType::Duration, None, &json!(-1)).is_err());
        assert!(from_json(ColumnType::Boolean, None, &json!("true")).is_err());
        assert!(from_json(ColumnType::Date, None, &json!("31/01/2026")).is_err());
    }

    #[test]
    fn enums() {
        let values = ["a".to_owned(), "b".to_owned()];
        assert!(from_json(ColumnType::Enum, Some(&values), &json!("a")).is_ok());
        let err = from_json(ColumnType::Enum, Some(&values), &json!("c")).unwrap_err();
        assert!(err.contains("ne fait pas partie"), "{err}");
    }

    #[test]
    fn text_values() {
        assert_eq!(
            from_text(ColumnType::Boolean, None, "true"),
            Ok(TypedValue::Boolean(true))
        );
        assert_eq!(
            from_text(ColumnType::Datetime, None, "2026-01-31T10:00:00+01:00"),
            Ok(TypedValue::Datetime(
                "2026-01-31T09:00:00Z".parse().unwrap()
            ))
        );
        assert!(from_text(ColumnType::Integer, None, "x").is_err());
    }
}
