//! API GraphQL, construite au démarrage à partir du schéma (`async-graphql`, schéma dynamique).
//!
//! | Méthode | Chemin | Effet |
//! |---|---|---|
//! | `POST` | `/api/graphql` | requêtes et mutations (authentifiées comme le reste de `/api/`) |
//! | `GET` | `/graphql` | GraphiQL, l'éditeur interactif |
//!
//! Les opérations et leurs noms sont décrits dans [`forge_schema::graphql`].
//! Les résolveurs personnalisés s'ajoutent par [`crate::App::graphql_query`] et
//! [`crate::App::graphql_mutation`] ; ils lisent l'état et l'utilisateur par
//! `ctx.data::<AppState>()` et `ctx.data::<CurrentUser>()`.

mod schema;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use async_graphql::ErrorExtensions;
use async_graphql::dataloader::{DataLoader, Loader};
use async_graphql::dynamic::{Field, Schema, Type};
use async_graphql::http::GraphiQLSource;
use axum::extract::State;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use forge_schema::Model;

use crate::app::AppState;
use crate::auth::CurrentUser;
use crate::error::Error;
use crate::resource::Service;

type Services = BTreeMap<String, Arc<dyn Service>>;

/// Champs et types GraphQL ajoutés par le code utilisateur.
#[derive(Default)]
pub(crate) struct Extensions {
    pub queries: Vec<Field>,
    pub mutations: Vec<Field>,
    pub types: Vec<Type>,
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extensions")
            .field("queries", &self.queries.len())
            .field("mutations", &self.mutations.len())
            .field("types", &self.types.len())
            .finish()
    }
}

/// Routes GraphQL : le schéma est construit une fois, au démarrage.
pub(crate) fn router(
    model: &Model,
    services: &Arc<Services>,
    extensions: Extensions,
) -> Result<Router<AppState>, Error> {
    let schema = schema::build(model, services, extensions)?;
    let services = services.clone();
    Ok(Router::new()
        .route(
            "/api/graphql",
            post(move |state, user, request| execute(schema, services, state, user, request)),
        )
        .route("/graphql", get(graphiql)))
}

async fn execute(
    schema: Schema,
    services: Arc<Services>,
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    // Un chargeur par requête : les références sont lues par lots, avec les
    // droits de l'utilisateur.
    let loader = DataLoader::new(
        RecordLoader {
            state: state.clone(),
            user: user.clone(),
            services,
        },
        tokio::spawn,
    );
    Json(
        schema
            .execute(request.data(state).data(user).data(loader))
            .await,
    )
}

async fn graphiql() -> Html<String> {
    Html(
        GraphiQLSource::build()
            .endpoint("/api/graphql")
            .title("GraphQL")
            .finish(),
    )
}

/// Erreur du runtime → erreur GraphQL, avec `code` (et `fields`) en extensions.
pub(crate) fn to_graphql(err: &Error) -> async_graphql::Error {
    let body = err.body();
    let message = body["message"].as_str().unwrap_or_default().to_owned();
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", body["code"].as_str().unwrap_or_default());
        if let Ok(fields) = async_graphql::Value::from_json(body["fields"].clone())
            && fields != async_graphql::Value::Null
        {
            extensions.set("fields", fields);
        }
    })
}

/// Charge par lots les enregistrements cibles des références, table par table.
pub(crate) struct RecordLoader {
    state: AppState,
    user: CurrentUser,
    services: Arc<Services>,
}

impl Loader<(String, i64)> for RecordLoader {
    type Value = serde_json::Value;
    type Error = Arc<Error>;

    async fn load(
        &self,
        keys: &[(String, i64)],
    ) -> Result<HashMap<(String, i64), Self::Value>, Self::Error> {
        let mut by_table: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (table, id) in keys {
            by_table.entry(table).or_default().push(*id);
        }
        let mut found = HashMap::new();
        for (table, ids) in by_table {
            let service = &self.services[table];
            let records = match service.read_many(&self.state, &self.user, &ids).await {
                Ok(records) => records,
                // Table illisible pour l'utilisateur : les références restent `null`.
                Err(Error::Forbidden(_)) => continue,
                Err(err) => return Err(Arc::new(err)),
            };
            for record in records {
                if let Some(id) = record["id"].as_i64() {
                    found.insert((table.to_owned(), id), record);
                }
            }
        }
        Ok(found)
    }
}
