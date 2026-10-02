//! Ligne de commande des applications générées.
//!
//! ```text
//! mon_app                     # démarre le serveur (migrations appliquées au démarrage)
//! mon_app migrate up|down|status
//! ```
//!
//! Configuration par options ou variables d'environnement :
//! `DATABASE_URL`, `FORGE_ADDR`, `FORGE_AUTO_MIGRATE`, `FORGE_CACHE_TTL`,
//! `FORGE_CACHE_URL`, `FORGE_CORS_ORIGINS`, `RUST_LOG`, `FORGE_LOG_FORMAT`, et pour l'authentification
//! `FORGE_JWT_SECRET`, `FORGE_ADMIN_EMAIL`, `FORGE_ADMIN_PASSWORD`.

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use axum::http::header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use clap::{ArgAction, Parser, Subcommand};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tower_http::cors::{AllowOrigin, CorsLayer};

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

    /// Origines autorisées à appeler l'API depuis un navigateur (application web
    /// servie ailleurs), séparées par des virgules ; `*` pour toutes.
    #[arg(long, env = "FORGE_CORS_ORIGINS", value_delimiter = ',')]
    cors_origins: Vec<String>,

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
            let cors = cors(&cli.cors_origins)?;
            serve(
                app,
                connect(&cli.database_url, false).await?,
                cli.addr,
                cors,
            )
            .await
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

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Accès depuis d'autres origines : aucun par défaut (application web servie
/// par le même domaine que l'API).
fn cors(origins: &[String]) -> Result<Option<CorsLayer>, Error> {
    let origins: Vec<&str> = origins
        .iter()
        .map(|o| o.trim())
        .filter(|o| !o.is_empty())
        .collect();
    if origins.is_empty() {
        return Ok(None);
    }
    let allowed = if origins.contains(&"*") {
        AllowOrigin::any()
    } else {
        let values = origins
            .iter()
            .map(|o| {
                HeaderValue::from_str(o)
                    .map_err(|_| Error::Config(format!("origine CORS invalide : `{o}`")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        AllowOrigin::list(values)
    };
    Ok(Some(
        CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE, REQUEST_ID])
            .expose_headers([REQUEST_ID, CONTENT_DISPOSITION]),
    ))
}

async fn serve(
    app: App,
    db: DatabaseConnection,
    addr: SocketAddr,
    cors: Option<CorsLayer>,
) -> Result<(), Error> {
    let mut router = app.into_router(db, AuthConfig::from_env()).await?;
    if let Some(cors) = cors {
        router = router.layer(cors);
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("écoute sur http://{addr}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;

    async fn preflight(origins: &[&str], origin: &str) -> Option<String> {
        let origins: Vec<String> = origins.iter().map(|o| (*o).to_owned()).collect();
        let mut router = Router::new().route("/api/tag", get(|| async { "ok" }));
        if let Some(layer) = cors(&origins).unwrap() {
            router = router.layer(layer);
        }
        let response = router
            .oneshot(
                Request::options("/api/tag")
                    .header("origin", origin)
                    .header("access-control-request-method", "PATCH")
                    .header("access-control-request-headers", "authorization")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        response
            .headers()
            .get("access-control-allow-origin")
            .map(|v| v.to_str().unwrap().to_owned())
    }

    #[tokio::test]
    async fn cors_origins() {
        assert_eq!(preflight(&[], "http://localhost:3000").await, None);
        let listed = ["http://localhost:3000", " https://app.test "];
        assert_eq!(
            preflight(&listed, "http://localhost:3000").await.as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(preflight(&listed, "http://evil.test").await, None);
        assert_eq!(
            preflight(&["*"], "http://evil.test").await.as_deref(),
            Some("*")
        );
        assert!(cors(&["pas\u{1}valide".to_owned()]).is_err());
    }
}
