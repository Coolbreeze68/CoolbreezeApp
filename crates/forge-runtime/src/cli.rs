//! Ligne de commande des applications générées.
//!
//! ```text
//! mon_app                     # démarre le serveur (migrations appliquées au démarrage)
//! mon_app migrate up|down|status
//! ```
//!
//! Configuration par options ou variables d'environnement :
//! `DATABASE_URL`, `FORGE_ADDR`, `FORGE_AUTO_MIGRATE`, `FORGE_CACHE_TTL`,
//! `FORGE_CACHE_URL`, `RUST_LOG`, `FORGE_LOG_FORMAT`, et pour l'authentification
//! `FORGE_JWT_SECRET`, `FORGE_ADMIN_EMAIL`, `FORGE_ADMIN_PASSWORD`.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgAction, Parser, Subcommand};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::app::{App, DEFAULT_CACHE_CAPACITY};
use crate::auth::AuthConfig;
use crate::cache::MemoryCache;
use crate::error::Error;
use crate::observability::{self, LogFormat};

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

    /// Durée de vie des lectures en cache, en secondes (0 : cache désactivé).
    #[arg(long, env = "FORGE_CACHE_TTL", default_value_t = 60)]
    cache_ttl: u64,

    /// Cache Redis partagé (`redis://hôte:6379`) ; sans lui, cache en mémoire.
    #[arg(long, env = "FORGE_CACHE_URL")]
    cache_url: Option<String>,

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
    observability::init_logging(LogFormat::from_env());
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
    match cli.command {
        Some(Command::Migrate { action }) => {
            let db = connect(&cli.database_url, true).await?;
            match action {
                MigrateAction::Up => Ok(M::up(&db, None).await?),
                MigrateAction::Down => Ok(M::down(&db, Some(1)).await?),
                MigrateAction::Status => Ok(M::status(&db).await?),
            }
        }
        None => {
            if cli.auto_migrate {
                let db = connect(&cli.database_url, true).await?;
                M::up(&db, None).await?;
                db.close().await?;
            }
            let app = with_cache(app()?, cli.cache_ttl, cli.cache_url.as_deref()).await?;
            serve(app, connect(&cli.database_url, false).await?, cli.addr).await
        }
    }
}

/// Connexion à la base. Pour migrer SQLite, une seule connexion est ouverte :
/// les reconstructions de tables désactivent les clés étrangères, réglage propre
/// à chaque connexion (voir [`crate::migration`]).
pub(crate) async fn connect(url: &str, migrating: bool) -> Result<DatabaseConnection, Error> {
    let mut options = ConnectOptions::new(url);
    if migrating && url.starts_with("sqlite:") {
        options.max_connections(1);
    }
    Database::connect(options)
        .await
        .map_err(|err| Error::Config(format!("connexion à la base impossible : {err}")))
}

/// Cache des lectures : désactivé (`ttl` nul), Redis (`url`) ou en mémoire.
/// Asynchrone pour la connexion à Redis, absente sans la feature `redis`.
#[cfg_attr(not(feature = "redis"), allow(clippy::unused_async))]
async fn with_cache(app: App, ttl: u64, url: Option<&str>) -> Result<App, Error> {
    if ttl == 0 {
        return Ok(app.without_cache());
    }
    let ttl = Duration::from_secs(ttl);
    match url {
        None => Ok(app.cache(MemoryCache::new(ttl, DEFAULT_CACHE_CAPACITY))),
        #[cfg(feature = "redis")]
        Some(url) => {
            let namespace = app.schema().spec().app.name.clone();
            let cache = crate::cache::RedisCache::connect(url, &namespace, ttl).await?;
            tracing::info!("cache Redis activé");
            Ok(app.cache(cache))
        }
        #[cfg(not(feature = "redis"))]
        Some(_) => Err(Error::Config(
            "FORGE_CACHE_URL exige la feature `redis` de forge-runtime (backend/Cargo.toml)".into(),
        )),
    }
}

async fn serve(app: App, db: DatabaseConnection, addr: SocketAddr) -> Result<(), Error> {
    let router = app.into_router(db, AuthConfig::from_env()).await?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("écoute sur http://{addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
