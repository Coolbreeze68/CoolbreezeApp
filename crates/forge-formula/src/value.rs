use std::cmp::Ordering;
use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

/// Type d'une expression, vérifié avant toute évaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// Entiers, décimaux et durées (en secondes).
    Number,
    Text,
    Boolean,
    Date,
    DateTime,
    /// Type inconnu, compatible avec tous les autres (`NULL`).
    Any,
}

impl Type {
    /// Nom tel qu'écrit dans le schéma (`functions[].args`, `returns`).
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "number" => Self::Number,
            "text" => Self::Text,
            "boolean" => Self::Boolean,
            "date" => Self::Date,
            "datetime" => Self::DateTime,
            "any" => Self::Any,
            _ => return None,
        })
    }

    /// `self` peut-il être utilisé là où `expected` est attendu ?
    pub fn fits(self, expected: Self) -> bool {
        self == expected || self == Self::Any || expected == Self::Any
    }

    pub fn is_ordered(self) -> bool {
        matches!(
            self,
            Self::Number | Self::Text | Self::Date | Self::DateTime | Self::Any
        )
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Number => "nombre",
            Self::Text => "texte",
            Self::Boolean => "booléen",
            Self::Date => "date",
            Self::DateTime => "date-heure",
            Self::Any => "valeur",
        })
    }
}

/// Valeur manipulée par l'évaluateur.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Number(Decimal),
    Text(String),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(DateTime<Utc>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Comparaison entre valeurs de même type ; `None` si incomparables ou `NULL`.
    pub fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Some(a.cmp(b)),
            (Self::Text(a), Self::Text(b)) => Some(a.cmp(b)),
            (Self::Boolean(a), Self::Boolean(b)) => Some(a.cmp(b)),
            (Self::Date(a), Self::Date(b)) => Some(a.cmp(b)),
            (Self::DateTime(a), Self::DateTime(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

impl From<Decimal> for Value {
    fn from(n: Decimal) -> Self {
        Self::Number(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Number(n.into())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

/// Représentation textuelle, utilisée par `CONCAT` (`NULL` donne une chaîne vide).
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => Ok(()),
            Self::Number(n) => write!(f, "{}", n.normalize()),
            Self::Text(s) => f.write_str(s),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Date(d) => write!(f, "{d}"),
            Self::DateTime(d) => write!(f, "{}", d.to_rfc3339()),
        }
    }
}
