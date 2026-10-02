//! Comptes utilisateurs et rôles.
//!
//! Réservé aux administrateurs :
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `GET` | `/api/users` | liste (`page`, `per_page`, `q`) |
//! | `POST` | `/api/users` | création : `{ email, password, display_name?, active?, roles? }` |
//! | `GET` | `/api/users/{id}` | lecture |
//! | `PATCH` | `/api/users/{id}` | modification partielle (mêmes champs) |
//! | `DELETE` | `/api/users/{id}` | suppression |

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use sea_orm::sea_query::{Expr, ExprTrait, Func};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use super::entities::{refresh_token, role, user, user_role};
use super::{ADMIN_ROLE, CurrentUser, InitialAdmin, password};
use crate::app::AppState;
use crate::error::{Error, FieldErrors};
use crate::query::{DEFAULT_PER_PAGE, MAX_PER_PAGE};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list).post(create))
        .route("/api/users/{id}", get(read).patch(update).delete(delete))
}

pub(crate) fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub(crate) async fn roles_of(
    db: &impl ConnectionTrait,
    user_id: i64,
) -> Result<Vec<String>, Error> {
    Ok(user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id))
        .order_by_asc(user_role::Column::Role)
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.role)
        .collect())
}

/// Représentation publique d'un compte (jamais le hash du mot de passe).
pub(crate) async fn to_json(
    db: &impl ConnectionTrait,
    account: &user::Model,
) -> Result<JsonValue, Error> {
    Ok(json!({
        "id": account.id,
        "email": account.email,
        "display_name": account.display_name,
        "active": account.active,
        "roles": roles_of(db, account.id).await?,
        "created_at": account.created_at,
        "updated_at": account.updated_at,
    }))
}

/// Change le mot de passe et ferme toutes les sessions du compte.
pub(crate) async fn set_password(
    db: &impl ConnectionTrait,
    account: user::Model,
    new: &str,
) -> Result<(), Error> {
    password::check_strength(new)?;
    let id = account.id;
    let mut record = account.into_active_model();
    record.password_hash = Set(Some(password::hash(new)?));
    record.updated_at = Set(chrono::Utc::now());
    record.update(db).await?;
    revoke_sessions(db, id).await
}

async fn revoke_sessions(db: &impl ConnectionTrait, user_id: i64) -> Result<(), Error> {
    refresh_token::Entity::delete_many()
        .filter(refresh_token::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

async fn set_roles(db: &impl ConnectionTrait, user_id: i64, roles: &[String]) -> Result<(), Error> {
    user_role::Entity::delete_many()
        .filter(user_role::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    if !roles.is_empty() {
        user_role::Entity::insert_many(roles.iter().map(|r| user_role::ActiveModel {
            user_id: Set(user_id),
            role: Set(r.clone()),
        }))
        .exec(db)
        .await?;
    }
    Ok(())
}

/// Crée en base les rôles déclarés dans le schéma qui n'y sont pas encore.
pub(crate) async fn seed_roles(db: &impl ConnectionTrait, roles: &[String]) -> Result<(), Error> {
    for name in roles {
        if role::Entity::find_by_id(name).one(db).await?.is_none() {
            role::ActiveModel {
                name: Set(name.clone()),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// Crée le compte administrateur initial si aucun utilisateur n'existe.
pub(crate) async fn ensure_initial_admin(
    db: &impl ConnectionTrait,
    admin: Option<&InitialAdmin>,
) -> Result<(), Error> {
    if user::Entity::find().count(db).await? > 0 {
        return Ok(());
    }
    let Some(admin) = admin else {
        tracing::warn!(
            "aucun utilisateur : définissez FORGE_ADMIN_EMAIL et FORGE_ADMIN_PASSWORD pour créer le compte administrateur"
        );
        return Ok(());
    };
    let account = Account {
        email: Some(admin.email.clone()),
        password: Some(admin.password.clone()),
        display_name: None,
        active: Some(true),
        roles: Some(vec![ADMIN_ROLE.to_owned()]),
    };
    let created = account.insert(db, &[ADMIN_ROLE.to_owned()]).await?;
    tracing::info!("compte administrateur initial créé : {}", created.email);
    Ok(())
}

/// Corps de création ou de modification d'un compte.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    email: Option<String>,
    password: Option<String>,
    display_name: Option<String>,
    active: Option<bool>,
    roles: Option<Vec<String>>,
}

impl Account {
    /// Valide les champs fournis ; `creating` exige email et mot de passe.
    fn validate(&mut self, declared_roles: &[String], creating: bool) -> Result<(), Error> {
        let mut errors = FieldErrors::new();
        let mut error = |field: &str, message: &str| {
            errors
                .entry(field.to_owned())
                .or_default()
                .push(message.to_owned());
        };
        match &self.email {
            Some(email) => {
                let email = normalize_email(email);
                let valid = email.len() <= 255
                    && email
                        .split_once('@')
                        .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));
                if !valid {
                    error("email", "adresse e-mail invalide");
                }
                self.email = Some(email);
            }
            None if creating => error("email", "valeur obligatoire"),
            None => {}
        }
        match &self.password {
            Some(p) if p.chars().count() < password::MIN_LENGTH => {
                error(
                    "password",
                    &format!("{} caractères au minimum", password::MIN_LENGTH),
                );
            }
            None if creating => error("password", "valeur obligatoire"),
            _ => {}
        }
        if let Some(roles) = &mut self.roles {
            roles.sort();
            roles.dedup();
            if let Some(unknown) = roles.iter().find(|r| !declared_roles.contains(r)) {
                error(
                    "roles",
                    &format!("rôle `{unknown}` non déclaré dans le schéma"),
                );
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Validation(errors))
        }
    }

    async fn insert(
        mut self,
        db: &impl ConnectionTrait,
        declared_roles: &[String],
    ) -> Result<user::Model, Error> {
        self.validate(declared_roles, true)?;
        let now = chrono::Utc::now();
        let password = self.password.as_deref().expect("validé");
        let created = user::ActiveModel {
            email: Set(self.email.clone().expect("validé")),
            password_hash: Set(Some(password::hash(password)?)),
            display_name: Set(self.display_name.clone()),
            active: Set(self.active.unwrap_or(true)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        set_roles(db, created.id, self.roles.as_deref().unwrap_or_default()).await?;
        Ok(created)
    }
}

#[derive(Deserialize)]
struct ListParams {
    page: Option<u64>,
    per_page: Option<u64>,
    q: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    current: CurrentUser,
    Query(params): Query<ListParams>,
) -> Result<Json<JsonValue>, Error> {
    current.require_admin()?;
    let page = Ord::max(params.page.unwrap_or(1), 1);
    let per_page = params
        .per_page
        .unwrap_or(DEFAULT_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);
    let mut select = user::Entity::find().order_by_asc(user::Column::Id);
    if let Some(q) = params.q.filter(|q| !q.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim().to_lowercase());
        let lower = |c: user::Column| Expr::expr(Func::lower(Expr::col(c)));
        select = select.filter(
            Condition::any()
                .add(lower(user::Column::Email).like(pattern.clone()))
                .add(lower(user::Column::DisplayName).like(pattern)),
        );
    }
    let total = select.clone().count(&state.db).await?;
    let accounts = select
        .offset((page - 1) * per_page)
        .limit(per_page)
        .all(&state.db)
        .await?;
    let mut data = Vec::with_capacity(accounts.len());
    for account in &accounts {
        data.push(to_json(&state.db, account).await?);
    }
    Ok(Json(
        json!({ "data": data, "page": page, "per_page": per_page, "total": total }),
    ))
}

async fn find(db: &impl ConnectionTrait, id: i64) -> Result<user::Model, Error> {
    user::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(Error::NotFound)
}

async fn read(
    State(state): State<AppState>,
    current: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<JsonValue>, Error> {
    current.require_admin()?;
    Ok(Json(to_json(&state.db, &find(&state.db, id).await?).await?))
}

async fn create(
    State(state): State<AppState>,
    current: CurrentUser,
    Json(account): Json<Account>,
) -> Result<(StatusCode, Json<JsonValue>), Error> {
    current.require_admin()?;
    let txn = state.db.begin().await?;
    let created = account.insert(&txn, &state.model.spec().roles).await?;
    let json = to_json(&txn, &created).await?;
    txn.commit().await?;
    Ok((StatusCode::CREATED, Json(json)))
}

async fn update(
    State(state): State<AppState>,
    current: CurrentUser,
    Path(id): Path<i64>,
    Json(mut account): Json<Account>,
) -> Result<Json<JsonValue>, Error> {
    current.require_admin()?;
    account.validate(&state.model.spec().roles, false)?;
    if id == current.id {
        let demoted = account
            .roles
            .as_ref()
            .is_some_and(|r| !r.iter().any(|r| r == ADMIN_ROLE));
        if demoted || account.active == Some(false) {
            return Err(Error::Conflict(
                "un administrateur ne peut ni se désactiver ni se retirer le rôle admin".into(),
            ));
        }
    }
    let txn = state.db.begin().await?;
    let existing = find(&txn, id).await?;
    if let Some(password) = &account.password {
        set_password(&txn, existing.clone(), password).await?;
    }
    let mut record = find(&txn, id).await?.into_active_model();
    if let Some(email) = account.email {
        record.email = Set(email);
    }
    if let Some(name) = account.display_name {
        record.display_name = Set(Some(name));
    }
    if let Some(active) = account.active {
        record.active = Set(active);
        if !active {
            revoke_sessions(&txn, id).await?;
        }
    }
    record.updated_at = Set(chrono::Utc::now());
    let updated = record.update(&txn).await?;
    if let Some(roles) = &account.roles {
        set_roles(&txn, id, roles).await?;
    }
    let json = to_json(&txn, &updated).await?;
    txn.commit().await?;
    Ok(Json(json))
}

async fn delete(
    State(state): State<AppState>,
    current: CurrentUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, Error> {
    current.require_admin()?;
    if id == current.id {
        return Err(Error::Conflict(
            "impossible de supprimer son propre compte".into(),
        ));
    }
    let account = find(&state.db, id).await?;
    user::Entity::delete_by_id(account.id)
        .exec(&state.db)
        .await?;
    // `owner` des enregistrements créés par ce compte passe à `null`.
    state.invalidate(&["users"]).await;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles() -> Vec<String> {
        vec!["admin".into(), "commercial".into()]
    }

    #[test]
    fn creation_requires_valid_fields() {
        let mut account = Account::default();
        let Err(Error::Validation(fields)) = account.validate(&roles(), true) else {
            panic!("validation attendue");
        };
        assert_eq!(fields.keys().collect::<Vec<_>>(), ["email", "password"]);

        let mut account = Account {
            email: Some(" Alice@Exemple.FR ".into()),
            password: Some("assez long".into()),
            roles: Some(vec!["commercial".into(), "commercial".into()]),
            ..Account::default()
        };
        account.validate(&roles(), true).unwrap();
        assert_eq!(account.email.as_deref(), Some("alice@exemple.fr"));
        assert_eq!(account.roles, Some(vec!["commercial".into()]));
    }

    #[test]
    fn invalid_values_are_reported() {
        let mut account = Account {
            email: Some("pas-une-adresse".into()),
            password: Some("court".into()),
            roles: Some(vec!["pirate".into()]),
            ..Account::default()
        };
        let Err(Error::Validation(fields)) = account.validate(&roles(), false) else {
            panic!("validation attendue");
        };
        assert_eq!(
            fields.keys().collect::<Vec<_>>(),
            ["email", "password", "roles"]
        );
    }
}
