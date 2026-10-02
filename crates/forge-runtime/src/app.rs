//! Assemblage d'une application : schéma, ressources, routes personnalisées.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::{Router, middleware};
use forge_formula::{FunctionRegistry, Value};
use forge_schema::Model;
use sea_orm::DatabaseConnection;

use crate::auth::{self, AuthConfig};
use crate::error::Error;
use crate::hooks::Hooks;
use crate::parameters;
use crate::resource::{self, ForgeEntity};

/// État partagé par tous les handlers, y compris les routes personnalisées.
#[derive(Debug, Clone)]
pub struct AppState {
    pub(crate) db: DatabaseConnection,
    pub(crate) model: Arc<Model>,
    pub(crate) auth: Arc<AuthConfig>,
    /// Fonctions de formule, implémentations comprises.
    pub(crate) functions: Arc<FunctionRegistry>,
}

impl AppState {
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn schema(&self) -> &Model {
        &self.model
    }
}

/// Description d'une application, construite par le code généré :
///
/// ```ignore
/// App::new(SCHEMA)?
///     .resource::<entities::contact::Entity, hooks::contact::ContactHooks>()
///     .routes(custom::routes::routes())
/// ```
#[derive(Debug)]
pub struct App {
    model: Arc<Model>,
    tables: BTreeSet<String>,
    router: Router<AppState>,
    functions: FunctionRegistry,
    /// Erreurs d'enregistrement de fonctions, rapportées au démarrage.
    errors: Vec<String>,
}

impl App {
    /// `schema` : contenu de `forge.json`, embarqué par le code généré.
    pub fn new(schema: &str) -> Result<Self, Error> {
        let model = Model::from_json(schema)?;
        Ok(Self {
            functions: model.functions().clone(),
            errors: Vec::new(),
            model: Arc::new(model),
            tables: BTreeSet::new(),
            router: parameters::router().merge(auth::router()),
        })
    }

    pub fn schema(&self) -> &Model {
        &self.model
    }

    /// Expose les endpoints CRUD de l'entité `E`, avec les hooks `H`
    /// (créés par `H::default()`).
    #[must_use]
    pub fn resource<E: ForgeEntity, H: Hooks<E> + Default>(mut self) -> Self {
        let (table, router) = resource::router::<E, H>(H::default());
        self.tables.insert(table);
        self.router = self.router.merge(router);
        self
    }

    /// Implémente une fonction de formule déclarée dans le schéma (`functions`).
    ///
    /// ```ignore
    /// app.function("TVA", |args| match args {
    ///     [Value::Number(montant)] => Ok(Value::Number(montant * Decimal::new(2, 1))),
    ///     _ => Ok(Value::Null),
    /// })
    /// ```
    #[must_use]
    pub fn function(
        mut self,
        name: &str,
        implementation: impl Fn(&[Value]) -> Result<Value, String> + Send + Sync + 'static,
    ) -> Self {
        if let Err(message) = self.functions.implement(name, implementation) {
            self.errors.push(message);
        }
        self
    }

    /// Ajoute des routes personnalisées.
    #[must_use]
    pub fn routes(mut self, router: Router<AppState>) -> Self {
        self.router = self.router.merge(router);
        self
    }

    /// Prépare la base (paramètres, rôles, compte administrateur initial) et
    /// retourne le routeur prêt à servir, authentification comprise.
    ///
    /// Échoue si une table du schéma n'a pas de ressource : le code généré est
    /// alors en retard sur `forge.json`.
    pub async fn into_router(
        self,
        db: DatabaseConnection,
        auth: AuthConfig,
    ) -> Result<Router, Error> {
        let missing: Vec<_> = self
            .model
            .tables()
            .iter()
            .filter(|t| !self.tables.contains(&t.name))
            .map(|t| t.name.as_str())
            .collect();
        if !missing.is_empty() {
            return Err(Error::Config(format!(
                "tables sans ressource : {} (lancez `forge generate`)",
                missing.join(", ")
            )));
        }
        let mut errors = self.errors;
        errors.extend(
            self.functions
                .missing_implementations()
                .into_iter()
                .map(|name| {
                    format!(
                        "fonction `{name}` déclarée mais non implémentée (src/custom/functions.rs)"
                    )
                }),
        );
        if !errors.is_empty() {
            return Err(Error::Config(errors.join(" ; ")));
        }
        parameters::seed(&db, &self.model.spec().parameters).await?;
        auth::users::seed_roles(&db, &self.model.spec().roles).await?;
        auth::users::ensure_initial_admin(&db, auth.initial_admin.as_ref()).await?;
        let state = AppState {
            db,
            model: self.model,
            auth: Arc::new(auth),
            functions: Arc::new(self.functions),
        };
        Ok(self
            .router
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth::authenticate,
            ))
            .with_state(state))
    }
}
