//! Migrations : tables système de forge et exécution des migrations générées.
//!
//! Une migration générée décrit ses changements avec un [`Plan`] : création et
//! suppression de tables, ajout, renommage, modification et suppression de colonnes,
//! contraintes d'unicité et clés étrangères. Le plan applique ces opérations dans un
//! ordre sûr, quel que soit l'ordre de déclaration.
//!
//! PostgreSQL et MySQL exécutent les opérations telles quelles. SQLite ne sait ni
//! modifier une colonne ni ajouter une clé étrangère à une table existante : chaque
//! table modifiée y est donc reconstruite à partir de sa définition complète
//! ([`Plan::redefine`]), données recopiées, clés étrangères désactivées pendant
//! l'opération puis vérifiées. Cela suppose une connexion unique à la base, ce que
//! garantissent la ligne de commande générée et les outils de test.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{
    big_integer, big_integer_null, big_pk_auto, boolean, string, string_null, text_null,
    timestamp_with_time_zone,
};

use crate::links;

/// Migrations système, à placer avant celles de l'application.
pub fn system() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(SystemTables), Box::new(AuthTables)]
}

/// Effet de la suppression d'une ligne référencée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDelete {
    /// Interdite tant que la référence existe (référence obligatoire).
    Restrict,
    /// La référence passe à `NULL` (référence facultative).
    SetNull,
    /// Les lignes qui référencent sont supprimées (tables de jointure).
    Cascade,
}

impl From<OnDelete> for ForeignKeyAction {
    fn from(action: OnDelete) -> Self {
        match action {
            OnDelete::Restrict => Self::Restrict,
            OnDelete::SetNull => Self::SetNull,
            OnDelete::Cascade => Self::Cascade,
        }
    }
}

#[derive(Debug, Clone)]
struct Reference {
    column: String,
    target: String,
    on_delete: OnDelete,
}

/// Définition complète d'une table métier. `id`, `owner`, `created_at` et
/// `updated_at` sont ajoutées automatiquement.
#[derive(Debug, Clone, Default)]
pub struct TableDef {
    columns: Vec<ColumnDef>,
    unique: Vec<String>,
    references: Vec<Reference>,
}

impl TableDef {
    pub fn col(&mut self, column: impl IntoColumnDef) -> &mut Self {
        self.columns.push(column.into_column_def());
        self
    }

    /// Valeurs uniques dans `column` (index unique `uq_<table>_<colonne>`).
    pub fn unique(&mut self, column: &str) -> &mut Self {
        self.unique.push(column.to_owned());
        self
    }

    /// Clé étrangère `column → target.id` (contrainte `fk_<table>_<colonne>`).
    pub fn reference(&mut self, column: &str, target: &str, on_delete: OnDelete) -> &mut Self {
        self.references.push(Reference {
            column: column.to_owned(),
            target: target.to_owned(),
            on_delete,
        });
        self
    }

    fn all_references(&self) -> impl Iterator<Item = Reference> + '_ {
        std::iter::once(Reference {
            column: "owner".into(),
            target: "users".into(),
            on_delete: OnDelete::SetNull,
        })
        .chain(self.references.iter().cloned())
    }

    /// Instruction de création ; les clés étrangères y sont incluses si `inline_keys`.
    fn create_statement(
        &self,
        table: &str,
        created_as: &str,
        inline_keys: bool,
    ) -> TableCreateStatement {
        let mut statement = Table::create();
        statement
            .table(Alias::new(created_as))
            .col(big_pk_auto("id"));
        for column in &self.columns {
            statement.col(column.clone());
        }
        statement
            .col(big_integer_null("owner"))
            .col(timestamp_with_time_zone("created_at"))
            .col(timestamp_with_time_zone("updated_at"));
        if inline_keys {
            for reference in self.all_references() {
                statement.foreign_key(&mut foreign_key(table, &reference));
            }
        }
        statement
    }

    /// Noms des colonnes, dans l'ordre de création.
    fn column_names(&self) -> Vec<String> {
        let mut names = vec!["id".to_owned()];
        names.extend(self.columns.iter().map(ColumnDef::get_column_name));
        names.extend(["owner", "created_at", "updated_at"].map(str::to_owned));
        names
    }
}

fn foreign_key(table: &str, reference: &Reference) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(format!("fk_{table}_{}", reference.column))
        .from(Alias::new(table), Alias::new(&reference.column))
        .to(Alias::new(&reference.target), Alias::new("id"))
        .on_delete(reference.on_delete.into())
        .to_owned()
}

fn unique_index(table: &str, column: &str) -> IndexCreateStatement {
    Index::create()
        .name(format!("uq_{table}_{column}"))
        .table(Alias::new(table))
        .col(Alias::new(column))
        .unique()
        .to_owned()
}

fn join_statement(
    name: &str,
    inline_keys: bool,
    source: &str,
    target: &str,
) -> TableCreateStatement {
    let mut statement = Table::create();
    statement
        .table(Alias::new(name))
        .col(
            ColumnDef::new(Alias::new(links::SOURCE))
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(Alias::new(links::TARGET))
                .big_integer()
                .not_null(),
        )
        .primary_key(
            Index::create()
                .col(Alias::new(links::SOURCE))
                .col(Alias::new(links::TARGET)),
        );
    if inline_keys {
        for reference in join_references(source, target) {
            statement.foreign_key(&mut foreign_key(name, &reference));
        }
    }
    statement
}

fn join_references(source: &str, target: &str) -> [Reference; 2] {
    [(links::SOURCE, source), (links::TARGET, target)].map(|(column, table)| Reference {
        column: column.to_owned(),
        target: table.to_owned(),
        on_delete: OnDelete::Cascade,
    })
}

#[derive(Debug, Clone)]
enum Op {
    CreateTable {
        name: String,
        def: TableDef,
    },
    CreateJoinTable {
        name: String,
        source: String,
        target: String,
    },
    DropTable {
        name: String,
    },
    AddColumn {
        table: String,
        column: ColumnDef,
    },
    DropColumn {
        table: String,
        column: String,
    },
    RenameColumn {
        table: String,
        from: String,
        to: String,
    },
    AlterColumn {
        table: String,
        column: ColumnDef,
    },
    AddUnique {
        table: String,
        column: String,
    },
    DropUnique {
        table: String,
        column: String,
    },
    AddReference {
        table: String,
        reference: Reference,
    },
    DropReference {
        table: String,
        column: String,
    },
    Redefine {
        name: String,
        def: TableDef,
    },
}

impl Op {
    /// Table modifiée en place (hors création et suppression).
    fn altered_table(&self) -> Option<&str> {
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
}

/// Ensemble de changements de structure, appliqués dans un ordre sûr.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    ops: Vec<Op>,
}

impl Plan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crée une table métier.
    #[must_use]
    pub fn table(mut self, name: &str, define: impl FnOnce(&mut TableDef)) -> Self {
        let mut def = TableDef::default();
        define(&mut def);
        self.ops.push(Op::CreateTable {
            name: name.to_owned(),
            def,
        });
        self
    }

    /// Crée la table de jointure d'une colonne `reference_list`.
    #[must_use]
    pub fn join_table(mut self, name: &str, source: &str, target: &str) -> Self {
        self.ops.push(Op::CreateJoinTable {
            name: name.to_owned(),
            source: source.to_owned(),
            target: target.to_owned(),
        });
        self
    }

    /// Supprime une table (métier ou de jointure) et ses données.
    #[must_use]
    pub fn drop_table(mut self, name: &str) -> Self {
        self.ops.push(Op::DropTable {
            name: name.to_owned(),
        });
        self
    }

    #[must_use]
    pub fn add_column(mut self, table: &str, column: impl IntoColumnDef) -> Self {
        self.ops.push(Op::AddColumn {
            table: table.to_owned(),
            column: column.into_column_def(),
        });
        self
    }

    #[must_use]
    pub fn drop_column(mut self, table: &str, column: &str) -> Self {
        self.ops.push(Op::DropColumn {
            table: table.to_owned(),
            column: column.to_owned(),
        });
        self
    }

    #[must_use]
    pub fn rename_column(mut self, table: &str, from: &str, to: &str) -> Self {
        self.ops.push(Op::RenameColumn {
            table: table.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
        });
        self
    }

    /// Change le type ou l'obligation d'une colonne (`column` : nouvelle définition).
    #[must_use]
    pub fn alter_column(mut self, table: &str, column: impl IntoColumnDef) -> Self {
        self.ops.push(Op::AlterColumn {
            table: table.to_owned(),
            column: column.into_column_def(),
        });
        self
    }

    #[must_use]
    pub fn unique(mut self, table: &str, column: &str) -> Self {
        self.ops.push(Op::AddUnique {
            table: table.to_owned(),
            column: column.to_owned(),
        });
        self
    }

    #[must_use]
    pub fn drop_unique(mut self, table: &str, column: &str) -> Self {
        self.ops.push(Op::DropUnique {
            table: table.to_owned(),
            column: column.to_owned(),
        });
        self
    }

    #[must_use]
    pub fn reference(
        mut self,
        table: &str,
        column: &str,
        target: &str,
        on_delete: OnDelete,
    ) -> Self {
        self.ops.push(Op::AddReference {
            table: table.to_owned(),
            reference: Reference {
                column: column.to_owned(),
                target: target.to_owned(),
                on_delete,
            },
        });
        self
    }

    #[must_use]
    pub fn drop_reference(mut self, table: &str, column: &str) -> Self {
        self.ops.push(Op::DropReference {
            table: table.to_owned(),
            column: column.to_owned(),
        });
        self
    }

    /// Définition complète d'une table après les changements de ce plan.
    ///
    /// Utilisée uniquement par SQLite, qui reconstruit la table (voir le module).
    #[must_use]
    pub fn redefine(mut self, name: &str, define: impl FnOnce(&mut TableDef)) -> Self {
        let mut def = TableDef::default();
        define(&mut def);
        self.ops.push(Op::Redefine {
            name: name.to_owned(),
            def,
        });
        self
    }

    pub async fn apply(self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        if manager.get_database_backend() == DbBackend::Sqlite {
            self.apply_sqlite(manager).await
        } else {
            self.apply_server(manager).await
        }
    }

    /// PostgreSQL et MySQL : opérations directes, suppressions d'abord.
    async fn apply_server(self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        remove_phase(manager, &self.ops).await?;
        create_phase(manager, &self.ops).await
    }

    /// SQLite : créations et suppressions directes, tables modifiées reconstruites,
    /// le tout dans une transaction, clés étrangères vérifiées à la fin.
    async fn apply_sqlite(self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        let redefined: BTreeMap<&str, &TableDef> = self
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Redefine { name, def } => Some((name.as_str(), def)),
                _ => None,
            })
            .collect();
        if let Some(table) = self
            .ops
            .iter()
            .filter_map(Op::altered_table)
            .find(|t| !redefined.contains_key(t))
        {
            return Err(DbErr::Migration(format!(
                "la table `{table}` est modifiée sans `redefine` : SQLite ne peut pas l'altérer"
            )));
        }

        let db = manager.get_connection();
        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
        db.execute_unprepared("BEGIN").await?;
        let result = self.sqlite_steps(manager, &redefined).await;
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        db.execute_unprepared(end).await?;
        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
        result
    }

    async fn sqlite_steps(
        &self,
        manager: &SchemaManager<'_>,
        redefined: &BTreeMap<&str, &TableDef>,
    ) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for op in &self.ops {
            match op {
                Op::DropTable { name } => {
                    manager
                        .drop_table(Table::drop().table(Alias::new(name)).to_owned())
                        .await?;
                }
                Op::CreateTable { name, def } => {
                    manager
                        .create_table(def.create_statement(name, name, true))
                        .await?;
                    for column in &def.unique {
                        manager.create_index(unique_index(name, column)).await?;
                    }
                }
                Op::CreateJoinTable {
                    name,
                    source,
                    target,
                } => {
                    manager
                        .create_table(join_statement(name, true, source, target))
                        .await?;
                }
                _ => {}
            }
        }
        for (name, def) in redefined {
            let renames: BTreeMap<&str, &str> = self
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::RenameColumn { table, from, to } if table == name => {
                        Some((to.as_str(), from.as_str()))
                    }
                    _ => None,
                })
                .collect();
            rebuild_sqlite_table(manager, name, def, &renames).await?;
        }
        let violations = db
            .query_all_raw(sea_orm::Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA foreign_key_check",
            ))
            .await?;
        if violations.is_empty() {
            Ok(())
        } else {
            Err(DbErr::Migration(format!(
                "{} ligne(s) référencent des enregistrements inexistants après migration",
                violations.len()
            )))
        }
    }
}

/// Suppressions (contraintes, tables, colonnes) et renommages.
async fn remove_phase(manager: &SchemaManager<'_>, ops: &[Op]) -> Result<(), DbErr> {
    let postgres = manager.get_database_backend() == DbBackend::Postgres;
    for op in ops {
        if let Op::DropReference { table, column } = op {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name(format!("fk_{table}_{column}"))
                        .table(Alias::new(table))
                        .to_owned(),
                )
                .await?;
        }
    }
    for op in ops {
        if let Op::DropUnique { table, column } = op {
            manager
                .drop_index(
                    Index::drop()
                        .name(format!("uq_{table}_{column}"))
                        .table(Alias::new(table))
                        .to_owned(),
                )
                .await?;
        }
    }
    for op in ops {
        if let Op::DropTable { name } = op {
            let mut statement = Table::drop();
            statement.table(Alias::new(name));
            if postgres {
                statement.cascade();
            }
            manager.drop_table(statement).await?;
        }
    }
    for op in ops {
        let alter = match op {
            Op::DropColumn { table, column } => Table::alter()
                .table(Alias::new(table))
                .drop_column(Alias::new(column))
                .to_owned(),
            Op::RenameColumn { table, from, to } => Table::alter()
                .table(Alias::new(table))
                .rename_column(Alias::new(from), Alias::new(to))
                .to_owned(),
            _ => continue,
        };
        manager.alter_table(alter).await?;
    }

    Ok(())
}

/// Créations (tables, colonnes, modifications), puis index et clés étrangères.
async fn create_phase(manager: &SchemaManager<'_>, ops: &[Op]) -> Result<(), DbErr> {
    let mut foreign_keys = Vec::new();
    let mut unique = Vec::new();
    for op in ops {
        match op {
            Op::CreateTable { name, def } => {
                manager
                    .create_table(def.create_statement(name, name, false))
                    .await?;
                foreign_keys.extend(def.all_references().map(|r| foreign_key(name, &r)));
                unique.extend(def.unique.iter().map(|c| unique_index(name, c)));
            }
            Op::CreateJoinTable {
                name,
                source,
                target,
            } => {
                manager
                    .create_table(join_statement(name, false, source, target))
                    .await?;
                foreign_keys.extend(
                    join_references(source, target)
                        .iter()
                        .map(|r| foreign_key(name, r)),
                );
            }
            _ => {}
        }
    }
    for op in ops {
        let alter = match op {
            Op::AddColumn { table, column } => Table::alter()
                .table(Alias::new(table))
                .add_column(column.clone())
                .to_owned(),
            Op::AlterColumn { table, column } => Table::alter()
                .table(Alias::new(table))
                .modify_column(column.clone())
                .to_owned(),
            _ => continue,
        };
        manager.alter_table(alter).await?;
    }
    for op in ops {
        match op {
            Op::AddUnique { table, column } => unique.push(unique_index(table, column)),
            Op::AddReference { table, reference } => {
                foreign_keys.push(foreign_key(table, reference));
            }
            _ => {}
        }
    }
    for index in unique {
        manager.create_index(index).await?;
    }
    for key in foreign_keys {
        manager.create_foreign_key(key).await?;
    }
    Ok(())
}

/// Reconstruit une table SQLite selon `def` en conservant ses données.
/// `renames` associe un nouveau nom de colonne à l'ancien.
async fn rebuild_sqlite_table(
    manager: &SchemaManager<'_>,
    name: &str,
    def: &TableDef,
    renames: &BTreeMap<&str, &str>,
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let rows = db
        .query_all_raw(sea_orm::Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info(\"{name}\")"),
        ))
        .await?;
    let existing = rows
        .iter()
        .map(|row| row.try_get::<String>("", "name"))
        .collect::<Result<Vec<_>, _>>()?;

    let temporary = format!("__forge_new_{name}");
    manager
        .create_table(def.create_statement(name, &temporary, true))
        .await?;

    // Colonnes recopiées : celles qui existaient (éventuellement sous un autre nom).
    // Les nouvelles colonnes prennent leur valeur par défaut.
    let (targets, sources): (Vec<_>, Vec<_>) = def
        .column_names()
        .into_iter()
        .filter_map(|column| {
            let source = renames
                .get(column.as_str())
                .map_or(column.clone(), |s| (*s).to_owned());
            existing
                .contains(&source)
                .then(|| (Alias::new(column), Alias::new(source)))
        })
        .unzip();
    let copy = Query::insert()
        .into_table(Alias::new(&temporary))
        .columns(targets)
        .select_from(
            Query::select()
                .columns(sources)
                .from(Alias::new(name))
                .to_owned(),
        )
        .map_err(|err| DbErr::Migration(err.to_string()))?
        .to_owned();
    db.execute(&copy).await?;

    manager
        .drop_table(Table::drop().table(Alias::new(name)).to_owned())
        .await?;
    manager
        .rename_table(
            Table::rename()
                .table(Alias::new(&temporary), Alias::new(name))
                .to_owned(),
        )
        .await?;
    for column in &def.unique {
        manager.create_index(unique_index(name, column)).await?;
    }
    Ok(())
}

/// Tables système : `users` (cible de `owner`) et `parameters`.
struct SystemTables;

impl MigrationName for SystemTables {
    fn name(&self) -> &'static str {
        "forge_0001_system"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for SystemTables {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("users"))
                    .col(big_pk_auto("id"))
                    .col(string("email").unique_key())
                    .col(timestamp_with_time_zone("created_at"))
                    .col(timestamp_with_time_zone("updated_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("parameters"))
                    .col(string("name").primary_key())
                    .col(text_null("value"))
                    .col(timestamp_with_time_zone("updated_at"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in ["parameters", "users"] {
            manager
                .drop_table(Table::drop().table(Alias::new(name)).to_owned())
                .await?;
        }
        Ok(())
    }
}

/// Authentification : colonnes de `users`, rôles et jetons de rafraîchissement.
struct AuthTables;

impl MigrationName for AuthTables {
    fn name(&self) -> &'static str {
        "forge_0002_auth"
    }
}

/// Clé étrangère déclarée dans la création de la table (cibles déjà existantes).
fn cascade(
    table: &str,
    column: &str,
    target: &str,
    target_column: &str,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(format!("fk_{table}_{column}"))
        .from(Alias::new(table), Alias::new(column))
        .to(Alias::new(target), Alias::new(target_column))
        .on_delete(ForeignKeyAction::Cascade)
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for AuthTables {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Une colonne par instruction : SQLite n'en accepte pas plus.
        for column in [
            string_null("password_hash"),
            string_null("display_name"),
            boolean("active").default(true).to_owned(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .add_column(column)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("roles"))
                    .col(string("name").primary_key())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_roles"))
                    .col(big_integer("user_id"))
                    .col(string("role"))
                    .primary_key(
                        Index::create()
                            .col(Alias::new("user_id"))
                            .col(Alias::new("role")),
                    )
                    .foreign_key(&mut cascade("user_roles", "user_id", "users", "id"))
                    .foreign_key(&mut cascade("user_roles", "role", "roles", "name"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("refresh_tokens"))
                    .col(big_pk_auto("id"))
                    .col(big_integer("user_id"))
                    .col(string("token_hash").unique_key())
                    .col(timestamp_with_time_zone("expires_at"))
                    .col(timestamp_with_time_zone("created_at"))
                    .foreign_key(&mut cascade("refresh_tokens", "user_id", "users", "id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in ["refresh_tokens", "user_roles", "roles"] {
            manager
                .drop_table(Table::drop().table(Alias::new(name)).to_owned())
                .await?;
        }
        for column in ["active", "display_name", "password_hash"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::schema::string_null;

    #[test]
    fn table_definition_adds_system_columns() {
        let mut def = TableDef::default();
        def.col(string_null("titre"))
            .reference("client", "client", OnDelete::Restrict);
        let sql = DbBackend::Postgres
            .build(&def.create_statement("note", "note", false))
            .to_string();
        assert!(
            sql.starts_with(r#"CREATE TABLE "note" ( "id" bigint"#),
            "{sql}"
        );
        assert!(
            sql.contains(r#""titre" varchar NULL, "owner" bigint NULL"#),
            "{sql}"
        );
        assert_eq!(def.all_references().count(), 2);
        assert_eq!(
            def.column_names(),
            ["id", "titre", "owner", "created_at", "updated_at"]
        );
    }
}
