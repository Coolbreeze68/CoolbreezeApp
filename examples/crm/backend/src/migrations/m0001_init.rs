//! Migration initiale : création des tables du schéma.
//!
//! Générée une seule fois par forge ; vous pouvez la compléter (données
//! initiales, index…). Ne la modifiez plus une fois appliquée en production.

use forge_runtime::migration::{OnDelete, Plan, drop_tables};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .table("entreprise", |t| {
                t.col(string("nom").unique_key());
                t.col(string_null("secteur"));
                t.col(string_null("ville"));
                t.col(string_null("site_web"));
                t.col(decimal_len_null("chiffre_affaires", 19, 4));
                t.col(decimal_len_null("pipeline", 19, 4));
            })
            .table("contact", |t| {
                t.col(string_null("prenom"));
                t.col(string("nom"));
                t.col(string_null("email").unique_key());
                t.col(string_null("telephone"));
                t.col(string_null("poste"));
                t.col(big_integer_null("entreprise"));
                t.col(text_null("notes"));
            })
            .table("opportunite", |t| {
                t.col(string("titre"));
                t.col(big_integer("entreprise"));
                t.col(big_integer_null("contact"));
                t.col(decimal_len("montant", 19, 4));
                t.col(big_integer_null("probabilite"));
                t.col(decimal_len_null("montant_pondere", 19, 4));
                t.col(string_null("etape"));
                t.col(date_null("date_cloture"));
                t.col(text_null("notes_internes"));
            })
            .table("activite", |t| {
                t.col(string("sujet"));
                t.col(string("nature"));
                t.col(timestamp_with_time_zone("debut"));
                t.col(big_integer_null("duree"));
                t.col(big_integer_null("opportunite"));
                t.col(big_integer_null("contact"));
                t.col(boolean_null("terminee"));
                t.col(text_null("compte_rendu"));
            })
            .table("tag", |t| {
                t.col(string("nom").unique_key());
                t.col(string_null("couleur"));
            })
            .reference("contact", "entreprise", "entreprise", OnDelete::SetNull)
            .reference(
                "opportunite",
                "entreprise",
                "entreprise",
                OnDelete::Restrict,
            )
            .reference("opportunite", "contact", "contact", OnDelete::SetNull)
            .reference("activite", "opportunite", "opportunite", OnDelete::SetNull)
            .reference("activite", "contact", "contact", OnDelete::SetNull)
            .join_table("opportunite_tags", "opportunite", "tag")
            .create(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_tables(
            manager,
            &[
                "entreprise",
                "contact",
                "opportunite",
                "activite",
                "tag",
                "opportunite_tags",
            ],
        )
        .await
    }
}
