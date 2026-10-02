//! Vérification des types d'une expression dont les références sont valides.

use crate::ast::{BinaryOp, Expr, ExprKind, Span, UnaryOp, VarScope};
use crate::functions::{FunctionKind, FunctionRegistry, Returns};
use crate::value::Type;

/// Types des données accessibles, fournis par l'appelant.
pub trait TypeEnv {
    fn path_type(&self, path: &[String]) -> Type;
    fn variable_type(&self, scope: VarScope, path: &[String]) -> Type;
    /// Type des éléments d'une relation « plusieurs » (`opportunites.montant`).
    fn collection_type(&self, path: &[String]) -> Type;
}

/// Erreur de type, localisée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

/// Type de `expr`, ou toutes les erreurs trouvées.
pub fn typecheck(
    expr: &Expr,
    env: &dyn TypeEnv,
    functions: &FunctionRegistry,
) -> Result<Type, Vec<TypeError>> {
    let mut checker = Checker {
        env,
        functions,
        errors: Vec::new(),
    };
    let ty = checker.check(expr);
    if checker.errors.is_empty() {
        Ok(ty)
    } else {
        Err(checker.errors)
    }
}

struct Checker<'a> {
    env: &'a dyn TypeEnv,
    functions: &'a FunctionRegistry,
    errors: Vec<TypeError>,
}

/// Un argument `date-heure` convient là où une `date` est attendue.
fn fits_param(actual: Type, expected: Type) -> bool {
    actual.fits(expected) || (actual == Type::DateTime && expected == Type::Date)
}

impl Checker<'_> {
    fn error(&mut self, expr: &Expr, message: String) -> Type {
        self.errors.push(TypeError {
            message,
            span: expr.span,
        });
        Type::Any
    }

    fn check(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Null => Type::Any,
            ExprKind::Number(_) => Type::Number,
            ExprKind::String(_) => Type::Text,
            ExprKind::Bool(_) => Type::Boolean,
            ExprKind::Path(path) => self.env.path_type(path),
            ExprKind::Var { scope, path } => self.env.variable_type(*scope, path),
            ExprKind::Unary { op, expr: inner } => {
                let (expected, ty) = match op {
                    UnaryOp::Neg => (Type::Number, self.check(inner)),
                    UnaryOp::Not => (Type::Boolean, self.check(inner)),
                };
                if ty.fits(expected) {
                    expected
                } else {
                    self.error(expr, format!("{expected} attendu, {ty} trouvé"))
                }
            }
            ExprKind::Binary { op, left, right } => {
                let (l, r) = (self.check(left), self.check(right));
                self.binary(expr, *op, l, r)
            }
            ExprKind::Call { name, args } => self.call(expr, name, args),
        }
    }

    fn binary(&mut self, expr: &Expr, op: BinaryOp, l: Type, r: Type) -> Type {
        let symbol = match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
        };
        let ok = match op {
            BinaryOp::Add | BinaryOp::Sub if l == Type::Date => r.fits(Type::Number),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                l.fits(Type::Number) && r.fits(Type::Number)
            }
            BinaryOp::Eq | BinaryOp::Ne => l.fits(r),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                l.fits(r) && l.is_ordered() && r.is_ordered()
            }
            BinaryOp::And | BinaryOp::Or => l.fits(Type::Boolean) && r.fits(Type::Boolean),
        };
        if !ok {
            return self.error(
                expr,
                format!("opération `{symbol}` impossible entre {l} et {r}"),
            );
        }
        match op {
            BinaryOp::Add | BinaryOp::Sub if l == Type::Date => Type::Date,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => Type::Number,
            _ => Type::Boolean,
        }
    }

    fn call(&mut self, expr: &Expr, name: &str, args: &[Expr]) -> Type {
        let Some(signature) = self.functions.get(name).cloned() else {
            return Type::Any; // signalé par la validation des références
        };
        let types: Vec<Type> = if signature.kind == FunctionKind::Aggregate {
            args.iter()
                .map(|a| match &a.kind {
                    ExprKind::Path(path) => self.env.collection_type(path),
                    _ => Type::Any,
                })
                .collect()
        } else {
            args.iter().map(|a| self.check(a)).collect()
        };
        for (index, (arg, ty)) in args.iter().zip(&types).enumerate() {
            let expected = signature.param(index);
            if !fits_param(*ty, expected) {
                self.error(
                    arg,
                    format!(
                        "`{name}` : argument {} de type {expected} attendu, {ty} trouvé",
                        index + 1
                    ),
                );
            }
        }
        match signature.returns {
            Returns::Fixed(ty) => ty,
            Returns::FirstArgument => {
                let ty = types.first().copied().unwrap_or(Type::Any);
                if ty.is_ordered() {
                    ty
                } else {
                    self.error(expr, format!("`{name}` : valeurs non comparables ({ty})"))
                }
            }
            Returns::Branches => match (types.get(1).copied(), types.get(2).copied()) {
                (Some(a), Some(b)) if a.fits(b) => {
                    if a == Type::Any {
                        b
                    } else {
                        a
                    }
                }
                (Some(a), Some(b)) => self.error(
                    expr,
                    format!("`{name}` : branches de types différents ({a} et {b})"),
                ),
                _ => Type::Any,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    struct Env;

    impl TypeEnv for Env {
        fn path_type(&self, path: &[String]) -> Type {
            match path.join(".").as_str() {
                "montant" => Type::Number,
                "titre" | "entreprise.nom" => Type::Text,
                "cloture" => Type::Date,
                "debut" => Type::DateTime,
                "actif" => Type::Boolean,
                _ => Type::Any,
            }
        }

        fn variable_type(&self, scope: VarScope, _: &[String]) -> Type {
            match scope {
                VarScope::Param => Type::Number,
                VarScope::User => Type::Text,
            }
        }

        fn collection_type(&self, path: &[String]) -> Type {
            if path.len() == 1 {
                Type::Any
            } else {
                self.path_type(&path[1..])
            }
        }
    }

    fn check(src: &str) -> Result<Type, Vec<String>> {
        typecheck(&parse(src).unwrap(), &Env, &FunctionRegistry::builtin())
            .map_err(|errors| errors.into_iter().map(|e| e.message).collect())
    }

    #[test]
    fn well_typed() {
        assert_eq!(check("montant * 2 + $param.tva"), Ok(Type::Number));
        assert_eq!(check("montant > 10 AND NOT actif"), Ok(Type::Boolean));
        assert_eq!(check("CONCAT(titre, montant)"), Ok(Type::Text));
        assert_eq!(check("IF(actif, montant, NULL)"), Ok(Type::Number));
        assert_eq!(check("DAYS_BETWEEN(TODAY(), debut)"), Ok(Type::Number));
        assert_eq!(check("cloture + 30"), Ok(Type::Date));
        assert_eq!(check("SUM(lignes.montant)"), Ok(Type::Number));
        assert_eq!(check("MAX(lignes.cloture)"), Ok(Type::Date));
        assert_eq!(check("COUNT(lignes)"), Ok(Type::Number));
        assert_eq!(check("titre == NULL"), Ok(Type::Boolean));
    }

    #[test]
    fn type_errors() {
        assert_eq!(
            check("montant + titre"),
            Err(vec![
                "opération `+` impossible entre nombre et texte".into()
            ])
        );
        assert_eq!(
            check("-titre"),
            Err(vec!["nombre attendu, texte trouvé".into()])
        );
        assert_eq!(
            check("actif AND montant"),
            Err(vec![
                "opération `AND` impossible entre booléen et nombre".into()
            ])
        );
        assert_eq!(
            check("SUM(lignes.titre)"),
            Err(vec![
                "`SUM` : argument 1 de type nombre attendu, texte trouvé".into()
            ])
        );
        assert_eq!(
            check("IF(montant, 1, 2)"),
            Err(vec![
                "`IF` : argument 1 de type booléen attendu, nombre trouvé".into()
            ])
        );
        assert_eq!(
            check("IF(actif, 1, \"a\")"),
            Err(vec![
                "`IF` : branches de types différents (nombre et texte)".into()
            ])
        );
        assert_eq!(
            check("MAX(lignes.actif)"),
            Err(vec!["`MAX` : valeurs non comparables (booléen)".into()])
        );
        assert_eq!(check("titre < 3").map_err(|e| e.len()), Err(1));
    }
}
