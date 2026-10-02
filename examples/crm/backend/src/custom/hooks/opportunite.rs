//! Hooks de la table `opportunite`. Ce fichier vous appartient : forge ne le modifie jamais.

use forge_runtime::{Error, HookContext, Hooks};
use sea_orm::prelude::Decimal;

use crate::generated::entities::opportunite::{ActiveModel, Entity};

/// Créé par `Default` à chaque démarrage : ajoutez-y des champs si besoin.
#[derive(Debug, Default)]
pub struct OpportuniteHooks;

impl Hooks<Entity> for OpportuniteHooks {
    /// Exemple de règle métier : le montant doit être strictement positif.
    async fn validate(&self, _ctx: &HookContext<'_>, record: &ActiveModel) -> Result<(), Error> {
        match record.montant.try_as_ref() {
            Some(montant) if *montant <= Decimal::ZERO => Err(Error::validation(
                "montant",
                "le montant doit être strictement positif",
            )),
            _ => Ok(()),
        }
    }
}
