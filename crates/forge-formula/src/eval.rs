//! Évaluation d'une expression déjà validée (références et types).
//!
//! `NULL` se propage : une opération sur `NULL` donne `NULL`, sauf `== NULL`,
//! `!= NULL`, `AND`/`OR` (logique à trois valeurs) et `IF`, dont la condition
//! `NULL` choisit la branche « sinon ». La division par zéro donne `NULL`.

use std::cmp::Ordering;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::ast::{BinaryOp, Expr, ExprKind, UnaryOp, VarScope};
use crate::functions::{FunctionKind, FunctionRegistry};
use crate::value::Value;

/// Données accessibles à une formule, fournies par l'appelant.
pub trait Env {
    /// Valeur d'une colonne ou d'un chemin de références (`montant`, `entreprise.secteur`).
    fn path(&self, path: &[String]) -> Value;
    /// Variable de contexte (`$param.tva`, `$user.id`).
    fn variable(&self, scope: VarScope, path: &[String]) -> Value;
    /// Valeurs d'une relation « plusieurs », argument d'un agrégat
    /// (`opportunites.montant`) ; pour une relation seule (`COUNT(tags)`),
    /// une valeur non nulle par enregistrement lié.
    fn collection(&self, path: &[String]) -> Vec<Value>;
}

/// Erreur d'évaluation : débordement, erreur d'une fonction personnalisée…
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EvalError(pub String);

/// Évalue `expr` dans `env`.
pub fn evaluate(
    expr: &Expr,
    env: &dyn Env,
    functions: &FunctionRegistry,
) -> Result<Value, EvalError> {
    Evaluator { env, functions }.eval(expr)
}

struct Evaluator<'a> {
    env: &'a dyn Env,
    functions: &'a FunctionRegistry,
}

impl Evaluator<'_> {
    fn eval(&self, expr: &Expr) -> Result<Value, EvalError> {
        Ok(match &expr.kind {
            ExprKind::Null => Value::Null,
            ExprKind::Number(n) => Value::Number(*n),
            ExprKind::String(s) => Value::Text(s.clone()),
            ExprKind::Bool(b) => Value::Boolean(*b),
            ExprKind::Path(path) => self.env.path(path),
            ExprKind::Var { scope, path } => self.env.variable(*scope, path),
            ExprKind::Unary { op, expr } => match (op, self.eval(expr)?) {
                (UnaryOp::Neg, Value::Number(n)) => Value::Number(-n),
                (UnaryOp::Not, Value::Boolean(b)) => Value::Boolean(!b),
                _ => Value::Null,
            },
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right)?,
            ExprKind::Call { name, args } => self.call(name, args)?,
        })
    }

    fn binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Result<Value, EvalError> {
        let (l, r) = (self.eval(left)?, self.eval(right)?);
        Ok(match op {
            BinaryOp::And => match (l, r) {
                (Value::Boolean(false), _) | (_, Value::Boolean(false)) => Value::Boolean(false),
                (Value::Boolean(true), Value::Boolean(true)) => Value::Boolean(true),
                _ => Value::Null,
            },
            BinaryOp::Or => match (l, r) {
                (Value::Boolean(true), _) | (_, Value::Boolean(true)) => Value::Boolean(true),
                (Value::Boolean(false), Value::Boolean(false)) => Value::Boolean(false),
                _ => Value::Null,
            },
            BinaryOp::Eq | BinaryOp::Ne if l.is_null() || r.is_null() => {
                Value::Boolean((l.is_null() && r.is_null()) == (op == BinaryOp::Eq))
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => l.compare(&r).map_or(Value::Null, |ordering| {
                Value::Boolean(match op {
                    BinaryOp::Eq => ordering == Ordering::Equal,
                    BinaryOp::Ne => ordering != Ordering::Equal,
                    BinaryOp::Lt => ordering == Ordering::Less,
                    BinaryOp::Le => ordering != Ordering::Greater,
                    BinaryOp::Gt => ordering == Ordering::Greater,
                    _ => ordering != Ordering::Less,
                })
            }),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                arithmetic(op, &l, &r)?
            }
        })
    }

    fn call(&self, name: &str, args: &[Expr]) -> Result<Value, EvalError> {
        let signature = self
            .functions
            .get(name)
            .ok_or_else(|| EvalError(format!("fonction `{name}` inconnue")))?;
        if signature.kind == FunctionKind::Aggregate {
            let ExprKind::Path(path) = &args[0].kind else {
                return Err(EvalError(format!("`{name}` attend une relation")));
            };
            return aggregate(name, self.env.collection(path));
        }
        if name == "IF" {
            return match self.eval(&args[0])? {
                Value::Boolean(true) => self.eval(&args[1]),
                _ => self.eval(&args[2]),
            };
        }
        let values = args
            .iter()
            .map(|a| self.eval(a))
            .collect::<Result<Vec<_>, _>>()?;
        let implementation = self
            .functions
            .implementation(name)
            .ok_or_else(|| EvalError(format!("fonction `{name}` déclarée mais non implémentée")))?;
        implementation(&values).map_err(|message| EvalError(format!("{name} : {message}")))
    }
}

fn arithmetic(op: BinaryOp, l: &Value, r: &Value) -> Result<Value, EvalError> {
    let overflow = || EvalError("dépassement de capacité numérique".into());
    Ok(match (l, r) {
        (Value::Number(a), Value::Number(b)) => Value::Number(match op {
            BinaryOp::Add => a.checked_add(*b).ok_or_else(overflow)?,
            BinaryOp::Sub => a.checked_sub(*b).ok_or_else(overflow)?,
            BinaryOp::Mul => a.checked_mul(*b).ok_or_else(overflow)?,
            _ if b.is_zero() => return Ok(Value::Null),
            _ => a.checked_div(*b).ok_or_else(overflow)?,
        }),
        // Date ± nombre de jours.
        (Value::Date(date), Value::Number(days)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            let days = days.trunc().to_i64().ok_or_else(overflow)?;
            let days = if op == BinaryOp::Sub { -days } else { days };
            date.checked_add_signed(chrono::Duration::days(days))
                .map_or(Value::Null, Value::Date)
        }
        _ => Value::Null,
    })
}

fn aggregate(name: &str, values: Vec<Value>) -> Result<Value, EvalError> {
    let present: Vec<Value> = values.into_iter().filter(|v| !v.is_null()).collect();
    let numbers = || {
        present.iter().filter_map(|v| match v {
            Value::Number(n) => Some(*n),
            _ => None,
        })
    };
    Ok(match name {
        "COUNT" => Value::Number(Decimal::from(present.len())),
        "SUM" => Value::Number(
            numbers()
                .try_fold(Decimal::ZERO, Decimal::checked_add)
                .ok_or_else(|| EvalError("dépassement de capacité numérique".into()))?,
        ),
        "AVG" if present.is_empty() => Value::Null,
        "AVG" => {
            let sum = numbers()
                .try_fold(Decimal::ZERO, Decimal::checked_add)
                .ok_or_else(|| EvalError("dépassement de capacité numérique".into()))?;
            Value::Number(sum / Decimal::from(present.len()))
        }
        "MIN" | "MAX" => {
            let wanted = if name == "MIN" {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            present
                .into_iter()
                .reduce(|best, v| {
                    if v.compare(&best) == Some(wanted) {
                        v
                    } else {
                        best
                    }
                })
                .unwrap_or(Value::Null)
        }
        other => return Err(EvalError(format!("agrégat `{other}` inconnu"))),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::parse;

    struct TestEnv {
        columns: BTreeMap<&'static str, Value>,
        lignes: Vec<Value>,
    }

    impl Env for TestEnv {
        fn path(&self, path: &[String]) -> Value {
            self.columns
                .get(path.join(".").as_str())
                .cloned()
                .unwrap_or(Value::Null)
        }

        fn variable(&self, _: VarScope, path: &[String]) -> Value {
            if path[0] == "tva" {
                n("20")
            } else {
                Value::Null
            }
        }

        fn collection(&self, _: &[String]) -> Vec<Value> {
            self.lignes.clone()
        }
    }

    fn n(s: &str) -> Value {
        Value::Number(s.parse().unwrap())
    }

    fn eval(src: &str) -> Value {
        let env = TestEnv {
            columns: BTreeMap::from([
                ("montant", n("1000")),
                ("probabilite", n("30")),
                ("titre", Value::Text("Contrat".into())),
                ("cloture", Value::Date("2026-03-01".parse().unwrap())),
                ("entreprise.nom", Value::Text("Acme".into())),
            ]),
            lignes: vec![n("10"), Value::Null, n("2.5")],
        };
        evaluate(&parse(src).unwrap(), &env, &FunctionRegistry::builtin()).unwrap()
    }

    #[test]
    fn arithmetic_and_parameters() {
        assert_eq!(eval("montant * probabilite / 100"), n("300"));
        assert_eq!(
            eval("ROUND(montant * (1 + $param.tva / 100), 2)"),
            n("1200")
        );
        assert_eq!(eval("-montant + 1"), n("-999"));
        assert_eq!(eval("montant / 0"), Value::Null);
        assert_eq!(eval("inconnu + 1"), Value::Null);
        assert_eq!(
            eval("cloture + 30"),
            Value::Date("2026-03-31".parse().unwrap())
        );
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(
            eval("montant >= 1000 AND titre == \"Contrat\""),
            Value::Boolean(true)
        );
        assert_eq!(eval("inconnu == NULL"), Value::Boolean(true));
        assert_eq!(eval("montant != NULL"), Value::Boolean(true));
        assert_eq!(eval("inconnu > 1"), Value::Null);
        assert_eq!(eval("inconnu > 1 OR TRUE"), Value::Boolean(true));
        assert_eq!(eval("inconnu > 1 AND FALSE"), Value::Boolean(false));
        assert_eq!(eval("NOT (montant < 10)"), Value::Boolean(true));
    }

    #[test]
    fn functions() {
        assert_eq!(
            eval("IF(montant > 500, \"gros\", \"petit\")"),
            Value::Text("gros".into())
        );
        assert_eq!(eval("IF(inconnu, 1, 2)"), n("2"));
        assert_eq!(
            eval("CONCAT(entreprise.nom, \" - \", titre, inconnu)"),
            Value::Text("Acme - Contrat".into())
        );
        assert!(eval("CONCAT(montant / 3)").to_string().len() > 5);
    }

    #[test]
    fn aggregates_ignore_nulls() {
        assert_eq!(eval("SUM(lignes.x)"), n("12.5"));
        assert_eq!(eval("COUNT(lignes)"), n("2"));
        assert_eq!(eval("AVG(lignes.x)"), n("6.25"));
        assert_eq!(eval("MIN(lignes.x)"), n("2.5"));
        assert_eq!(eval("MAX(lignes.x)"), n("10"));
    }

    #[test]
    fn custom_functions() {
        use crate::{FunctionSignature, Type};
        let mut functions = FunctionRegistry::builtin();
        functions.declare(FunctionSignature::scalar(
            "DOUBLE",
            vec![Type::Number],
            Type::Number,
        ));
        let env = TestEnv {
            columns: BTreeMap::new(),
            lignes: Vec::new(),
        };
        let expr = parse("DOUBLE(21)").unwrap();
        let err = evaluate(&expr, &env, &functions).unwrap_err();
        assert!(err.0.contains("non implémentée"), "{err}");
        functions
            .implement("DOUBLE", |args| match &args[0] {
                Value::Number(n) => Ok(Value::Number(n * Decimal::TWO)),
                _ => Err("nombre attendu".into()),
            })
            .unwrap();
        assert_eq!(evaluate(&expr, &env, &functions).unwrap(), n("42"));
        let err = evaluate(&parse("DOUBLE(\"x\")").unwrap(), &env, &functions).unwrap_err();
        assert_eq!(err.0, "DOUBLE : nombre attendu");
    }
}
