//! Migrations : tables système de forge et outils pour les migrations générées.
//!
//! Les migrations générées décrivent les tables avec [`Plan`], qui ajoute les
//! colonnes système et gère les clés étrangères selon la base :
//! SQLite les exige dans le `CREATE TABLE` (et accepte une table cible créée plus tard),
//! PostgreSQL et MySQL les reçoivent une fois toutes les tables créées.

use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{
    big_integer_null, big_pk_auto, string, text_null, timestamp_with_time_zone,
};

use crate::links;

/// Migrations système, à placer avant celles de l'application.
pub fn system() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(SystemTables)]
}

/// Suppression d'une ligne référencée.
#[derive(Debug, Clone, Copy)]
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

/// Création d'un ensemble de tables et de leurs clés étrangères.
#[derive(Debug, Default)]
pub struct Plan {
    tables: Vec<(String, TableCreateStatement)>,
    foreign_keys: Vec<(String, ForeignKeyCreateStatement)>,
}

impl Plan {
    pub fn new() -> Self {
        Self::default()
    }

    /// Table métier : `id`, puis les colonnes décrites par `columns`,
    /// puis `owner`, `created_at` et `updated_at`.
    #[must_use]
    pub fn table(mut self, name: &str, columns: impl FnOnce(&mut TableCreateStatement)) -> Self {
        let mut table = Table::create();
        table.table(Alias::new(name)).col(big_pk_auto("id"));
        columns(&mut table);
        table
            .col(big_integer_null("owner"))
            .col(timestamp_with_time_zone("created_at"))
            .col(timestamp_with_time_zone("updated_at"));
        self.tables.push((name.to_owned(), table));
        self.reference(name, "owner", "users", OnDelete::SetNull)
    }

    /// Table de jointure d'une colonne `reference_list`.
    #[must_use]
    pub fn join_table(mut self, name: &str, source: &str, target: &str) -> Self {
        let mut table = Table::create();
        table
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
        self.tables.push((name.to_owned(), table));
        self.reference(name, links::SOURCE, source, OnDelete::Cascade)
            .reference(name, links::TARGET, target, OnDelete::Cascade)
    }

    /// Clé étrangère `table.column → target.id`.
    #[must_use]
    pub fn reference(
        mut self,
        table: &str,
        column: &str,
        target: &str,
        on_delete: OnDelete,
    ) -> Self {
        let foreign_key = ForeignKey::create()
            .name(format!("fk_{table}_{column}"))
            .from(Alias::new(table), Alias::new(column))
            .to(Alias::new(target), Alias::new("id"))
            .on_delete(on_delete.into())
            .to_owned();
        self.foreign_keys.push((table.to_owned(), foreign_key));
        self
    }

    pub async fn create(self, manager: &SchemaManager<'_>) -> Result<(), DbErr> {
        let Self {
            mut tables,
            foreign_keys,
        } = self;
        if manager.get_database_backend() == DbBackend::Sqlite {
            for (table, mut foreign_key) in foreign_keys {
                if let Some((_, statement)) = tables.iter_mut().find(|(name, _)| *name == table) {
                    statement.foreign_key(&mut foreign_key);
                }
            }
            for (_, table) in tables {
                manager.create_table(table).await?;
            }
        } else {
            for (_, table) in tables {
                manager.create_table(table).await?;
            }
            for (_, foreign_key) in foreign_keys {
                manager.create_foreign_key(foreign_key).await?;
            }
        }
        Ok(())
    }
}

/// Supprime des tables, dans l'ordre inverse de leur création.
pub async fn drop_tables(manager: &SchemaManager<'_>, names: &[&str]) -> Result<(), DbErr> {
    let cascade = manager.get_database_backend() == DbBackend::Postgres;
    for name in names.iter().rev() {
        let mut statement = Table::drop();
        statement.table(Alias::new(*name)).if_exists();
        if cascade {
            statement.cascade();
        }
        manager.drop_table(statement).await?;
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
        drop_tables(manager, &["users", "parameters"]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::schema::string_null;

    #[test]
    fn plan_adds_system_columns_and_owner() {
        let plan = Plan::new().table("note", |t| {
            t.col(string_null("titre"));
        });
        let sql = DbBackend::Postgres.build(&plan.tables[0].1).to_string();
        assert!(
            sql.starts_with(r#"CREATE TABLE "note" ( "id" bigint"#),
            "{sql}"
        );
        assert!(
            sql.contains(r#""titre" varchar NULL, "owner" bigint NULL"#),
            "{sql}"
        );
        assert_eq!(plan.foreign_keys.len(), 1);
    }
}
