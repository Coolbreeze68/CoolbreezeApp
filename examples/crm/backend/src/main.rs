//! `cargo run` démarre le serveur ; `cargo run -- migrate up|down|status` gère la base.
//! Configuration : `DATABASE_URL`, `FORGE_ADDR`, `FORGE_AUTO_MIGRATE`, `RUST_LOG`.

use std::process::ExitCode;

use mini_crm::generated::{Migrator, app};

#[tokio::main]
async fn main() -> ExitCode {
    forge_runtime::cli::run::<Migrator>(app).await
}
