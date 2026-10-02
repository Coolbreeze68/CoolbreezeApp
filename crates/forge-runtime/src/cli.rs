//! Ligne de commande des applications générées.
//!
//! ```text
//! mon_app                     # démarre le serveur (migrations appliquées au démarrage)
//! mon_app migrate up|down|status
//! mon_app health              # le serveur local répond-il ? (HEALTHCHECK Docker)
//! ```
//!
//! Configuration par options ou variables d'environnement :
//! `DATABASE_URL`, `FORGE_ADDR`, `FORGE_AUTO_MIGRATE`, `FORGE_CACHE_TTL`,
//! `FORGE_CACHE_URL`, `FORGE_CORS_ORIGINS`, `FORGE_STATIC_DIR`, `RUST_LOG`, `FORGE_LOG_FORMAT`,
//! et pour l'authentification
//! `FORGE_JWT_SECRET`, `FORGE_ADMIN_EMAIL`, `FORGE_ADMIN_PASSWORD`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use axum::Router;
use axum::extract::Request;
use axum::http::header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use axum::response::IntoResponse;
use clap::{ArgAction, Parser, Subcommand};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

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

    /// Application web à servir (dossier de `flutter build web`) : toute
    /// adresse hors de l'API y est cherchée, `index.html` par défaut.
    #[arg(long, env = "FORGE_STATIC_DIR")]
    static_dir: Option<PathBuf>,

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
    /// Vérifie que le serveur démarré sur le port de `FORGE_ADDR` répond
    /// (code de sortie non nul sinon).
    Health,
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
        Some(Command::Health) => health(cli.addr.port()).await,
        None => {
            if cli.auto_migrate {
                let db = connect(&cli.database_url, true).await?;
                M::up(&db, None).await?;
                db.close().await?;
            }
            // Une variable vide (docker-compose) vaut une variable absente.
            let cache_url = cli.cache_url.as_deref().filter(|u| !u.is_empty());
            let app = with_cache(app()?, cli.cache_ttl, cache_url).await?;
            let db = connect(&cli.database_url, false).await?;
            let mut router = app.into_router(db, AuthConfig::from_env()).await?;
            if let Some(dir) = cli.static_dir.filter(|d| !d.as_os_str().is_empty()) {
                router = with_web_app(router, &dir)?;
            }
            if let Some(cors) = cors(&cli.cors_origins)? {
                router = router.layer(cors);
            }
            serve(router, cli.addr).await
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

/// Sert l'application web pour toute adresse sans route, hors de `/api/`
/// (une route d'API inconnue reste une erreur JSON).
fn with_web_app(router: Router, dir: &Path) -> Result<Router, Error> {
    let index = dir.join("index.html");
    if !index.is_file() {
        return Err(Error::Config(format!(
            "FORGE_STATIC_DIR : `{}` introuvable",
            index.display()
        )));
    }
    let files = ServeDir::new(dir).fallback(ServeFile::new(index));
    tracing::info!("application web servie depuis {}", dir.display());
    Ok(router.fallback(move |request: Request| async move {
        if request.uri().path().starts_with("/api/") {
            return Error::NotFound.into_response();
        }
        files.oneshot(request).await.into_response()
    }))
}

/// `GET /health` sur le serveur local, sans client HTTP : l'image Docker
/// n'embarque pas `curl`.
async fn health(port: u16) -> Result<(), Error> {
    let unavailable = |detail: String| Error::Config(format!("serveur indisponible : {detail}"));
    let check = async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        stream
            .write_all(b"GET /health HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        Ok::<_, std::io::Error>(response)
    };
    let response = tokio::time::timeout(Duration::from_secs(5), check)
        .await
        .map_err(|_| unavailable("pas de réponse en 5 s".into()))?
        .map_err(|err| unavailable(err.to_string()))?;
    let status = String::from_utf8_lossy(&response);
    let status = status.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        Err(unavailable(status.to_owned()))
    }
}

async fn serve(router: Router, addr: SocketAddr) -> Result<(), Error> {
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
    use axum::body::Body;
    use axum::routing::get as get_route;

    use super::*;

    async fn preflight(origins: &[&str], origin: &str) -> Option<String> {
        let origins: Vec<String> = origins.iter().map(|o| (*o).to_owned()).collect();
        let mut router = Router::new().route("/api/tag", get_route(|| async { "ok" }));
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

    async fn get(router: Router, path: &str) -> (u16, String) {
        let response = router
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status().as_u16();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn web_app_served_outside_the_api() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>app</html>").unwrap();
        std::fs::write(dir.path().join("main.dart.js"), "js").unwrap();
        let api = Router::new().route("/api/tag", get_route(|| async { "api" }));
        let router = with_web_app(api, dir.path()).unwrap();
        assert_eq!(get(router.clone(), "/api/tag").await, (200, "api".into()));
        assert_eq!(
            get(router.clone(), "/main.dart.js").await,
            (200, "js".into())
        );
        assert_eq!(get(router.clone(), "/").await.1, "<html>app</html>");
        // Adresse inconnue : l'application (routage côté client).
        assert_eq!(
            get(router.clone(), "/data/contact").await.1,
            "<html>app</html>"
        );
        let (status, body) = get(router, "/api/inconnu").await;
        assert_eq!(status, 404);
        assert!(body.contains("not_found"));

        let empty = tempfile::tempdir().unwrap();
        assert!(with_web_app(Router::new(), empty.path()).is_err());
    }

    #[tokio::test]
    async fn health_check() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = Router::new().route("/health", get_route(|| async { "ok" }));
        tokio::spawn(async move { axum::serve(listener, router).await });
        assert!(health(port).await.is_ok());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, Router::new()).await });
        assert!(health(port).await.unwrap_err().to_string().contains("404"));
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
