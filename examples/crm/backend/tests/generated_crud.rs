//! NE PAS MODIFIER : généré par forge. Vérifie le CRUD de chaque table via l'API.
//!
//! Base de test : `TEST_DATABASE_URL` (SQLite en mémoire par défaut).
//! Attention : sur PostgreSQL et MySQL, toutes les tables de cette base sont recréées.

use mini_crm::generated::{Migrator, app};

#[tokio::test]
async fn crud_de_chaque_table() {
    let db = forge_runtime::testing::database::<Migrator>().await;
    forge_runtime::testing::check_resources(app().expect("application"), db).await;
}
