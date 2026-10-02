//! Structure de stockage : ce que les migrations doivent créer en base.
//!
//! Seul ce qui touche au stockage y figure (pas les libellés, vues ou règles) :
//! deux schémas de même structure ne nécessitent aucune migration.
//! Les colonnes système (`id`, `owner`, `created_at`, `updated_at`) sont ajoutées
//! par `forge_runtime::migration::Plan` et ne sont pas répétées ici.

use forge_schema::Model;
use forge_schema::spec::ColumnType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub tables: Vec<TableLayout>,
    pub join_tables: Vec<JoinTable>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableLayout {
    pub name: String,
    pub columns: Vec<ColumnLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnLayout {
    pub name: String,
    pub storage: Storage,
    pub nullable: bool,
    pub unique: bool,
    /// Table cible d'une colonne `reference`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinTable {
    pub name: String,
    pub source: String,
    pub target: String,
}

/// Type de stockage SQL d'une colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Storage {
    /// `varchar(255)`.
    String,
    Text,
    BigInteger,
    /// `decimal(19, 4)`.
    Decimal,
    Boolean,
    Date,
    /// Date-heure avec fuseau.
    Timestamp,
}

impl Storage {
    pub fn name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Text => "text",
            Self::BigInteger => "big_integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Timestamp => "timestamp",
        }
    }

    fn of(ty: ColumnType) -> Option<Self> {
        Some(match ty {
            ColumnType::String | ColumnType::Enum => Self::String,
            ColumnType::Text => Self::Text,
            ColumnType::Integer | ColumnType::Duration | ColumnType::Reference => Self::BigInteger,
            ColumnType::Decimal => Self::Decimal,
            ColumnType::Boolean => Self::Boolean,
            ColumnType::Date => Self::Date,
            ColumnType::Datetime => Self::Timestamp,
            ColumnType::ReferenceList | ColumnType::Lookup => return None,
        })
    }
}

impl Layout {
    /// Base vide, point de départ de la première migration.
    pub fn empty() -> Self {
        Self {
            tables: Vec::new(),
            join_tables: Vec::new(),
        }
    }

    pub fn table(&self, name: &str) -> Option<&TableLayout> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn of(model: &Model) -> Self {
        let tables = model
            .tables()
            .iter()
            .map(|table| TableLayout {
                name: table.name.clone(),
                columns: table
                    .columns
                    .iter()
                    .filter(|c| c.is_stored())
                    .filter_map(|c| {
                        Some(ColumnLayout {
                            name: c.name.clone(),
                            storage: Storage::of(c.ty)?,
                            nullable: !c.required || c.is_computed(),
                            unique: c.unique,
                            references: c.target.clone().filter(|_| c.ty == ColumnType::Reference),
                        })
                    })
                    .collect(),
            })
            .collect();
        let join_tables = model
            .relations()
            .iter()
            .filter_map(|r| {
                Some(JoinTable {
                    name: r.join_table()?,
                    source: r.source.table.clone(),
                    target: r.target.clone(),
                })
            })
            .collect();
        Self {
            tables,
            join_tables,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crm_layout() {
        let model = Model::from_json(include_str!("../../../examples/crm/forge.json")).unwrap();
        let layout = Layout::of(&model);
        let opportunite = layout
            .tables
            .iter()
            .find(|t| t.name == "opportunite")
            .unwrap();
        let names: Vec<_> = opportunite
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        // Ni lookup (`secteur`), ni formule non persistée, ni reference_list.
        assert_eq!(
            names,
            [
                "titre",
                "entreprise",
                "contact",
                "montant",
                "probabilite",
                "montant_pondere",
                "montant_ttc",
                "etape",
                "date_cloture",
                "notes_internes"
            ]
        );
        let entreprise = &opportunite.columns[1];
        assert_eq!(entreprise.references.as_deref(), Some("entreprise"));
        assert!(!entreprise.nullable);
        assert!(
            opportunite.columns[5].nullable,
            "formule persistée : nullable"
        );
        assert_eq!(
            layout.join_tables,
            [JoinTable {
                name: "opportunite_tags".into(),
                source: "opportunite".into(),
                target: "tag".into()
            }]
        );
    }
}
