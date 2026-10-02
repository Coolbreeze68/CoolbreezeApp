//! Migration `m0001_init`, générée par forge :
//!
//! - nouvelle table `entreprise`
//! - nouvelle table `contact`
//! - nouvelle table `opportunite`
//! - nouvelle table `activite`
//! - nouvelle table `tag`
//! - nouvelle table de jointure `opportunite_tags`
//!
//! Créée une seule fois : vous pouvez la compléter (données, index…), mais ne la
//! modifiez plus une fois appliquée. Les appels `redefine` décrivent les tables
//! modifiées ; seul SQLite s'en sert, pour reconstruire ces tables.

use forge_runtime::migration::{OnDelete, Plan};
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
                t.col(string("nom"));
                t.col(string_null("secteur"));
                t.col(string_null("ville"));
                t.col(string_null("site_web"));
                t.col(decimal_len_null("chiffre_affaires", 19, 4));
                t.col(decimal_len_null("pipeline", 19, 4));
                t.unique("nom");
            })
            .table("contact", |t| {
                t.col(string_null("prenom"));
                t.col(string("nom"));
                t.col(string_null("email"));
                t.col(string_null("telephone"));
                t.col(string_null("poste"));
                t.col(big_integer_null("entreprise"));
                t.col(text_null("notes"));
                t.unique("email");
                t.reference("entreprise", "entreprise", OnDelete::SetNull);
            })
            .table("opportunite", |t| {
                t.col(string("titre"));
                t.col(big_integer("entreprise"));
                t.col(big_integer_null("contact"));
                t.col(decimal_len("montant", 19, 4));
                t.col(big_integer_null("probabilite").default(50));
                t.col(decimal_len_null("montant_pondere", 19, 4));
                t.col(string_null("etape").default("prospect"));
                t.col(date_null("date_cloture"));
                t.col(text_null("notes_internes"));
                t.reference("entreprise", "entreprise", OnDelete::Restrict);
                t.reference("contact", "contact", OnDelete::SetNull);
            })
            .table("activite", |t| {
                t.col(string("sujet"));
                t.col(string("nature"));
                t.col(timestamp_with_time_zone("debut"));
                t.col(big_integer_null("duree").default(1800));
                t.col(big_integer_null("opportunite"));
                t.col(big_integer_null("contact"));
                t.col(boolean_null("terminee").default(false));
                t.col(text_null("compte_rendu"));
                t.reference("opportunite", "opportunite", OnDelete::SetNull);
                t.reference("contact", "contact", OnDelete::SetNull);
            })
            .table("tag", |t| {
                t.col(string("nom"));
                t.col(string_null("couleur").default("#607D8B"));
                t.unique("nom");
            })
            .join_table("opportunite_tags", "opportunite", "tag")
            .apply(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .drop_table("entreprise")
            .drop_table("contact")
            .drop_table("opportunite")
            .drop_table("activite")
            .drop_table("tag")
            .drop_table("opportunite_tags")
            .apply(manager)
            .await
    }
}
