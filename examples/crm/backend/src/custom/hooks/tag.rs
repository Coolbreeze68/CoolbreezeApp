//! Hooks de la table `tag`. Ce fichier vous appartient : forge ne le modifie jamais.
//!
//! Surchargez les méthodes utiles du trait [`Hooks`], par exemple :
//!
//! ```ignore
//! impl Hooks<Entity> for TagHooks {
//!     async fn validate(&self, _ctx: &HookContext<'_>, record: &ActiveModel) -> Result<(), Error> {
//!         Ok(())
//!     }
//! }
//! ```

use forge_runtime::Hooks;

use crate::generated::entities::tag::Entity;

/// Créé par `Default` à chaque démarrage : ajoutez-y des champs si besoin.
#[derive(Debug, Default)]
pub struct TagHooks;

impl Hooks<Entity> for TagHooks {}
