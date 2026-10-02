//! Fichiers du backend Rust : entités, branchement, migrations, tests, code utilisateur.

use std::path::PathBuf;

use serde::Serialize;

use crate::error::Error;
use crate::layout::{ColumnLayout, Layout, Storage, TableLayout};
use crate::render::Renderer;
use crate::writer::{OutputFile, Policy};

/// Dossiers entièrement gérés par forge.
pub(crate) const GENERATED_DIRS: &[&str] = &["backend/src/generated"];

/// Fichiers réécrits hors de ces dossiers.
pub(crate) const GENERATED_TEST: &str = "backend/tests/generated_crud.rs";

/// Données communes aux templates du backend.
pub(crate) struct Backend<'a> {
    pub crate_name: &'a str,
    /// Contenu de `forge.json`, embarqué dans le binaire.
    pub schema_source: &'a str,
    pub layout: &'a Layout,
    /// Chemin de `forge-runtime`, relatif au dossier `backend/`.
    pub runtime_path: &'a str,
    /// Noms des migrations (`m0001_init`…), dans l'ordre.
    pub migrations: &'a [String],
}

#[derive(Serialize)]
struct TableView {
    name: String,
    type_name: String,
    fields: Vec<FieldView>,
}

#[derive(Serialize)]
struct FieldView {
    name: String,
    rust_type: String,
    text: bool,
}

impl TableView {
    fn of(table: &TableLayout) -> Self {
        Self {
            name: table.name.clone(),
            type_name: pascal_case(&table.name),
            fields: table.columns.iter().map(FieldView::of).collect(),
        }
    }
}

impl FieldView {
    fn of(column: &ColumnLayout) -> Self {
        let base = match column.storage {
            Storage::String | Storage::Text => "String",
            Storage::BigInteger => "i64",
            Storage::Decimal => "Decimal",
            Storage::Boolean => "bool",
            Storage::Date => "Date",
            Storage::Timestamp => "DateTimeUtc",
        };
        Self {
            name: column.name.clone(),
            rust_type: if column.nullable {
                format!("Option<{base}>")
            } else {
                base.to_owned()
            },
            text: column.storage == Storage::Text,
        }
    }
}

impl Backend<'_> {
    pub(crate) fn files(&self, renderer: &Renderer) -> Result<Vec<OutputFile>, Error> {
        let tables: Vec<TableView> = self.layout.tables.iter().map(TableView::of).collect();
        let base = serde_json::json!({
            "crate_name": self.crate_name,
            "runtime_path": self.runtime_path,
            "tables": tables,
            "migrations": self.migrations,
        });
        let generated = |path: &str, template: &str| -> Result<OutputFile, Error> {
            Ok(OutputFile {
                path: PathBuf::from(path),
                content: renderer.render(template, &base)?,
                policy: Policy::Generated,
            })
        };
        let once = |path: &str, template: &str| -> Result<OutputFile, Error> {
            Ok(OutputFile {
                path: PathBuf::from(path),
                content: renderer.render(template, &base)?,
                policy: Policy::Once,
            })
        };

        let mut files = vec![
            once("backend/Cargo.toml", "backend/cargo.toml")?,
            once("backend/src/main.rs", "backend/main.rs")?,
            once("backend/src/lib.rs", "backend/lib.rs")?,
            once("backend/src/custom/mod.rs", "backend/custom_mod.rs")?,
            once("backend/src/custom/routes.rs", "backend/custom_routes.rs")?,
            once(".gitignore", "backend/gitignore")?,
            generated("backend/src/generated/mod.rs", "backend/generated_mod.rs")?,
            generated(
                "backend/src/generated/entities/mod.rs",
                "backend/entities_mod.rs",
            )?,
            generated("backend/src/generated/hooks.rs", "backend/hooks_mod.rs")?,
            generated(
                "backend/src/generated/migrations.rs",
                "backend/migrations_mod.rs",
            )?,
            generated(GENERATED_TEST, "backend/crud_test.rs")?,
            OutputFile {
                path: PathBuf::from("backend/src/generated/forge.json"),
                content: self.schema_source.to_owned(),
                policy: Policy::Generated,
            },
        ];
        for table in &tables {
            let context = serde_json::json!({ "table": table });
            files.push(OutputFile {
                path: PathBuf::from(format!("backend/src/generated/entities/{}.rs", table.name)),
                content: renderer.render("backend/entity.rs", &context)?,
                policy: Policy::Generated,
            });
            files.push(OutputFile {
                path: PathBuf::from(format!("backend/src/custom/hooks/{}.rs", table.name)),
                content: renderer.render("backend/custom_hooks.rs", &context)?,
                policy: Policy::Once,
            });
        }
        Ok(files)
    }
}

/// `date_cloture` → `DateCloture`.
pub(crate) fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + chars.as_str()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(pascal_case("date_cloture"), "DateCloture");
        assert_eq!(pascal_case("tag"), "Tag");
        assert_eq!(pascal_case("x2_y"), "X2Y");
    }
}
