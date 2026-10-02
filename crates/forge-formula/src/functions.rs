use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::value::{Type, Value};

/// Nature d'une fonction : opère sur des valeurs ou sur une relation « plusieurs ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    Scalar,
    /// Agrège une relation : `SUM(opportunites.montant)`, `COUNT(tags)`.
    Aggregate,
}

/// Nombre d'arguments accepté, bornes incluses ; `max = None` signifie illimité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arity {
    pub min: usize,
    pub max: Option<usize>,
}

impl Arity {
    pub const fn exactly(n: usize) -> Self {
        Self {
            min: n,
            max: Some(n),
        }
    }

    pub const fn between(min: usize, max: usize) -> Self {
        Self {
            min,
            max: Some(max),
        }
    }

    pub const fn at_least(min: usize) -> Self {
        Self { min, max: None }
    }

    pub fn accepts(self, count: usize) -> bool {
        count >= self.min && self.max.is_none_or(|max| count <= max)
    }
}

impl fmt::Display for Arity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.max {
            Some(max) if max == self.min => write!(f, "{max}"),
            Some(max) => write!(f, "{} à {max}", self.min),
            None => write!(f, "au moins {}", self.min),
        }
    }
}

/// Type du résultat d'une fonction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Returns {
    Fixed(Type),
    /// Type du premier argument (`MIN`, `MAX`).
    FirstArgument,
    /// Type commun des deux branches (`IF`).
    Branches,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    /// Nom en majuscules.
    pub name: String,
    pub kind: FunctionKind,
    pub arity: Arity,
    /// Type attendu de chaque argument ; le dernier vaut pour les arguments suivants.
    pub params: Vec<Type>,
    pub returns: Returns,
    /// Le résultat dépend d'autre chose que des arguments (ex. `TODAY`) :
    /// interdit dans une formule persistée, dont la valeur deviendrait fausse.
    pub volatile: bool,
}

impl FunctionSignature {
    /// Fonction scalaire à nombre d'arguments fixe.
    pub fn scalar(name: &str, params: Vec<Type>, returns: Type) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            kind: FunctionKind::Scalar,
            arity: Arity::exactly(params.len()),
            params,
            returns: Returns::Fixed(returns),
            volatile: false,
        }
    }

    fn aggregate(name: &str, param: Type, returns: Returns) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            kind: FunctionKind::Aggregate,
            arity: Arity::exactly(1),
            params: vec![param],
            returns,
            volatile: false,
        }
    }

    #[must_use]
    pub fn with_arity(mut self, arity: Arity) -> Self {
        self.arity = arity;
        self
    }

    #[must_use]
    pub fn volatile(mut self) -> Self {
        self.volatile = true;
        self
    }

    /// Type attendu de l'argument `index`.
    pub fn param(&self, index: usize) -> Type {
        self.params
            .get(index)
            .or(self.params.last())
            .copied()
            .unwrap_or(Type::Any)
    }
}

/// Implémentation d'une fonction scalaire ; une erreur interrompt l'évaluation.
pub type Implementation = Arc<dyn Fn(&[Value]) -> Result<Value, String> + Send + Sync>;

/// Registre des fonctions : signatures (pour la validation) et implémentations
/// (pour l'évaluation). Les fonctions personnalisées sont déclarées dans le
/// schéma, puis implémentées par l'application.
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    signatures: BTreeMap<String, FunctionSignature>,
    implementations: BTreeMap<String, Implementation>,
}

impl fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionRegistry")
            .field("functions", &self.signatures.keys().collect::<Vec<_>>())
            .field(
                "implemented",
                &self.implementations.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl FunctionRegistry {
    /// Fonctions fournies par forge, toutes implémentées.
    pub fn builtin() -> Self {
        use Type::{Any, Boolean, Date, Number, Text};

        let mut registry = Self::default();
        for signature in [
            FunctionSignature {
                returns: Returns::Branches,
                ..FunctionSignature::scalar("IF", vec![Boolean, Any, Any], Any)
            },
            FunctionSignature::scalar("ROUND", vec![Number, Number], Number)
                .with_arity(Arity::between(1, 2)),
            FunctionSignature::scalar("CONCAT", vec![Any], Text).with_arity(Arity::at_least(1)),
            FunctionSignature::scalar("DAYS_BETWEEN", vec![Date, Date], Number),
            FunctionSignature::scalar("TODAY", Vec::new(), Date).volatile(),
            FunctionSignature::aggregate("SUM", Number, Returns::Fixed(Number)),
            FunctionSignature::aggregate("AVG", Number, Returns::Fixed(Number)),
            FunctionSignature::aggregate("MIN", Any, Returns::FirstArgument),
            FunctionSignature::aggregate("MAX", Any, Returns::FirstArgument),
            FunctionSignature::aggregate("COUNT", Any, Returns::Fixed(Number)),
        ] {
            registry.declare(signature);
        }
        registry.implement_unchecked("ROUND", round);
        registry.implement_unchecked("CONCAT", |args| {
            Ok(Value::Text(args.iter().map(ToString::to_string).collect()))
        });
        registry.implement_unchecked("DAYS_BETWEEN", |args| Ok(days_between(args)));
        registry.implement_unchecked("TODAY", |_| {
            Ok(Value::Date(chrono::Utc::now().date_naive()))
        });
        registry
    }

    /// Ajoute ou remplace une signature.
    pub fn declare(&mut self, signature: FunctionSignature) {
        self.signatures.insert(signature.name.clone(), signature);
    }

    /// Fournit l'implémentation d'une fonction scalaire déclarée.
    pub fn implement(
        &mut self,
        name: &str,
        implementation: impl Fn(&[Value]) -> Result<Value, String> + Send + Sync + 'static,
    ) -> Result<(), String> {
        match self.get(name) {
            None => Err(format!(
                "fonction `{name}` non déclarée dans le schéma (`functions`)"
            )),
            Some(s) if s.kind == FunctionKind::Aggregate => {
                Err(format!("`{name}` est un agrégat intégré"))
            }
            Some(_) => {
                self.implement_unchecked(name, implementation);
                Ok(())
            }
        }
    }

    fn implement_unchecked(
        &mut self,
        name: &str,
        implementation: impl Fn(&[Value]) -> Result<Value, String> + Send + Sync + 'static,
    ) {
        self.implementations
            .insert(name.to_ascii_uppercase(), Arc::new(implementation));
    }

    /// Recherche insensible à la casse.
    pub fn get(&self, name: &str) -> Option<&FunctionSignature> {
        self.signatures.get(&name.to_ascii_uppercase())
    }

    pub(crate) fn implementation(&self, name: &str) -> Option<&Implementation> {
        self.implementations.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.signatures.keys().map(String::as_str)
    }

    /// Fonctions scalaires déclarées sans implémentation.
    pub fn missing_implementations(&self) -> Vec<&str> {
        self.signatures
            .values()
            .filter(|s| s.kind == FunctionKind::Scalar && s.name != "IF")
            .filter(|s| !self.implementations.contains_key(&s.name))
            .map(|s| s.name.as_str())
            .collect()
    }
}

/// `ROUND(x, décimales = 0)`, arrondi au plus proche, demi s'éloignant de zéro (comme Excel).
fn round(args: &[Value]) -> Result<Value, String> {
    let places = match args.get(1) {
        None => 0,
        Some(Value::Number(n)) => n.to_u32().ok_or("nombre de décimales invalide")?,
        Some(_) => return Ok(Value::Null),
    };
    Ok(match &args[0] {
        Value::Number(n) => {
            Value::Number(n.round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero))
        }
        _ => Value::Null,
    })
}

/// `DAYS_BETWEEN(début, fin)` : nombre de jours de `début` à `fin`.
fn days_between(args: &[Value]) -> Value {
    let day = |v: &Value| match v {
        Value::Date(d) => Some(*d),
        Value::DateTime(d) => Some(d.date_naive()),
        _ => None,
    };
    match (day(&args[0]), day(&args[1])) {
        (Some(start), Some(end)) => Value::Number(Decimal::from((end - start).num_days())),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arity() {
        assert!(Arity::exactly(2).accepts(2));
        assert!(!Arity::exactly(2).accepts(3));
        assert!(Arity::between(1, 2).accepts(1));
        assert!(Arity::at_least(1).accepts(10));
        assert!(!Arity::at_least(1).accepts(0));
        assert_eq!(Arity::between(1, 2).to_string(), "1 à 2");
    }

    #[test]
    fn declared_functions_need_an_implementation() {
        let mut registry = FunctionRegistry::builtin();
        assert!(registry.get("sum").is_some());
        assert!(registry.get("TODAY").unwrap().volatile);
        assert!(registry.missing_implementations().is_empty());
        assert!(
            registry.implement("TVA", |_| Ok(Value::Null)).is_err(),
            "non déclarée"
        );

        registry.declare(FunctionSignature::scalar(
            "tva",
            vec![Type::Number],
            Type::Number,
        ));
        assert_eq!(registry.missing_implementations(), ["TVA"]);
        registry
            .implement("Tva", |args| Ok(args[0].clone()))
            .unwrap();
        assert!(registry.missing_implementations().is_empty());
        assert!(
            registry.implement("SUM", |_| Ok(Value::Null)).is_err(),
            "agrégat"
        );
    }

    #[test]
    fn builtin_implementations() {
        let n = |s: &str| Value::Number(s.parse().unwrap());
        assert_eq!(round(&[n("2.5")]).unwrap(), n("3"));
        assert_eq!(round(&[n("-2.345"), n("2")]).unwrap(), n("-2.35"));
        let date = |s: &str| Value::Date(s.parse().unwrap());
        assert_eq!(
            days_between(&[date("2026-01-01"), date("2026-03-01")]),
            n("59")
        );
        assert_eq!(
            days_between(&[Value::Null, date("2026-03-01")]),
            Value::Null
        );
    }
}
