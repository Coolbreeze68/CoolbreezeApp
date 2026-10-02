//! NE PAS MODIFIER : généré par forge. Vérifie l'API de chaque table : CRUD
//! complet, puis droits de chaque rôle selon les règles du schéma.
//!
//! Base de test : `TEST_DATABASE_URL` (SQLite en mémoire par défaut).
//! Attention : sur PostgreSQL et MySQL, toutes les tables de cette base sont recréées.

use mini_crm::generated::{Migrator, app};

#[tokio::test]
async fn crud_de_chaque_table() {
    let db = forge_runtime::testing::database::<Migrator>().await;
    forge_runtime::testing::check_resources(app().expect("application"), db).await;
}

#[tokio::test]
async fn droits_de_chaque_role() {
    let db = forge_runtime::testing::database::<Migrator>().await;
    forge_runtime::testing::check_rules(app().expect("application"), db).await;
}
