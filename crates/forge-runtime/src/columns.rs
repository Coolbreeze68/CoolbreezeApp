//! Lecture des métadonnées de colonnes du schéma, du point de vue de l'API.

use forge_schema::spec::{Column, ColumnType, Table};
use forge_schema::value::Domain;

/// Colonne stockée et interrogeable (filtres, tri) : métier ou système.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Queryable<'a> {
    pub ty: ColumnType,
    pub domain: Domain<'a>,
}

/// Type d'une colonne système (`id`, `owner`, `created_at`, `updated_at`).
pub(crate) fn system_type(name: &str) -> Option<ColumnType> {
    match name {
        "id" | "owner" => Some(ColumnType::Reference),
        "created_at" | "updated_at" => Some(ColumnType::Datetime),
        _ => None,
    }
}

pub(crate) fn find<'a>(table: &'a Table, name: &str) -> Option<&'a Column> {
    table.columns.iter().find(|c| c.name == name)
}

/// Colonne présente en base, donc utilisable pour filtrer et trier.
pub(crate) fn queryable<'a>(table: &'a Table, name: &str) -> Option<Queryable<'a>> {
    if let Some(ty) = system_type(name) {
        return Some(Queryable {
            ty,
            domain: Domain::default(),
        });
    }
    find(table, name)
        .filter(|c| c.is_stored())
        .map(|c| Queryable {
            ty: c.ty,
            domain: Domain::of(c),
        })
}

/// Colonne modifiable par l'API (ni système, ni calculée).
pub(crate) fn is_writable(column: &Column) -> bool {
    !column.is_computed()
}
