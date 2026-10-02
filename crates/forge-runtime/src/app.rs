//! Assemblage d'une application : schéma, ressources, routes et résolveurs personnalisés.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_graphql::dynamic::{Field, Type};
use axum::{Router, middleware};
use forge_formula::{FunctionRegistry, Value};
use forge_schema::Model;
use forge_schema::spec::Table;
use sea_orm::DatabaseConnection;

use crate::auth::{self, AuthConfig};
use crate::error::Error;
use crate::graphql::{self, Extensions};
use crate::hooks::Hooks;
use crate::resource::{ForgeEntity, Resource, Service};
use crate::{openapi, parameters, rest};

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

    /// Table du schéma servie par une ressource enregistrée.
    pub(crate) fn table(&self, name: &str) -> &Table {
        self.model
            .table(name)
            .expect("ressource enregistrée à partir du schéma")
    }
}

/// Description d'une application, construite par le code généré :
///
/// ```ignore
/// App::new(SCHEMA)?
///     .resource::<entities::contact::Entity, hooks::contact::ContactHooks>()
///     .routes(custom::routes::routes())
/// ```
pub struct App {
    model: Arc<Model>,
    services: BTreeMap<String, Arc<dyn Service>>,
    router: Router<AppState>,
    graphql: Extensions,
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
            services: BTreeMap::new(),
            router: parameters::router().merge(auth::router()),
            graphql: Extensions::default(),
        })
    }

    pub fn schema(&self) -> &Model {
        &self.model
    }

    /// Expose l'entité `E` (REST, GraphQL, CSV), avec les hooks `H`
    /// (créés par `H::default()`).
    #[must_use]
    pub fn resource<E: ForgeEntity, H: Hooks<E> + Default>(mut self) -> Self {
        let service: Arc<dyn Service> = Arc::new(Resource::<E, H>::new(H::default()));
        self.services.insert(service.name().to_owned(), service);
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

    /// Ajoute un champ à la racine `Query` du schéma GraphQL. Le résolveur lit
    /// l'état et l'utilisateur par `ctx.data::<AppState>()` et `ctx.data::<CurrentUser>()`.
    #[must_use]
    pub fn graphql_query(mut self, field: Field) -> Self {
        self.graphql.queries.push(field);
        self
    }

    /// Ajoute un champ à la racine `Mutation` du schéma GraphQL.
    #[must_use]
    pub fn graphql_mutation(mut self, field: Field) -> Self {
        self.graphql.mutations.push(field);
        self
    }

    /// Déclare un type GraphQL utilisé par les champs personnalisés.
    #[must_use]
    pub fn graphql_type(mut self, ty: impl Into<Type>) -> Self {
        self.graphql.types.push(ty.into());
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
            .filter(|t| !self.services.contains_key(&t.name))
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
        let services = Arc::new(self.services);
        let mut router = self
            .router
            .merge(graphql::router(&self.model, &services, self.graphql)?)
            .merge(openapi::router(&self.model)?);
        for service in services.values() {
            router = router.merge(rest::router(service));
        }
        let state = AppState {
            db,
            model: self.model,
            auth: Arc::new(auth),
            functions: Arc::new(self.functions),
        };
        Ok(router
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth::authenticate,
            ))
            .with_state(state))
    }
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("tables", &self.services.keys().collect::<Vec<_>>())
            .field("graphql", &self.graphql)
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}
