//! Lecture du modèle commune aux interfaces générées (Flutter, web).

use forge_schema::Model;
use forge_schema::spec::{Action, Column, ColumnType, Rule};

/// Opérations d'API, dans l'ordre d'affichage.
pub(crate) const OPERATIONS: [&str; 4] = ["read", "create", "update", "delete"];

/// Options de saisie et d'affichage d'un modèle de champ, résolues (valeurs
/// par défaut comprises) : `column` est la colonne affichée (lookup résolu).
#[derive(Debug, Default)]
pub(crate) struct FieldOptions<'a> {
    /// `rating` : note maximale.
    pub max: Option<u32>,
    /// `money` : devise.
    pub currency: Option<&'a str>,
    /// `file` / `image` : taille maximale en Mo.
    pub max_size: Option<u32>,
    /// `file` : types acceptés.
    pub accept: &'a [String],
}

pub(crate) fn field_options<'a>(model: &'a Model, column: &'a Column) -> FieldOptions<'a> {
    FieldOptions {
        max: (column.ty == ColumnType::Rating).then(|| column.rating_max()),
        currency: (column.ty == ColumnType::Money).then(|| {
            column
                .currency
                .as_deref()
                .unwrap_or_else(|| model.spec().app.currency())
        }),
        max_size: column
            .ty
            .is_file()
            .then(|| column.max_size.unwrap_or(Column::DEFAULT_MAX_SIZE_MB)),
        accept: column.accept.as_deref().unwrap_or_default(),
    }
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
