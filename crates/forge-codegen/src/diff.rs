//! Différences entre deux structures de stockage, et risque de chaque changement.

use std::collections::BTreeMap;
use std::fmt;

use forge_schema::Model;
use serde_json::Value;

use crate::layout::{ColumnLayout, JoinTable, Layout, TableLayout};

/// Informations du schéma utiles au diff, absentes de la structure de stockage.
#[derive(Debug, Clone, Default)]
pub struct Hints {
    /// `(table, nouveau nom)` → ancien nom (`renamed_from`).
    renames: BTreeMap<(String, String), String>,
    /// `(table, colonne)` → valeur par défaut du schéma.
    defaults: BTreeMap<(String, String), Value>,
}

impl Hints {
    pub fn of(model: &Model) -> Self {
        let mut hints = Self::default();
        for table in model.tables() {
            for column in &table.columns {
                let key = (table.name.clone(), column.name.clone());
                if let Some(old) = &column.renamed_from {
                    hints.renames.insert(key.clone(), old.clone());
                }
                if let Some(default) = &column.default {
                    hints.defaults.insert(key, default.clone());
                }
            }
        }
        hints
    }

    /// Renommages inversés, pour la migration de retour ; sans valeurs par défaut.
    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            renames: self
                .renames
                .iter()
                .map(|((table, new), old)| ((table.clone(), old.clone()), new.clone()))
                .collect(),
            defaults: BTreeMap::new(),
        }
    }

    pub fn default_of(&self, table: &str, column: &str) -> Option<&Value> {
        self.defaults.get(&(table.to_owned(), column.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    CreateTable(TableLayout),
    DropTable(TableLayout),
    CreateJoinTable(JoinTable),
    DropJoinTable(JoinTable),
    AddColumn {
        table: String,
        column: ColumnLayout,
    },
    DropColumn {
        table: String,
        column: ColumnLayout,
    },
    RenameColumn {
        table: String,
        from: String,
        to: String,
    },
    AlterColumn {
        table: String,
        before: ColumnLayout,
        after: ColumnLayout,
    },
    AddUnique {
        table: String,
        column: String,
        existing: bool,
    },
    DropUnique {
        table: String,
        column: String,
    },
    AddReference {
        table: String,
        column: ColumnLayout,
        existing: bool,
    },
    DropReference {
        table: String,
        column: String,
    },
}

/// Conséquence possible d'un changement sur des données existantes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Risk {
    Safe,
    /// La migration échouera si les données ne s'y prêtent pas, sans rien perdre.
    MayFail(String),
    /// Des données seront perdues : exige `--allow-destructive`.
    DataLoss(String),
}

impl Change {
    /// Table existante modifiée par ce changement (ni créée, ni supprimée).
    pub(crate) fn altered_table(&self) -> Option<&str> {
        match self {
            Self::AddColumn { table, .. }
            | Self::DropColumn { table, .. }
            | Self::RenameColumn { table, .. }
            | Self::AlterColumn { table, .. }
            | Self::AddUnique { table, .. }
            | Self::DropUnique { table, .. }
            | Self::AddReference { table, .. }
            | Self::DropReference { table, .. } => Some(table),
            _ => None,
        }
    }

    /// Table concernée, quelle que soit la nature du changement.
    pub(crate) fn table(&self) -> &str {
        match self {
            Self::CreateTable(t) | Self::DropTable(t) => &t.name,
            Self::CreateJoinTable(j) | Self::DropJoinTable(j) => &j.source,
            _ => self.altered_table().expect("changement de colonne"),
        }
    }

    pub fn risk(&self) -> Risk {
        match self {
            Self::DropTable(t) => {
                Risk::DataLoss(format!("table `{}` et ses données supprimées", t.name))
            }
            Self::DropJoinTable(j) => Risk::DataLoss(format!("liens de `{}` supprimés", j.name)),
            Self::DropColumn { table, column } => Risk::DataLoss(format!(
                "colonne `{table}.{}` et ses valeurs supprimées",
                column.name
            )),
            Self::AlterColumn {
                table,
                before,
                after,
            } if before.storage != after.storage => Risk::DataLoss(format!(
                "`{table}.{}` convertie de {} en {} : valeurs incompatibles perdues ou migration en échec",
                after.name,
                before.storage.name(),
                after.storage.name()
            )),
            Self::AlterColumn {
                table,
                before,
                after,
            } if before.nullable && !after.nullable => Risk::MayFail(format!(
                "`{table}.{}` devient obligatoire : échec si des valeurs sont vides",
                after.name
            )),
            Self::AddColumn { table, column } if !column.nullable => Risk::MayFail(format!(
                "`{table}.{}` est obligatoire : sans valeur par défaut, échec si la table contient des lignes",
                column.name
            )),
            Self::AddUnique {
                table,
                column,
                existing: true,
            } => Risk::MayFail(format!(
                "`{table}.{column}` devient unique : échec en cas de doublons"
            )),
            Self::AddReference {
                table,
                column,
                existing: true,
            } => Risk::MayFail(format!(
                "`{table}.{}` référence désormais `{}` : échec si des valeurs n'y correspondent pas",
                column.name,
                column.references.as_deref().unwrap_or_default()
            )),
            _ => Risk::Safe,
        }
    }
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateTable(t) => write!(f, "nouvelle table `{}`", t.name),
            Self::DropTable(t) => write!(f, "suppression de la table `{}`", t.name),
            Self::CreateJoinTable(j) => write!(f, "nouvelle table de jointure `{}`", j.name),
            Self::DropJoinTable(j) => write!(f, "suppression de la table de jointure `{}`", j.name),
            Self::AddColumn { table, column } => {
                write!(f, "`{table}` : nouvelle colonne `{}`", column.name)
            }
            Self::DropColumn { table, column } => {
                write!(f, "`{table}` : suppression de `{}`", column.name)
            }
            Self::RenameColumn { table, from, to } => {
                write!(f, "`{table}` : `{from}` renommée en `{to}`")
            }
            Self::AlterColumn { table, after, .. } => {
                write!(f, "`{table}` : `{}` modifiée", after.name)
            }
            Self::AddUnique { table, column, .. } => write!(f, "`{table}` : `{column}` unique"),
            Self::DropUnique { table, column } => {
                write!(f, "`{table}` : `{column}` n'est plus unique")
            }
            Self::AddReference { table, column, .. } => write!(
                f,
                "`{table}` : `{}` référence `{}`",
                column.name,
                column.references.as_deref().unwrap_or_default()
            ),
            Self::DropReference { table, column } => {
                write!(f, "`{table}` : `{column}` ne référence plus")
            }
        }
    }
}

/// Changements pour passer de `before` à `after`.
pub fn diff(before: &Layout, after: &Layout, hints: &Hints) -> Vec<Change> {
    let mut changes = Vec::new();
    let find = |layout: &Layout, name: &str| layout.tables.iter().find(|t| t.name == name).cloned();

    for table in &after.tables {
        match find(before, &table.name) {
            None => changes.push(Change::CreateTable(table.clone())),
            Some(previous) => diff_table(&previous, table, hints, &mut changes),
        }
    }
    for table in &before.tables {
        if find(after, &table.name).is_none() {
            changes.push(Change::DropTable(table.clone()));
        }
    }

    for join in &before.join_tables {
        if !after.join_tables.contains(join) {
            changes.push(Change::DropJoinTable(join.clone()));
        }
    }
    for join in &after.join_tables {
        if !before.join_tables.contains(join) {
            changes.push(Change::CreateJoinTable(join.clone()));
        }
    }
    changes
}

fn diff_table(before: &TableLayout, after: &TableLayout, hints: &Hints, changes: &mut Vec<Change>) {
    let table = &after.name;
    let previous = |name: &str| before.columns.iter().find(|c| c.name == name);
    let mut kept = Vec::new();

    for column in &after.columns {
        // Renommage : seulement si l'ancienne colonne existe et la nouvelle pas encore.
        let renamed_from = hints
            .renames
            .get(&(table.clone(), column.name.clone()))
            .filter(|old| previous(old).is_some() && previous(&column.name).is_none());
        let source = renamed_from.map_or(column.name.as_str(), String::as_str);

        let Some(old) = previous(source) else {
            changes.push(Change::AddColumn {
                table: table.clone(),
                column: column.clone(),
            });
            push_constraints(changes, table, column);
            continue;
        };
        kept.push(source.to_owned());

        let renamed = renamed_from.is_some();
        let reference_changed = renamed
            || (old.references.clone(), old.nullable)
                != (column.references.clone(), column.nullable);
        let unique_changed = renamed || old.unique != column.unique;
        if reference_changed && old.references.is_some() {
            changes.push(Change::DropReference {
                table: table.clone(),
                column: old.name.clone(),
            });
        }
        if unique_changed && old.unique {
            changes.push(Change::DropUnique {
                table: table.clone(),
                column: old.name.clone(),
            });
        }
        if renamed {
            changes.push(Change::RenameColumn {
                table: table.clone(),
                from: old.name.clone(),
                to: column.name.clone(),
            });
        }
        if (old.storage, old.nullable) != (column.storage, column.nullable) {
            changes.push(Change::AlterColumn {
                table: table.clone(),
                before: old.clone(),
                after: column.clone(),
            });
        }
        if unique_changed && column.unique {
            changes.push(Change::AddUnique {
                table: table.clone(),
                column: column.name.clone(),
                existing: !old.unique,
            });
        }
        if reference_changed && column.references.is_some() {
            changes.push(Change::AddReference {
                table: table.clone(),
                column: column.clone(),
                existing: old.references != column.references,
            });
        }
    }

    for column in before.columns.iter().filter(|c| !kept.contains(&c.name)) {
        if column.references.is_some() {
            changes.push(Change::DropReference {
                table: table.clone(),
                column: column.name.clone(),
            });
        }
        if column.unique {
            changes.push(Change::DropUnique {
                table: table.clone(),
                column: column.name.clone(),
            });
        }
        changes.push(Change::DropColumn {
            table: table.clone(),
            column: column.clone(),
        });
    }
}

/// Contraintes d'une colonne nouvelle (sans données existantes).
fn push_constraints(changes: &mut Vec<Change>, table: &str, column: &ColumnLayout) {
    if column.unique {
        changes.push(Change::AddUnique {
            table: table.to_owned(),
            column: column.name.clone(),
            existing: false,
        });
    }
    if column.references.is_some() {
        changes.push(Change::AddReference {
            table: table.to_owned(),
            column: column.clone(),
            existing: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Storage;

    fn column(name: &str, storage: Storage, nullable: bool) -> ColumnLayout {
        ColumnLayout {
            name: name.into(),
            storage,
            nullable,
            unique: false,
            references: None,
        }
    }

    fn layout(columns: Vec<ColumnLayout>) -> Layout {
        Layout {
            tables: vec![TableLayout {
                name: "client".into(),
                columns,
            }],
            join_tables: Vec::new(),
        }
    }

    fn hints(renames: &[(&str, &str)]) -> Hints {
        Hints {
            renames: renames
                .iter()
                .map(|(new, old)| (("client".into(), (*new).into()), (*old).into()))
                .collect(),
            defaults: BTreeMap::new(),
        }
    }

    fn describe(changes: &[Change]) -> Vec<String> {
        changes.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn identical_layouts_have_no_changes() {
        let l = layout(vec![column("nom", Storage::String, false)]);
        assert!(diff(&l, &l, &Hints::default()).is_empty());
    }

    #[test]
    fn tables_created_and_dropped() {
        let changes = diff(&Layout::empty(), &layout(vec![]), &Hints::default());
        assert_eq!(describe(&changes), ["nouvelle table `client`"]);
        let changes = diff(&layout(vec![]), &Layout::empty(), &Hints::default());
        assert!(matches!(changes[0].risk(), Risk::DataLoss(_)));
    }

    #[test]
    fn columns_added_dropped_and_altered() {
        let before = layout(vec![
            column("nom", Storage::String, true),
            column("fax", Storage::String, true),
            column("age", Storage::BigInteger, true),
        ]);
        let after = layout(vec![
            column("nom", Storage::String, false),
            column("age", Storage::Decimal, true),
            column("score", Storage::BigInteger, false),
        ]);
        let changes = diff(&before, &after, &Hints::default());
        assert_eq!(
            describe(&changes),
            [
                "`client` : `nom` modifiée",
                "`client` : `age` modifiée",
                "`client` : nouvelle colonne `score`",
                "`client` : suppression de `fax`",
            ]
        );
        assert!(
            matches!(changes[0].risk(), Risk::MayFail(_)),
            "nom devient obligatoire"
        );
        assert!(
            matches!(changes[1].risk(), Risk::DataLoss(_)),
            "changement de type"
        );
        assert!(
            matches!(changes[2].risk(), Risk::MayFail(_)),
            "obligatoire sans défaut"
        );
        assert!(matches!(changes[3].risk(), Risk::DataLoss(_)));
    }

    #[test]
    fn renames_keep_data_and_move_constraints() {
        let mut tel = column("tel", Storage::String, true);
        tel.unique = true;
        let before = layout(vec![tel]);
        let mut telephone = column("telephone", Storage::String, true);
        telephone.unique = true;
        let after = layout(vec![telephone]);

        let changes = diff(&before, &after, &hints(&[("telephone", "tel")]));
        assert_eq!(
            describe(&changes),
            [
                "`client` : `tel` n'est plus unique",
                "`client` : `tel` renommée en `telephone`",
                "`client` : `telephone` unique",
            ]
        );
        assert!(
            changes.iter().all(|c| c.risk() == Risk::Safe),
            "l'unicité existait déjà"
        );

        // Sans indication de renommage : suppression puis ajout (destructif).
        let changes = diff(&before, &after, &Hints::default());
        assert!(
            changes
                .iter()
                .any(|c| matches!(c.risk(), Risk::DataLoss(_)))
        );

        // Renommage déjà appliqué par une migration précédente : ignoré.
        assert!(diff(&after, &after, &hints(&[("telephone", "tel")])).is_empty());
    }

    #[test]
    fn references() {
        let mut entreprise = column("entreprise", Storage::BigInteger, true);
        entreprise.references = Some("entreprise".into());
        let before = layout(vec![column("nom", Storage::String, true)]);
        let after = layout(vec![
            column("nom", Storage::String, true),
            entreprise.clone(),
        ]);
        let changes = diff(&before, &after, &Hints::default());
        assert_eq!(
            describe(&changes),
            [
                "`client` : nouvelle colonne `entreprise`",
                "`client` : `entreprise` référence `entreprise`"
            ]
        );
        assert_eq!(
            changes[1].risk(),
            Risk::Safe,
            "nouvelle colonne : aucune donnée"
        );

        // Devenir obligatoire change l'action de suppression : contrainte recréée.
        let mut required = entreprise.clone();
        required.nullable = false;
        let changes = diff(
            &after,
            &layout(vec![column("nom", Storage::String, true), required]),
            &Hints::default(),
        );
        assert_eq!(
            describe(&changes),
            [
                "`client` : `entreprise` ne référence plus",
                "`client` : `entreprise` modifiée",
                "`client` : `entreprise` référence `entreprise`",
            ]
        );
    }

    #[test]
    fn join_tables() {
        let join = JoinTable {
            name: "client_tags".into(),
            source: "client".into(),
            target: "tag".into(),
        };
        let mut after = layout(vec![]);
        after.join_tables.push(join.clone());
        let changes = diff(&layout(vec![]), &after, &Hints::default());
        assert_eq!(changes, [Change::CreateJoinTable(join)]);
    }
}
