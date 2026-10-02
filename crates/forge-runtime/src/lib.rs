//! Runtime des applications générées par forge.
//!
//! Le code généré se limite aux déclarations (entités, migrations, branchement) ;
//! toute la logique est ici, pilotée par le schéma embarqué : une correction du
//! runtime profite à toutes les applications sans régénération.

mod aggregate;
mod app;
pub mod auth;
pub mod cache;
pub mod cli;
mod columns;
mod compute;
mod csv_io;
mod error;
mod graphql;
pub mod hooks;
mod links;
pub mod migration;
pub mod observability;
mod openapi;
mod parameters;
mod payload;
mod query;
mod resource;
mod rest;
mod rules;
#[cfg(feature = "testing")]
pub mod testing;
mod values;

pub use app::{App, AppState, DEFAULT_CACHE_CAPACITY, DEFAULT_CACHE_TTL};

/// Valeurs et types des formules, pour implémenter des fonctions personnalisées.
/// Valeurs manipulées par les fonctions de formule personnalisées.
pub mod formula {
    pub use forge_formula::{Type, Value};
    pub use rust_decimal::Decimal;
}
pub use auth::{AuthConfig, CurrentUser};
pub use error::{Error, FieldErrors, RowError};
pub use hooks::{HookContext, Hooks};
pub use resource::ForgeEntity;

/// Types GraphQL (`async_graphql::dynamic`) pour les résolveurs personnalisés.
pub use async_graphql;
