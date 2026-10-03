//! Valeurs typées : conversion depuis JSON ou depuis du texte (paramètres d'URL).
//!
//! Source unique des règles de typage, partagée par la validation du schéma
//! (valeurs par défaut) et par le runtime (corps de requêtes, filtres, paramètres).

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value as Json;

use crate::spec::{Column, ColumnType};

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

/// Contraintes d'une colonne sur ses valeurs, au-delà de son type.
#[derive(Debug, Clone, Copy, Default)]
pub struct Domain<'a> {
    /// `enum` : valeurs autorisées.
    pub values: Option<&'a [String]>,
    /// `rating` : note maximale (5 si absente).
    pub max: Option<u32>,
}

impl<'a> Domain<'a> {
    pub fn of(column: &'a Column) -> Self {
        Self {
            values: column.values.as_deref(),
            max: column.max,
        }
    }
}

/// Convertit une valeur JSON.
pub fn from_json(ty: ColumnType, domain: Domain, json: &Json) -> Result<TypedValue, String> {
    let typed = match (ty.base(), json) {
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
            return from_text(ty, domain, s);
        }
        _ => None,
    };
    let typed = typed.ok_or_else(|| invalid(ty, &json.to_string()))?;
    check(ty, domain, typed)
}

/// Convertit un texte, par exemple un paramètre d'URL (`?montant[gte]=1000`).
pub fn from_text(ty: ColumnType, domain: Domain, text: &str) -> Result<TypedValue, String> {
    let typed = match ty.base() {
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
        _ => None,
    };
    let typed = typed.ok_or_else(|| invalid(ty, &format!("\"{text}\"")))?;
    check(ty, domain, typed)
}

/// Contrôles propres au type de colonne (le type de base est déjà vérifié) ;
/// renvoie la valeur normalisée (couleur en minuscules).
fn check(ty: ColumnType, domain: Domain, typed: TypedValue) -> Result<TypedValue, String> {
    match (ty, typed) {
        (ColumnType::Enum, TypedValue::String(s)) => match domain.values {
            Some(values) if !values.contains(&s) => Err(format!(
                "`{s}` ne fait pas partie des valeurs ({})",
                values.join(", ")
            )),
            _ => Ok(TypedValue::String(s)),
        },
        (ColumnType::Color, TypedValue::String(s)) => {
            let hex = s.strip_prefix('#').unwrap_or_default();
            if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(TypedValue::String(s.to_ascii_lowercase()))
            } else {
                Err(format!("couleur `{s}` invalide : attendu `#rrggbb`"))
            }
        }
        (ColumnType::Email, TypedValue::String(s)) if !is_email(&s) => {
            Err(format!("adresse e-mail `{s}` invalide"))
        }
        (ColumnType::Url, TypedValue::String(s)) if !is_url(&s) => Err(format!(
            "adresse `{s}` invalide : attendu `https://…` ou `http://…`"
        )),
        (ColumnType::Phone, TypedValue::String(s)) if !is_phone(&s) => Err(format!(
            "numéro `{s}` invalide : de 6 à 15 chiffres, avec `+`, espaces, `.`, `-` ou parenthèses"
        )),
        (ColumnType::File | ColumnType::Image, TypedValue::String(s)) if !is_file_id(&s) => {
            Err(format!("identifiant de fichier `{s}` invalide"))
        }
        (ColumnType::Rating, TypedValue::Integer(n)) => {
            let max = domain.max.unwrap_or(Column::DEFAULT_RATING_MAX);
            if (0..=i64::from(max)).contains(&n) {
                Ok(TypedValue::Integer(n))
            } else {
                Err(format!("note {n} invalide : attendu de 0 à {max}"))
            }
        }
        (_, typed) => Ok(typed),
    }
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    s.len() <= 254
        && !local.is_empty()
        && !s.chars().any(char::is_whitespace)
        && !domain.contains('@')
        && domain.contains('.')
        && domain.split('.').all(|part| !part.is_empty())
}

fn is_url(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
        !host.is_empty() && !s.chars().any(char::is_whitespace)
    })
}

fn is_phone(s: &str) -> bool {
    let digits = s.chars().filter(char::is_ascii_digit).count();
    let body = s.strip_prefix('+').unwrap_or(s);
    (6..=15).contains(&digits)
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '.' | '-' | '(' | ')'))
}

/// Identifiant de fichier : UUID en minuscules (`8-4-4-4-12`).
fn is_file_id(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_digit() || ('a'..='f').contains(&c),
        })
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
        ColumnType::Rating => "une note entière",
        ColumnType::Percent => "une proportion (`0.25` pour 25 %)",
        ColumnType::Money => "un montant",
        ColumnType::File | ColumnType::Image => "l'identifiant d'un fichier téléversé",
        ColumnType::ReferenceList | ColumnType::Lookup => "aucune valeur directe",
        _ => "un texte",
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
            from_json(ColumnType::Integer, Domain::default(), &json!(3)),
            Ok(TypedValue::Integer(3))
        );
        assert_eq!(
            from_json(ColumnType::Decimal, Domain::default(), &json!(12.5)),
            Ok(TypedValue::Decimal(Decimal::new(125, 1)))
        );
        assert_eq!(
            from_json(ColumnType::Decimal, Domain::default(), &json!("12.50")),
            Ok(TypedValue::Decimal(Decimal::new(1250, 2)))
        );
        assert_eq!(
            from_json(ColumnType::Date, Domain::default(), &json!(null)),
            Ok(TypedValue::Null)
        );
        assert!(from_json(ColumnType::Integer, Domain::default(), &json!(1.5)).is_err());
        assert!(from_json(ColumnType::Duration, Domain::default(), &json!(-1)).is_err());
        assert!(from_json(ColumnType::Boolean, Domain::default(), &json!("true")).is_err());
        assert!(from_json(ColumnType::Date, Domain::default(), &json!("31/01/2026")).is_err());
    }

    #[test]
    fn enums() {
        let values = ["a".to_owned(), "b".to_owned()];
        assert!(
            from_json(
                ColumnType::Enum,
                Domain {
                    values: Some(&values),
                    max: None
                },
                &json!("a")
            )
            .is_ok()
        );
        let err = from_json(
            ColumnType::Enum,
            Domain {
                values: Some(&values),
                max: None,
            },
            &json!("c"),
        )
        .unwrap_err();
        assert!(err.contains("ne fait pas partie"), "{err}");
    }

    #[test]
    fn field_models() {
        let none = Domain::default();
        assert_eq!(
            from_json(ColumnType::Color, none, &json!("#A1B2C3")),
            Ok(TypedValue::String("#a1b2c3".into()))
        );
        assert!(from_json(ColumnType::Color, none, &json!("red")).is_err());
        assert!(from_json(ColumnType::Email, none, &json!("a@b.fr")).is_ok());
        assert!(from_json(ColumnType::Email, none, &json!("a@b")).is_err());
        assert!(from_json(ColumnType::Url, none, &json!("https://x.fr/a?b")).is_ok());
        assert!(from_json(ColumnType::Url, none, &json!("javascript:alert(1)")).is_err());
        assert!(from_json(ColumnType::Phone, none, &json!("+33 1 23 45 67 89")).is_ok());
        assert!(from_json(ColumnType::Phone, none, &json!("12-ab")).is_err());
        assert!(from_json(ColumnType::Rating, none, &json!(5)).is_ok());
        assert!(from_json(ColumnType::Rating, none, &json!(6)).is_err());
        let ten = Domain {
            values: None,
            max: Some(10),
        };
        assert!(from_json(ColumnType::Rating, ten, &json!(8)).is_ok());
        assert_eq!(
            from_json(ColumnType::Percent, none, &json!("0.25")),
            Ok(TypedValue::Decimal(Decimal::new(25, 2)))
        );
        let id = "0b9f3c1e-5d2a-4c4e-9a8b-1f2e3d4c5b6a";
        assert!(from_json(ColumnType::Image, none, &json!(id)).is_ok());
        assert!(from_json(ColumnType::File, none, &json!("../etc/passwd")).is_err());
    }

    #[test]
    fn text_values() {
        assert_eq!(
            from_text(ColumnType::Boolean, Domain::default(), "true"),
            Ok(TypedValue::Boolean(true))
        );
        assert_eq!(
            from_text(
                ColumnType::Datetime,
                Domain::default(),
                "2026-01-31T10:00:00+01:00"
            ),
            Ok(TypedValue::Datetime(
                "2026-01-31T09:00:00Z".parse().unwrap()
            ))
        );
        assert!(from_text(ColumnType::Integer, Domain::default(), "x").is_err());
    }
}
