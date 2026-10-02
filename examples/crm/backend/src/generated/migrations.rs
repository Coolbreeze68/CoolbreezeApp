//! NE PAS MODIFIER : liste les migrations de `src/migrations/`, dans l'ordre.

use sea_orm_migration::prelude::*;

#[path = "../migrations/m0001_init.rs"]
mod m0001_init;
#[path = "../migrations/m0002_opportunite.rs"]
mod m0002_opportunite;

/// Migrations système de forge, puis celles de l'application.
pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = forge_runtime::migration::system();
        migrations.push(Box::new(m0001_init::Migration));
        migrations.push(Box::new(m0002_opportunite::Migration));
        migrations
    }
}
