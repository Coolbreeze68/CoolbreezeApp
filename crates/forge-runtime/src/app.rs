//! Assemblage d'une application : schéma, ressources, routes personnalisées.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use forge_schema::Model;
use sea_orm::DatabaseConnection;

use crate::error::Error;
use crate::hooks::Hooks;
use crate::parameters;
use crate::resource::{self, ForgeEntity};

/// État partagé par tous les handlers, y compris les routes personnalisées.
#[derive(Debug, Clone)]
pub struct AppState {
    pub(crate) db: DatabaseConnection,
    pub(crate) model: Arc<Model>,
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
}

impl App {
    /// `schema` : contenu de `forge.json`, embarqué par le code généré.
    pub fn new(schema: &str) -> Result<Self, Error> {
        Ok(Self {
            model: Arc::new(Model::from_json(schema)?),
            tables: BTreeSet::new(),
            router: parameters::router(),
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

    /// Ajoute des routes personnalisées.
    #[must_use]
    pub fn routes(mut self, router: Router<AppState>) -> Self {
        self.router = self.router.merge(router);
        self
    }

    /// Prépare la base (paramètres) et retourne le routeur prêt à servir.
    ///
    /// Échoue si une table du schéma n'a pas de ressource : le code généré est
    /// alors en retard sur `forge.json`.
    pub async fn into_router(self, db: DatabaseConnection) -> Result<Router, Error> {
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
        parameters::seed(&db, &self.model.spec().parameters).await?;
        Ok(self.router.with_state(AppState {
            db,
            model: self.model,
        }))
    }
}
