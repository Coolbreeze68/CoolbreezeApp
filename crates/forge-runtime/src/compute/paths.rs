//! Chemins des formules, découpés en étapes de relation.

use std::collections::BTreeSet;

use forge_formula::{Expr, ExprKind, VarScope};
use forge_schema::spec::ColumnType;
use forge_schema::{ColumnRef, Model, RelationKind};

/// Étape d'un chemin, d'une table vers une autre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hop {
    /// `from.column` référence `to` (N→1).
    Reference {
        from: String,
        column: String,
        to: String,
    },
    /// `from.column` est une `reference_list` vers `to`, via `join`.
    List {
        from: String,
        column: String,
        to: String,
        join: String,
    },
    /// Enregistrements de `to` dont `column` référence `from` (relation inverse).
    Inverse {
        from: String,
        to: String,
        column: String,
    },
    /// Sources `to` d'une `reference_list` qui lient `from`, via `join`.
    InverseList {
        from: String,
        to: String,
        join: String,
    },
}

impl Hop {
    pub(crate) fn to(&self) -> &str {
        match self {
            Self::Reference { to, .. }
            | Self::List { to, .. }
            | Self::Inverse { to, .. }
            | Self::InverseList { to, .. } => to,
        }
    }

    /// Clé des liens « plusieurs » mémorisés pour cette étape.
    pub(crate) fn key(&self) -> (String, String) {
        match self {
            Self::Reference { from, column, .. } | Self::List { from, column, .. } => {
                (from.clone(), column.clone())
            }
            Self::Inverse { from, to, column } => (from.clone(), format!("{to}.{column}")),
            Self::InverseList { from, to, join } => (from.clone(), format!("{to}.{join}")),
        }
    }
}

/// Chemin résolu : étapes, puis colonne finale (`None` si le chemin désigne
/// une relation, comme dans `COUNT(tags)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Path {
    pub hops: Vec<Hop>,
    pub column: Option<String>,
}

impl Path {
    /// Table de la colonne finale.
    pub(crate) fn end_table<'a>(&'a self, start: &'a str) -> &'a str {
        self.hops.last().map_or(start, Hop::to)
    }
}

/// Résout un chemin depuis `table` ; le schéma l'a déjà validé.
pub(crate) fn resolve(model: &Model, table: &str, segments: &[String]) -> Path {
    let mut current = table.to_owned();
    let mut hops = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        let column = model
            .table(&current)
            .and_then(|t| t.columns.iter().find(|c| c.name == *segment));
        if let Some(column) = column {
            let target = column.target.clone().unwrap_or_default();
            match column.ty {
                ColumnType::Reference if !last => {
                    hops.push(Hop::Reference {
                        from: current.clone(),
                        column: segment.clone(),
                        to: target.clone(),
                    });
                    current = target;
                    continue;
                }
                ColumnType::ReferenceList => {
                    let join = format!("{current}_{segment}");
                    hops.push(Hop::List {
                        from: current.clone(),
                        column: segment.clone(),
                        to: target.clone(),
                        join,
                    });
                    current = target;
                    if last {
                        return Path { hops, column: None };
                    }
                    continue;
                }
                _ => {}
            }
        }
        let relation = model
            .relations()
            .iter()
            .find(|r| r.target == current && r.inverse == *segment);
        match relation {
            Some(relation) if column.is_none() => {
                let source = relation.source.table.clone();
                hops.push(match relation.kind {
                    RelationKind::ManyToOne => Hop::Inverse {
                        from: current.clone(),
                        to: source.clone(),
                        column: relation.source.column.clone(),
                    },
                    RelationKind::ManyToMany => Hop::InverseList {
                        from: current.clone(),
                        to: source.clone(),
                        join: relation.join_table().expect("jointure N↔N"),
                    },
                });
                current = source;
                if last {
                    return Path { hops, column: None };
                }
            }
            _ => {
                return Path {
                    hops,
                    column: Some(segment.clone()),
                };
            }
        }
    }
    Path { hops, column: None }
}

/// Chemins lus par une expression, arguments d'agrégats compris.
pub(crate) fn paths(expr: &Expr) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    walk(expr, &mut |e| {
        if let ExprKind::Path(path) = &e.kind {
            found.push(path.clone());
        }
    });
    found
}

/// Paramètres (`$param.x`) lus par une colonne calculée, directement ou via les
/// colonnes calculées dont elle dépend.
pub(crate) fn parameters(model: &Model, column: &ColumnRef) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![column.clone()];
    let mut seen = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(expr) = model.formula(&current) {
            walk(expr, &mut |e| {
                if let ExprKind::Var {
                    scope: VarScope::Param,
                    path,
                } = &e.kind
                {
                    found.insert(path[0].clone());
                }
            });
        }
        pending.extend(model.dependencies(&current).into_iter().flatten().cloned());
    }
    found
}

fn walk(expr: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expr);
    match &expr.kind {
        ExprKind::Unary { expr, .. } => walk(expr, visit),
        ExprKind::Binary { left, right, .. } => {
            walk(left, visit);
            walk(right, visit);
        }
        ExprKind::Call { args, .. } => args.iter().for_each(|a| walk(a, visit)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crm() -> Model {
        Model::from_json(include_str!("../../../../examples/crm/forge.json")).unwrap()
    }

    fn path(model: &Model, table: &str, src: &str) -> Path {
        resolve(
            model,
            table,
            &src.split('.').map(str::to_owned).collect::<Vec<_>>(),
        )
    }

    #[test]
    fn resolution() {
        let model = crm();
        assert_eq!(
            path(&model, "opportunite", "montant"),
            Path {
                hops: vec![],
                column: Some("montant".into())
            }
        );
        assert_eq!(
            path(&model, "opportunite", "entreprise"),
            Path {
                hops: vec![],
                column: Some("entreprise".into())
            }
        );
        let secteur = path(&model, "opportunite", "entreprise.secteur");
        assert_eq!(secteur.hops.len(), 1);
        assert_eq!(secteur.column.as_deref(), Some("secteur"));
        assert_eq!(secteur.end_table("opportunite"), "entreprise");

        let pipeline = path(&model, "entreprise", "opportunites.montant");
        assert_eq!(
            pipeline.hops,
            [Hop::Inverse {
                from: "entreprise".into(),
                to: "opportunite".into(),
                column: "entreprise".into()
            }]
        );
        let count = path(&model, "tag", "opportunites");
        assert_eq!(count.column, None);
        assert!(matches!(count.hops[0], Hop::InverseList { .. }));
        let tags = path(&model, "opportunite", "tags");
        assert!(matches!(&tags.hops[0], Hop::List { join, .. } if join == "opportunite_tags"));
    }

    #[test]
    fn parameters_are_found_transitively() {
        let model = crm();
        let ttc = ColumnRef::new("opportunite", "montant_ttc");
        assert_eq!(parameters(&model, &ttc), BTreeSet::from(["tva".to_owned()]));
        assert!(parameters(&model, &ColumnRef::new("entreprise", "pipeline")).is_empty());
    }
}
