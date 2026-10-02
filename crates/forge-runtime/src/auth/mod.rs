//! Authentification : jetons JWT, utilisateur courant, connexion.
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `POST` | `/api/auth/login` | `{ email, password }` → session |
//! | `POST` | `/api/auth/refresh` | `{ refresh_token }` → nouvelle session (l'ancien jeton est révoqué) |
//! | `POST` | `/api/auth/logout` | `{ refresh_token }` → révocation |
//! | `GET` | `/api/auth/me` | utilisateur connecté |
//! | `PUT` | `/api/auth/password` | `{ current_password, new_password }` |
//!
//! Une session contient un jeton d'accès JWT (`Authorization: Bearer …`, courte durée)
//! et un jeton de rafraîchissement opaque, stocké haché en base. Toutes les routes
//! `/api/` exigent un jeton d'accès, sauf la connexion et le rafraîchissement.

mod entities;
mod password;
pub(crate) mod users;

use std::fmt;
use std::time::Duration;

use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::error::Error;
use entities::{refresh_token, user};

/// Routes accessibles sans jeton d'accès.
const PUBLIC_PATHS: [&str; 2] = ["/api/auth/login", "/api/auth/refresh"];

/// Rôle disposant de tous les droits, sans considération des règles.
pub const ADMIN_ROLE: &str = "admin";

/// Compte administrateur créé au démarrage si la base n'a aucun utilisateur.
#[derive(Clone)]
pub struct InitialAdmin {
    pub email: String,
    pub password: String,
}

impl fmt::Debug for InitialAdmin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InitialAdmin")
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

/// Configuration de l'authentification.
#[derive(Clone)]
pub struct AuthConfig {
    secret: Vec<u8>,
    pub access_ttl: Duration,
    pub refresh_ttl: Duration,
    pub initial_admin: Option<InitialAdmin>,
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthConfig")
            .field("access_ttl", &self.access_ttl)
            .field("refresh_ttl", &self.refresh_ttl)
            .field("initial_admin", &self.initial_admin)
            .finish_non_exhaustive()
    }
}

impl AuthConfig {
    /// Jetons d'accès de 15 minutes, de rafraîchissement de 30 jours.
    pub fn new(secret: impl Into<Vec<u8>>) -> Self {
        Self {
            secret: secret.into(),
            access_ttl: Duration::from_secs(15 * 60),
            refresh_ttl: Duration::from_secs(30 * 24 * 3600),
            initial_admin: None,
        }
    }

    /// Lit `FORGE_JWT_SECRET`, `FORGE_ADMIN_EMAIL` et `FORGE_ADMIN_PASSWORD`.
    ///
    /// Sans secret, un secret aléatoire est tiré : les sessions ne survivent
    /// alors pas au redémarrage du serveur.
    pub fn from_env() -> Self {
        let secret = std::env::var("FORGE_JWT_SECRET").map_or_else(
            |_| {
                tracing::warn!(
                    "FORGE_JWT_SECRET absent : secret aléatoire, sessions perdues au redémarrage"
                );
                random_bytes().to_vec()
            },
            String::into_bytes,
        );
        let mut config = Self::new(secret);
        if let (Ok(email), Ok(password)) = (
            std::env::var("FORGE_ADMIN_EMAIL"),
            std::env::var("FORGE_ADMIN_PASSWORD"),
        ) {
            config.initial_admin = Some(InitialAdmin { email, password });
        }
        config
    }

    fn issue_access(&self, user: &CurrentUser) -> Result<String, Error> {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            roles: user.roles.clone(),
            iat: now,
            exp: now + i64::try_from(self.access_ttl.as_secs()).unwrap_or(i64::MAX),
        };
        jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(&self.secret),
        )
        .map_err(|err| Error::Config(format!("signature du jeton : {err}")))
    }

    fn verify_access(&self, token: &str) -> Result<CurrentUser, Error> {
        let data = jsonwebtoken::decode::<Claims>(
            token,
            &DecodingKey::from_secret(&self.secret),
            &Validation::default(),
        )
        .map_err(|_| Error::Unauthorized)?;
        Ok(CurrentUser {
            id: data.claims.sub.parse().map_err(|_| Error::Unauthorized)?,
            email: data.claims.email,
            roles: data.claims.roles,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    email: String,
    roles: Vec<String>,
    iat: i64,
    exp: i64,
}

/// Utilisateur authentifié de la requête (lu dans le jeton d'accès).
///
/// Extracteur axum, utilisable dans les routes personnalisées :
/// `async fn handler(user: CurrentUser) -> …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUser {
    pub id: i64,
    pub email: String,
    pub roles: Vec<String>,
}

impl CurrentUser {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn is_admin(&self) -> bool {
        self.has_role(ADMIN_ROLE)
    }

    /// Erreur 403 si l'utilisateur n'est pas administrateur.
    pub fn require_admin(&self) -> Result<(), Error> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(Error::Forbidden("réservé aux administrateurs".into()))
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for CurrentUser {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Error> {
        parts
            .extensions
            .get::<Self>()
            .cloned()
            .ok_or(Error::Unauthorized)
    }
}

/// Middleware : exige un jeton d'accès valide sur `/api/`, sauf routes publiques.
pub(crate) async fn authenticate(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, Error> {
    let path = request.uri().path();
    if path.starts_with("/api/") && !PUBLIC_PATHS.contains(&path) {
        let token = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(Error::Unauthorized)?;
        let user = state.auth.verify_access(token)?;
        request.extensions_mut().insert(user);
    }
    Ok(next.run(request).await)
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/auth/password", put(change_password))
        .merge(users::router())
}

fn random_bytes() -> [u8; 32] {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn token_hash(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

/// Ouvre une session : jeton d'accès et jeton de rafraîchissement (stocké haché).
async fn open_session(state: &AppState, account: &user::Model) -> Result<JsonValue, Error> {
    let roles = users::roles_of(&state.db, account.id).await?;
    let current = CurrentUser {
        id: account.id,
        email: account.email.clone(),
        roles,
    };
    let access_token = state.auth.issue_access(&current)?;
    let refresh = URL_SAFE_NO_PAD.encode(random_bytes());
    let now = chrono::Utc::now();
    let refresh_ttl =
        chrono::Duration::from_std(state.auth.refresh_ttl).unwrap_or(chrono::Duration::MAX);
    refresh_token::ActiveModel {
        user_id: Set(account.id),
        token_hash: Set(token_hash(&refresh)),
        expires_at: Set(now + refresh_ttl),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;
    Ok(json!({
        "access_token": access_token,
        "refresh_token": refresh,
        "token_type": "Bearer",
        "expires_in": state.auth.access_ttl.as_secs(),
        "user": users::to_json(&state.db, account).await?,
    }))
}

#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<Credentials>,
) -> Result<Json<JsonValue>, Error> {
    let account = user::Entity::find()
        .filter(user::Column::Email.eq(users::normalize_email(&body.email)))
        .one(&state.db)
        .await?;
    let hash = account.as_ref().and_then(|a| a.password_hash.as_deref());
    let valid = password::verify(&body.password, hash);
    match account {
        Some(account) if valid && account.active => Ok(Json(open_session(&state, &account).await?)),
        _ => Err(Error::Unauthorized),
    }
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

/// Retire un jeton de rafraîchissement et retourne son enregistrement s'il existait.
async fn revoke(
    db: &impl ConnectionTrait,
    token: &str,
) -> Result<Option<refresh_token::Model>, Error> {
    let stored = refresh_token::Entity::find()
        .filter(refresh_token::Column::TokenHash.eq(token_hash(token)))
        .one(db)
        .await?;
    if let Some(stored) = &stored {
        refresh_token::Entity::delete_by_id(stored.id)
            .exec(db)
            .await?;
    }
    Ok(stored)
}

async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<JsonValue>, Error> {
    let stored = revoke(&state.db, &body.refresh_token)
        .await?
        .ok_or(Error::Unauthorized)?;
    if stored.expires_at < chrono::Utc::now() {
        return Err(Error::Unauthorized);
    }
    let account = user::Entity::find_by_id(stored.user_id)
        .one(&state.db)
        .await?
        .filter(|a| a.active)
        .ok_or(Error::Unauthorized)?;
    Ok(Json(open_session(&state, &account).await?))
}

async fn logout(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<StatusCode, Error> {
    revoke(&state.db, &body.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(state): State<AppState>, current: CurrentUser) -> Result<Json<JsonValue>, Error> {
    let account = user::Entity::find_by_id(current.id)
        .one(&state.db)
        .await?
        .ok_or(Error::Unauthorized)?;
    Ok(Json(users::to_json(&state.db, &account).await?))
}

#[derive(Deserialize)]
struct PasswordChange {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    current: CurrentUser,
    Json(body): Json<PasswordChange>,
) -> Result<StatusCode, Error> {
    let account = user::Entity::find_by_id(current.id)
        .one(&state.db)
        .await?
        .ok_or(Error::Unauthorized)?;
    if !password::verify(&body.current_password, account.password_hash.as_deref()) {
        return Err(Error::validation(
            "current_password",
            "mot de passe actuel incorrect",
        ));
    }
    users::set_password(&state.db, account, &body.new_password).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_tokens_roundtrip_and_reject_tampering() {
        let config = AuthConfig::new("secret");
        let user = CurrentUser {
            id: 7,
            email: "a@b.c".into(),
            roles: vec!["commercial".into()],
        };
        let token = config.issue_access(&user).unwrap();
        assert_eq!(config.verify_access(&token).unwrap(), user);
        assert!(AuthConfig::new("autre").verify_access(&token).is_err());
        assert!(config.verify_access("n'importe quoi").is_err());
    }

    #[test]
    fn expired_tokens_are_rejected() {
        let mut config = AuthConfig::new("secret");
        config.access_ttl = Duration::ZERO;
        let user = CurrentUser {
            id: 1,
            email: "a@b.c".into(),
            roles: Vec::new(),
        };
        // La validation tolère 60 s de décalage d'horloge : on remonte l'émission.
        let claims = Claims {
            sub: "1".into(),
            email: user.email.clone(),
            roles: Vec::new(),
            iat: 0,
            exp: chrono::Utc::now().timestamp() - 120,
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(matches!(
            config.verify_access(&token),
            Err(Error::Unauthorized)
        ));
    }

    #[test]
    fn refresh_tokens_are_random_and_hashed() {
        let a = URL_SAFE_NO_PAD.encode(random_bytes());
        let b = URL_SAFE_NO_PAD.encode(random_bytes());
        assert_ne!(a, b);
        assert_eq!(token_hash(&a), token_hash(&a));
        assert_ne!(token_hash(&a), a);
    }
}
