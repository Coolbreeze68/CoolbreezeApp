//! Hooks de la table `activite`. Ce fichier vous appartient : forge ne le modifie jamais.

use forge_runtime::{Error, HookContext, Hooks};
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::generated::entities::activite::{Entity, Model};
use crate::generated::entities::opportunite;

/// Créé par `Default` à chaque démarrage : ajoutez-y des champs si besoin.
#[derive(Debug, Default)]
pub struct ActiviteHooks;

impl Hooks<Entity> for ActiviteHooks {
    /// Exemple d'écriture dans une autre table : une première activité sur une
    /// opportunité encore au stade `prospect` la fait passer à `proposition`.
    async fn after_create(&self, ctx: &HookContext<'_>, record: &Model) -> Result<(), Error> {
        let Some(id) = record.opportunite else {
            return Ok(());
        };
        opportunite::Entity::update_many()
            .col_expr(opportunite::Column::Etape, Expr::value("proposition"))
            .filter(opportunite::Column::Id.eq(id))
            .filter(opportunite::Column::Etape.eq("prospect"))
            .exec(ctx.db())
            .await?;
        // Écriture faite hors de forge : les lectures en cache d'`opportunite`
        // sont invalidées une fois la transaction validée.
        ctx.modified("opportunite");
        Ok(())
    }
}
