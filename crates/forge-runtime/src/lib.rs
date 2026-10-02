//! Runtime des applications générées par forge.
//!
//! Le code généré se limite aux déclarations (entités, migrations, branchement) ;
//! toute la logique est ici, pilotée par le schéma embarqué : une correction du
//! runtime profite à toutes les applications sans régénération.

mod app;
pub mod auth;
pub mod cli;
mod columns;
mod error;
pub mod hooks;
mod links;
pub mod migration;
mod parameters;
mod payload;
mod query;
mod resource;
mod rules;
#[cfg(feature = "testing")]
pub mod testing;
mod values;

pub use app::{App, AppState};
pub use auth::{AuthConfig, CurrentUser};
pub use error::{Error, FieldErrors};
pub use hooks::{HookContext, Hooks};
pub use resource::ForgeEntity;
