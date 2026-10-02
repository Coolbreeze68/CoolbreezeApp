use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sea_orm::{DbErr, SqlErr};
use serde_json::json;

/// Erreurs par champ : nom du champ → messages.
pub type FieldErrors = BTreeMap<String, Vec<String>>;

/// Erreur du runtime, convertie en réponse HTTP JSON :
///
/// ```json
/// { "error": { "code": "validation", "message": "…", "fields": { "montant": ["…"] } } }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("données invalides")]
    Validation(FieldErrors),
    #[error("requête invalide : {0}")]
    BadRequest(String),
    #[error("authentification requise")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("enregistrement introuvable")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("schéma embarqué invalide : {0}")]
    Schema(#[from] forge_schema::SchemaError),
    #[error("configuration : {0}")]
    Config(String),
    #[error("base de données : {0}")]
    Database(DbErr),
    #[error("entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Erreur de validation sur un seul champ, pratique dans les hooks.
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation(BTreeMap::from([(field.into(), vec![message.into()])]))
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Schema(_) | Self::Config(_) | Self::Database(_) | Self::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation",
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::Schema(_) | Self::Config(_) | Self::Database(_) | Self::Io(_) => "internal",
        }
    }
}

/// SQLite signale une suppression bloquée par `ON DELETE RESTRICT` avec le code
/// 1811, que `DbErr::sql_err` ne classe pas : on se rabat sur son message, stable.
fn is_sqlite_restrict(err: &DbErr) -> bool {
    err.to_string().contains("FOREIGN KEY constraint failed")
}

impl From<DbErr> for Error {
    fn from(err: DbErr) -> Self {
        let kind = err.sql_err().or_else(|| {
            is_sqlite_restrict(&err).then(|| SqlErr::ForeignKeyConstraintViolation(String::new()))
        });
        match kind {
            Some(SqlErr::UniqueConstraintViolation(_)) => Self::Conflict(
                "une valeur unique est déjà utilisée par un autre enregistrement".into(),
            ),
            Some(SqlErr::ForeignKeyConstraintViolation(_)) => Self::Conflict(
                "référence invalide, ou enregistrement encore référencé par d'autres données"
                    .into(),
            ),
            _ => match err {
                DbErr::RecordNotFound(_) => Self::NotFound,
                err => Self::Database(err),
            },
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, "erreur interne");
        }
        // Les détails internes ne sont pas exposés au client.
        let message = if status.is_server_error() {
            "erreur interne du serveur".to_owned()
        } else {
            self.to_string()
        };
        let mut body = json!({ "code": self.code(), "message": message });
        if let Self::Validation(fields) = &self {
            body["fields"] = json!(fields);
        }
        (status, Json(json!({ "error": body }))).into_response()
    }
}
