//! Autorisations : règles du schéma et conditions `when` traduites en SQL.
//!
//! Pour une table, une action et un utilisateur :
//! - le rôle `admin` a tous les droits ;
//! - une règle s'applique si l'un des rôles de l'utilisateur y figure et que
//!   l'action fait partie de ses `actions` (ou `*`) ;
//! - aucune règle applicable : refus (403) ;
//! - une règle applicable sans `when` : accès à tous les enregistrements ;
//! - sinon : accès aux enregistrements vérifiant au moins une des conditions.
//!
//! Les conditions deviennent des clauses SQL : elles filtrent les listes autant
//! qu'elles contrôlent les accès unitaires.

use std::collections::BTreeMap;

use forge_formula::{BinaryOp, Expr, ExprKind, UnaryOp, VarScope};
use forge_schema::spec::{Action, Table};
use forge_schema::value::{self, TypedValue};
use rust_decimal::prelude::ToPrimitive;
use sea_orm::sea_query::{Alias, ExprTrait, SimpleExpr};
use sea_orm::{ConnectionTrait, Value};
use serde_json::Value as JsonValue;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::Error;
use crate::{parameters, values};

/// Portée d'une action autorisée.
#[derive(Debug, Clone)]
pub(crate) enum Scope {
    /// Tous les enregistrements.
    All,
    /// Les enregistrements vérifiant la condition.
    Where(SimpleExpr),
}

impl Scope {
    pub(crate) fn condition(&self) -> Option<SimpleExpr> {
        match self {
            Self::All => None,
            Self::Where(condition) => Some(condition.clone()),
        }
    }
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Read => "lecture",
        Action::Create => "création",
        Action::Update => "modification",
        Action::Delete => "suppression",
        Action::All => "toute action",
    }
}

/// Portée de `action` sur `table` pour `user`, ou refus (403).
pub(crate) async fn scope(
    state: &AppState,
    table: &Table,
    action: Action,
    user: &CurrentUser,
) -> Result<Scope, Error> {
    if user.is_admin() {
        return Ok(Scope::All);
    }
    let applicable: Vec<(usize, bool)> = table
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            rule.roles.iter().any(|r| user.has_role(r))
                && rule
                    .actions
                    .iter()
                    .any(|a| *a == action || *a == Action::All)
        })
        .map(|(index, rule)| (index, rule.when.is_some()))
        .collect();
    if applicable.is_empty() {
        return Err(Error::Forbidden(format!(
            "{} non autorisée sur `{}`",
            action_name(action),
            table.name
        )));
    }
    if applicable.iter().any(|(_, conditional)| !conditional) {
        return Ok(Scope::All);
    }

    let context = Context {
        table: &table.name,
        user,
        parameters: parameters::values(&state.db, &state.model.spec().parameters).await?,
    };
    let condition = applicable
        .iter()
        .map(|(index, _)| {
            let expr = state
                .model
                .condition(&table.name, *index)
                .expect("condition analysée par le schéma");
            context.boolean(expr)
        })
        .reduce(ExprTrait::or)
        .expect("au moins une règle conditionnelle");
    Ok(Scope::Where(condition))
}

/// Vrai si l'enregistrement `id` de l'entité `E` vérifie `scope`.
pub(crate) async fn allows<E: crate::ForgeEntity>(
    db: &impl ConnectionTrait,
    scope: &Scope,
    id: i64,
) -> Result<bool, Error> {
    use sea_orm::{PaginatorTrait, QueryFilter};
    let Some(condition) = scope.condition() else {
        return Ok(true);
    };
    Ok(E::find_by_id(id).filter(condition).count(db).await? > 0)
}

/// Valeurs disponibles pour traduire une condition.
struct Context<'a> {
    table: &'a str,
    user: &'a CurrentUser,
    parameters: BTreeMap<String, (forge_schema::spec::ColumnType, JsonValue)>,
}

/// Opérande traduite : expression SQL, ou `NULL` (traité à part pour les comparaisons).
enum Operand {
    Sql(SimpleExpr),
    Null,
}

impl Context<'_> {
    /// Traduit une condition en clause SQL booléenne (`NULL` y vaut faux).
    fn boolean(&self, expr: &Expr) -> SimpleExpr {
        match self.operand(expr) {
            Operand::Sql(sql) => sql,
            Operand::Null => false.into(),
        }
    }

    fn operand(&self, expr: &Expr) -> Operand {
        let sql = |value: Value| Operand::Sql(sea_orm::sea_query::Expr::val(value));
        match &expr.kind {
            ExprKind::Null => Operand::Null,
            ExprKind::Bool(b) => sql((*b).into()),
            ExprKind::String(s) => sql(s.clone().into()),
            ExprKind::Number(n) => match n.fract().is_zero().then(|| n.to_i64()).flatten() {
                Some(integer) => sql(integer.into()),
                None => sql((*n).into()),
            },
            ExprKind::Path(path) => Operand::Sql(sea_orm::sea_query::Expr::col((
                Alias::new(self.table),
                Alias::new(&path[0]),
            ))),
            ExprKind::Var {
                scope: VarScope::User,
                path,
            } => match path[0].as_str() {
                "id" => sql(self.user.id.into()),
                _ => sql(self.user.email.clone().into()),
            },
            ExprKind::Var {
                scope: VarScope::Param,
                path,
            } => {
                match self.parameters.get(&path[0]).and_then(|(ty, json)| {
                    value::from_json(*ty, value::Domain::default(), json)
                        .ok()
                        .map(|typed| (*ty, typed))
                }) {
                    None | Some((_, TypedValue::Null)) => Operand::Null,
                    Some((ty, typed)) => sql(values::to_db(ty, typed)),
                }
            }
            ExprKind::Unary { op, expr } => match (op, self.operand(expr)) {
                (_, Operand::Null) => Operand::Null,
                (UnaryOp::Not, Operand::Sql(inner)) => Operand::Sql(inner.not()),
                (UnaryOp::Neg, Operand::Sql(inner)) => {
                    Operand::Sql(sea_orm::sea_query::Expr::val(0).sub(inner))
                }
            },
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right),
            ExprKind::Call { .. } => {
                unreachable!("fonctions refusées dans les conditions par le schéma")
            }
        }
    }

    fn binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Operand {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let (left, right) = (self.boolean(left), self.boolean(right));
            return Operand::Sql(if op == BinaryOp::And {
                left.and(right)
            } else {
                left.or(right)
            });
        }
        match (self.operand(left), self.operand(right)) {
            (Operand::Sql(l), Operand::Sql(r)) => Operand::Sql(match op {
                BinaryOp::Add => l.add(r),
                BinaryOp::Sub => l.sub(r),
                BinaryOp::Mul => l.mul(r),
                BinaryOp::Div => l.div(r),
                BinaryOp::Eq => l.eq(r),
                BinaryOp::Ne => l.ne(r),
                BinaryOp::Lt => l.lt(r),
                BinaryOp::Le => l.lte(r),
                BinaryOp::Gt => l.gt(r),
                BinaryOp::Ge => l.gte(r),
                BinaryOp::And | BinaryOp::Or => unreachable!("traité ci-dessus"),
            }),
            (Operand::Null, Operand::Null) => match op {
                BinaryOp::Eq => Operand::Sql(true.into()),
                BinaryOp::Ne => Operand::Sql(false.into()),
                _ => Operand::Null,
            },
            (Operand::Sql(side), Operand::Null) | (Operand::Null, Operand::Sql(side)) => match op {
                BinaryOp::Eq => Operand::Sql(side.is_null()),
                BinaryOp::Ne => Operand::Sql(side.is_not_null()),
                _ => Operand::Null,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_schema::spec::ColumnType;
    use sea_orm::sea_query::{Query, SqliteQueryBuilder};
    use serde_json::json;

    fn sql(condition: &str, parameters: &[(&str, ColumnType, JsonValue)]) -> String {
        let user = CurrentUser {
            id: 7,
            email: "a@b.c".into(),
            roles: vec!["commercial".into()],
        };
        let context = Context {
            table: "contact",
            user: &user,
            parameters: parameters
                .iter()
                .map(|(name, ty, value)| ((*name).to_owned(), (*ty, value.clone())))
                .collect(),
        };
        let expr = forge_formula::parse(condition).unwrap();
        let select = Query::select().expr(context.boolean(&expr)).to_owned();
        select
            .to_string(SqliteQueryBuilder)
            .trim_start_matches("SELECT ")
            .to_owned()
    }

    #[test]
    fn owner_condition() {
        assert_eq!(sql("owner == $user.id", &[]), r#""contact"."owner" = 7"#);
    }

    #[test]
    fn literals_operators_and_parameters() {
        assert_eq!(
            sql(
                r#"montant * 2 >= $param.seuil AND NOT archive OR nom != "x""#,
                &[("seuil", ColumnType::Decimal, json!(10.5))]
            ),
            r#"("contact"."montant" * 2 >= 10.5 AND (NOT "contact"."archive")) OR "contact"."nom" <> 'x'"#
        );
        assert_eq!(
            sql("email = $user.email", &[]),
            r#""contact"."email" = 'a@b.c'"#
        );
    }

    #[test]
    fn null_comparisons() {
        assert_eq!(sql("owner == NULL", &[]), r#""contact"."owner" IS NULL"#);
        assert_eq!(
            sql("NULL != owner", &[]),
            r#""contact"."owner" IS NOT NULL"#
        );
        assert_eq!(sql("owner > NULL", &[]), "FALSE");
        // Paramètre sans valeur : traité comme NULL.
        assert_eq!(
            sql(
                "montant < $param.seuil",
                &[("seuil", ColumnType::Decimal, JsonValue::Null)]
            ),
            "FALSE"
        );
    }
}
