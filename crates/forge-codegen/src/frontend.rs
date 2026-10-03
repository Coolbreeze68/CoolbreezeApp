//! Lecture du modèle commune aux interfaces générées (Flutter, web).

use forge_schema::spec::{Action, Column, ColumnType, Rule};
use forge_schema::{ColumnRef, Model};

/// Opérations d'API, dans l'ordre d'affichage.
pub(crate) const OPERATIONS: [&str; 4] = ["read", "create", "update", "delete"];

/// Colonne dont la valeur est affichée : pour un lookup, la colonne lue au
/// bout du chemin (avec sa table).
pub(crate) fn shown<'a>(
    model: &'a Model,
    table: &'a str,
    column: &'a Column,
) -> (&'a str, &'a Column) {
    let (mut table, mut column) = (table, column);
    while column.ty == ColumnType::Lookup {
        let Some((dep, target)) = model
            .dependencies(&ColumnRef::new(table, &column.name))
            .and_then(|deps| deps.first())
            .and_then(|dep| Some((dep, model.column(dep)?)))
        else {
            break;
        };
        table = &dep.table;
        column = target;
    }
    (table, column)
}

/// Opérations permises par une règle (`*` développé), dans l'ordre de [`OPERATIONS`].
pub(crate) fn operations(rule: &Rule) -> Vec<&'static str> {
    OPERATIONS
        .into_iter()
        .filter(|op| {
            rule.actions.iter().any(|action| match action {
                Action::All => true,
                Action::Read => *op == "read",
                Action::Create => *op == "create",
                Action::Update => *op == "update",
                Action::Delete => *op == "delete",
            })
        })
        .collect()
}
