//! Ligne de commande des applications générées.
//!
//! ```text
//! mon_app                     # démarre le serveur (migrations appliquées au démarrage)
//! mon_app migrate up|down|status
//! ```
//!
//! Configuration par options ou variables d'environnement :
//! `DATABASE_URL`, `FORGE_ADDR`, `FORGE_AUTO_MIGRATE`, `RUST_LOG`.

use std::net::SocketAddr;
use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::error::Error;

#[derive(Debug, Parser)]
#[command(about = "Application générée par forge")]
struct Cli {
    /// URL de la base : `sqlite://data.db?mode=rwc`, `postgres://…`, `mysql://…`.
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "sqlite://data.db?mode=rwc",
        global = true
    )]
    database_url: String,

    /// Adresse d'écoute du serveur.
    #[arg(long, env = "FORGE_ADDR", default_value = "0.0.0.0:8080")]
    addr: SocketAddr,

    /// Applique les migrations en attente au démarrage du serveur.
    #[arg(long, env = "FORGE_AUTO_MIGRATE", default_value_t = true, action = ArgAction::Set)]
    auto_migrate: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Gère les migrations de la base.
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
}

#[derive(Debug, Subcommand)]
enum MigrateAction {
    /// Applique les migrations en attente.
    Up,
    /// Annule la dernière migration.
    Down,
    /// Affiche l'état des migrations.
    Status,
}

/// Point d'entrée du binaire généré.
pub async fn run<M: MigratorTrait>(app: impl FnOnce() -> Result<App, Error>) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();
    match execute::<M>(Cli::parse(), app).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

async fn execute<M: MigratorTrait>(
    cli: Cli,
    app: impl FnOnce() -> Result<App, Error>,
) -> Result<(), Error> {
    let db = connect(&cli.database_url).await?;
    match cli.command {
        Some(Command::Migrate { action }) => match action {
            MigrateAction::Up => Ok(M::up(&db, None).await?),
            MigrateAction::Down => Ok(M::down(&db, Some(1)).await?),
            MigrateAction::Status => Ok(M::status(&db).await?),
        },
        None => {
            if cli.auto_migrate {
                M::up(&db, None).await?;
            }
            serve(app()?, db, cli.addr).await
        }
    }
}

pub(crate) async fn connect(url: &str) -> Result<DatabaseConnection, Error> {
    Database::connect(url)
        .await
        .map_err(|err| Error::Config(format!("connexion à la base impossible : {err}")))
}

async fn serve(app: App, db: DatabaseConnection, addr: SocketAddr) -> Result<(), Error> {
    let router = app.into_router(db).await?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("écoute sur http://{addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
