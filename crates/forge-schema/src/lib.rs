//! Format d'entrée de forge.
//!
//! - [`spec`] : types du fichier `forge.json` (et source de `forge.schema.json`) ;
//! - [`Model`] : schéma validé, références résolues et formules analysées.
//!
//! ```
//! let src = r#"{
//!   "app": { "name": "demo", "default_locale": "fr", "locales": ["fr"] },
//!   "roles": ["admin"],
//!   "tables": [{ "name": "note", "columns": [{ "name": "titre", "type": "string" }] }]
//! }"#;
//! let model = forge_schema::Model::from_json(src).unwrap();
//! assert_eq!(model.tables()[0].name, "note");
//! ```

mod error;
mod graph;
mod model;
pub mod names;
pub mod spec;
mod validate;
pub mod value;

pub use error::{Issue, SchemaError};
pub use model::{ColumnRef, Model, Relation, RelationKind};

/// JSON Schema du format d'entrée, publié dans `forge.schema.json`.
pub fn json_schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(spec::Spec);
    schema.insert("title".into(), "forge".into());
    schema.to_value()
}
