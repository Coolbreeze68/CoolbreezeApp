//! Assemblage d'une application : schéma, ressources, routes et résolveurs personnalisés.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_graphql::dynamic::{Field, Type};
use axum::{Router, middleware};
use forge_formula::{FunctionRegistry, Value};
use forge_schema::Model;
use forge_schema::spec::Table;
use sea_orm::DatabaseConnection;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::{self, AuthConfig};
use crate::cache::{Cache, MemoryCache, Reads};
use crate::error::Error;
use crate::files::{DiskStorage, Storage};
use crate::graphql::{self, Extensions};
use crate::hooks::{Hooks, Written};
use crate::resource::{ForgeEntity, Resource, Service};
use crate::{files, observability, openapi, parameters, rest};

/// Cache par défaut : en mémoire, entrées valables 60 secondes.
pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(60);
/// Nombre maximal d'entrées du cache en mémoire par défaut.
pub const DEFAULT_CACHE_CAPACITY: u64 = 10_000;
/// Dossier des fichiers téléversés par défaut.
pub const DEFAULT_UPLOAD_DIR: &str = "uploads";

/// État partagé par tous les handlers, y compris les routes personnalisées.
#[derive(Debug, Clone)]
pub struct AppState {
    pub(crate) db: DatabaseConnection,
    pub(crate) model: Arc<Model>,
    pub(crate) auth: Arc<AuthConfig>,
    /// Fonctions de formule, implémentations comprises.
    pub(crate) functions: Arc<FunctionRegistry>,
    /// Cache des lectures, s'il est activé.
    pub(crate) reads: Option<Arc<Reads>>,
    /// Contenu des fichiers téléversés.
    pub(crate) storage: Arc<dyn Storage>,
}

impl AppState {
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn schema(&self) -> &Model {
        &self.model
    }

    /// Invalide les lectures en cache qui dépendent de `tables`. À appeler après
    /// une écriture faite hors de forge (route personnalisée), une fois validée.
    pub async fn invalidate(&self, tables: &[&str]) {
        if let Some(reads) = &self.reads {
            reads
                .invalidate(tables.iter().map(|t| (*t).to_owned()).collect())
                .await;
        }
    }

    /// Lecture de `table` via le cache, s'il est activé (voir [`crate::cache`]).
    pub(crate) async fn cached<T, F, Fut>(
        &self,
        table: &str,
        request: &str,
        load: F,
    ) -> Result<T, Error>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        match &self.reads {
            Some(reads) => reads.get_or_load(table, request, load).await,
            None => load().await,
        }
    }

    /// Applique les effets d'une écriture, après validation de sa transaction :
    /// invalidation du cache, suppression des fichiers détachés.
    pub(crate) async fn committed(&self, written: Written) {
        let (tables, files) = written.into_parts();
        if let Some(reads) = &self.reads {
            reads.invalidate(tables).await;
        }
        files::remove(self, &files).await;
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
    cache: Option<Arc<dyn Cache>>,
    storage: Arc<dyn Storage>,
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
            cache: Some(Arc::new(MemoryCache::new(
                DEFAULT_CACHE_TTL,
                DEFAULT_CACHE_CAPACITY,
            ))),
            storage: Arc::new(DiskStorage::new(DEFAULT_UPLOAD_DIR)),
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

    /// Remplace le cache des lectures (par défaut : en mémoire, voir
    /// [`DEFAULT_CACHE_TTL`]), par exemple par un cache Redis partagé.
    #[must_use]
    pub fn cache(mut self, cache: impl Cache + 'static) -> Self {
        self.cache = Some(Arc::new(cache));
        self
    }

    /// Désactive le cache des lectures.
    #[must_use]
    pub fn without_cache(mut self) -> Self {
        self.cache = None;
        self
    }

    /// Remplace le stockage des fichiers téléversés (par défaut : sur disque,
    /// dans [`DEFAULT_UPLOAD_DIR`]).
    #[must_use]
    pub fn storage(mut self, storage: impl Storage + 'static) -> Self {
        self.storage = Arc::new(storage);
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
            .merge(openapi::router(&self.model)?)
            .merge(files::router(&self.model))
            .merge(observability::router());
        for service in services.values() {
            router = router.merge(rest::router(service));
        }
        let reads = self
            .cache
            .map(|cache| Arc::new(Reads::new(cache, &self.model)));
        let state = AppState {
            db,
            model: self.model,
            auth: Arc::new(auth),
            functions: Arc::new(self.functions),
            reads,
            storage: self.storage,
        };
        let router = router
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth::authenticate,
            ))
            .with_state(state);
        Ok(observability::layer(router))
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
