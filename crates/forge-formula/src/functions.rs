use std::collections::BTreeMap;

/// Nature d'une fonction : opère sur une valeur ou sur une relation « plusieurs ».
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

impl std::fmt::Display for Arity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.max {
            Some(max) if max == self.min => write!(f, "{max}"),
            Some(max) => write!(f, "{} à {max}", self.min),
            None => write!(f, "au moins {}", self.min),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    /// Nom en majuscules.
    pub name: String,
    pub kind: FunctionKind,
    pub arity: Arity,
    /// Le résultat dépend d'autre chose que des arguments (ex. `TODAY`) :
    /// interdit dans une formule persistée, dont la valeur deviendrait fausse.
    pub volatile: bool,
}

impl FunctionSignature {
    pub fn scalar(name: &str, arity: Arity) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            kind: FunctionKind::Scalar,
            arity,
            volatile: false,
        }
    }

    pub fn aggregate(name: &str) -> Self {
        Self {
            name: name.to_ascii_uppercase(),
            kind: FunctionKind::Aggregate,
            arity: Arity::exactly(1),
            volatile: false,
        }
    }

    #[must_use]
    pub fn volatile(mut self) -> Self {
        self.volatile = true;
        self
    }
}

/// Registre des fonctions connues ; extensible via [`FunctionRegistry::register`].
#[derive(Debug, Clone, Default)]
pub struct FunctionRegistry {
    functions: BTreeMap<String, FunctionSignature>,
}

impl FunctionRegistry {
    /// Fonctions fournies par forge.
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for signature in [
            FunctionSignature::scalar("IF", Arity::exactly(3)),
            FunctionSignature::scalar("ROUND", Arity::between(1, 2)),
            FunctionSignature::scalar("CONCAT", Arity::at_least(1)),
            FunctionSignature::scalar("DAYS_BETWEEN", Arity::exactly(2)),
            FunctionSignature::scalar("TODAY", Arity::exactly(0)).volatile(),
            FunctionSignature::aggregate("SUM"),
            FunctionSignature::aggregate("AVG"),
            FunctionSignature::aggregate("MIN"),
            FunctionSignature::aggregate("MAX"),
            FunctionSignature::aggregate("COUNT"),
        ] {
            registry.register(signature);
        }
        registry
    }

    /// Ajoute ou remplace une fonction.
    pub fn register(&mut self, signature: FunctionSignature) {
        self.functions.insert(signature.name.clone(), signature);
    }

    /// Recherche insensible à la casse.
    pub fn get(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(&name.to_ascii_uppercase())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(String::as_str)
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
    fn registry_is_case_insensitive_and_extensible() {
        let mut registry = FunctionRegistry::builtin();
        assert!(registry.get("sum").is_some());
        assert!(registry.get("TODAY").unwrap().volatile);
        assert!(registry.get("TVA").is_none());

        registry.register(FunctionSignature::scalar("tva", Arity::exactly(1)));
        assert_eq!(registry.get("Tva").unwrap().name, "TVA");
    }
}
