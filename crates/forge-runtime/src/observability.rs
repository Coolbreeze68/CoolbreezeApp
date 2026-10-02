//! Observabilité : journaux, identifiant de requête, métriques, santé.
//!
//! | Chemin | Contenu |
//! |---|---|
//! | `/health` | `200 { "status": "ok", "database": "ok" }`, ou `503` si la base ne répond pas |
//! | `/metrics` | métriques Prometheus (format texte) |
//!
//! Chaque requête reçoit un identifiant (`x-request-id`, repris s'il est fourni),
//! renvoyé dans la réponse et présent dans tous ses journaux. Métriques :
//!
//! - `http_requests_total{method, path, status}` et
//!   `http_request_duration_seconds{method, path}` (histogramme) ; `path` est le
//!   modèle de route (`/api/opportunite/{id}`), pas l'URL ;
//! - `forge_cache_requests_total{table, result}` (`hit` ou `miss`).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, middleware};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use serde_json::json;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use crate::app::AppState;

/// Bornes de l'histogramme des durées de requête, en secondes.
const DURATION_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Format des journaux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Lisible, pour le développement.
    Text,
    /// Une ligne JSON par événement, avec les champs de la requête : pour la production.
    Json,
}

impl LogFormat {
    /// `FORGE_LOG_FORMAT` (`text` ou `json`) ; par défaut `json` dans un binaire
    /// optimisé (`--release`), `text` sinon.
    pub fn from_env() -> Self {
        match std::env::var("FORGE_LOG_FORMAT").as_deref() {
            Ok("json") => Self::Json,
            Ok("text") => Self::Text,
            _ if cfg!(debug_assertions) => Self::Text,
            _ => Self::Json,
        }
    }
}

/// Initialise les journaux (`RUST_LOG`, `info,sqlx=warn` par défaut).
pub fn init_logging(format: LogFormat) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .init(),
    }
}

/// Enregistreur Prometheus du processus (unique : les métriques sont globales).
/// `None` si un autre enregistreur a déjà été installé par l'application.
fn prometheus() -> Option<&'static PrometheusHandle> {
    static HANDLE: OnceLock<Option<PrometheusHandle>> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("http_request_duration_seconds".into()),
                    &DURATION_BUCKETS,
                )
                .and_then(PrometheusBuilder::install_recorder)
                .inspect_err(
                    |err| tracing::warn!(error = %err, "métriques Prometheus indisponibles"),
                )
                .ok()
        })
        .as_ref()
}

/// Routes `/health` et `/metrics`.
pub(crate) fn router() -> Router<AppState> {
    prometheus();
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(render_metrics))
}

/// Couches communes à toutes les routes : identifiant de requête, trace, métriques.
pub(crate) fn layer(router: Router) -> Router {
    let trace = TraceLayer::new_for_http()
        .make_span_with(|request: &Request| {
            let id = request
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            tracing::info_span!(
                "requête",
                method = %request.method(),
                path = %request.uri().path(),
                request_id = %id,
            )
        })
        .on_response(DefaultOnResponse::new().level(Level::INFO));
    // La dernière couche ajoutée s'exécute en premier : l'identifiant est posé
    // avant la trace, qui le lit.
    router
        .layer(middleware::from_fn(track))
        .layer(trace)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
}

/// Compte les requêtes et mesure leur durée, par modèle de route.
async fn track(request: Request, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "inconnu".to_owned(), |p| p.as_str().to_owned());
    let start = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();
    metrics::counter!("http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status)
        .increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "path" => path)
        .record(start.elapsed().as_secs_f64());
    response
}

async fn health(State(state): State<AppState>) -> Response {
    let ping = tokio::time::timeout(Duration::from_secs(2), state.db.ping()).await;
    match ping {
        Ok(Ok(())) => Json(json!({ "status": "ok", "database": "ok" })).into_response(),
        Ok(Err(err)) => unavailable(&err.to_string()),
        Err(_) => unavailable("délai dépassé"),
    }
}

fn unavailable(reason: &str) -> Response {
    tracing::error!(reason, "base de données indisponible");
    let body = json!({ "status": "error", "database": "indisponible" });
    (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
}

async fn render_metrics() -> Response {
    match prometheus() {
        Some(handle) => {
            handle.run_upkeep();
            (
                [("content-type", "text/plain; version=0.0.4")],
                handle.render(),
            )
                .into_response()
        }
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
