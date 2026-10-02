//! NE PAS MODIFIER : code généré par forge depuis `forge.json`.
//!
//! Ce dossier est réécrit à chaque `forge generate`. Votre code va dans `src/custom/`.

pub mod entities;
#[path = "../custom/functions.rs"]
pub mod functions;
pub mod hooks;
mod migrations;

pub use migrations::Migrator;

/// Schéma de l'application, tel qu'au dernier `forge generate`.
pub const SCHEMA: &str = include_str!("forge.json");

/// Application complète : une ressource par table avec ses hooks, les fonctions
/// de `src/custom/functions.rs` et les routes de `src/custom/routes.rs`.
pub fn app() -> Result<forge_runtime::App, forge_runtime::Error> {
    let app = forge_runtime::App::new(SCHEMA)?
        .resource::<entities::entreprise::Entity, hooks::entreprise::EntrepriseHooks>()
        .resource::<entities::contact::Entity, hooks::contact::ContactHooks>()
        .resource::<entities::opportunite::Entity, hooks::opportunite::OpportuniteHooks>()
        .resource::<entities::activite::Entity, hooks::activite::ActiviteHooks>()
        .resource::<entities::tag::Entity, hooks::tag::TagHooks>();
    Ok(functions::register(app).routes(crate::custom::routes::routes()))
}
