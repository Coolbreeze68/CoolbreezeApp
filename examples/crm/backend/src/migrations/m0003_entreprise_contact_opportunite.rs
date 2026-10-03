//! Migration `m0003_entreprise_contact_opportunite`, générée par forge :
//!
//! - `entreprise` : nouvelle colonne `logo`
//! - `entreprise` : nouvelle colonne `satisfaction`
//! - `entreprise` : nouvelle colonne `presentation`
//! - `contact` : nouvelle colonne `photo`
//! - `opportunite` : nouvelle colonne `devis`
//!
//! Créée une seule fois : vous pouvez la compléter (données, index…), mais ne la
//! modifiez plus une fois appliquée.
//!
//! Les appels `redefine` décrivent les tables modifiées dans leur état final ;
//! seul SQLite s'en sert, pour reconstruire ces tables en conservant les données.

use forge_runtime::migration::{OnDelete, Plan};
use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m0003_entreprise_contact_opportunite"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .add_column("entreprise", string_null("logo"))
            .add_column("entreprise", big_integer_null("satisfaction"))
            .add_column("entreprise", text_null("presentation"))
            .add_column("contact", string_null("photo"))
            .add_column("opportunite", string_null("devis"))
            .redefine("entreprise", |t| {
                t.col(string("nom"));
                t.col(string_null("secteur"));
                t.col(string_null("ville"));
                t.col(string_null("site_web"));
                t.col(string_null("logo"));
                t.col(big_integer_null("satisfaction"));
                t.col(decimal_len_null("chiffre_affaires", 19, 4));
                t.col(text_null("presentation"));
                t.col(decimal_len_null("pipeline", 19, 4));
                t.unique("nom");
            })
            .redefine("contact", |t| {
                t.col(string_null("prenom"));
                t.col(string("nom"));
                t.col(string_null("email"));
                t.col(string_null("telephone"));
                t.col(string_null("photo"));
                t.col(string_null("poste"));
                t.col(big_integer_null("entreprise"));
                t.col(text_null("notes"));
                t.unique("email");
                t.reference("entreprise", "entreprise", OnDelete::SetNull);
            })
            .redefine("opportunite", |t| {
                t.col(string("titre"));
                t.col(big_integer("entreprise"));
                t.col(big_integer_null("contact"));
                t.col(decimal_len("montant", 19, 4));
                t.col(big_integer_null("probabilite").default(50));
                t.col(decimal_len_null("montant_pondere", 19, 4));
                t.col(decimal_len_null("montant_ttc", 19, 4));
                t.col(string_null("etape").default("prospect"));
                t.col(date_null("date_cloture"));
                t.col(string_null("devis"));
                t.col(text_null("notes_internes"));
                t.reference("entreprise", "entreprise", OnDelete::Restrict);
                t.reference("contact", "contact", OnDelete::SetNull);
            })
            .apply(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .drop_column("entreprise", "logo")
            .drop_column("entreprise", "satisfaction")
            .drop_column("entreprise", "presentation")
            .drop_column("contact", "photo")
            .drop_column("opportunite", "devis")
            .redefine("entreprise", |t| {
                t.col(string("nom"));
                t.col(string_null("secteur"));
                t.col(string_null("ville"));
                t.col(string_null("site_web"));
                t.col(decimal_len_null("chiffre_affaires", 19, 4));
                t.col(decimal_len_null("pipeline", 19, 4));
                t.unique("nom");
            })
            .redefine("contact", |t| {
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
            .redefine("opportunite", |t| {
                t.col(string("titre"));
                t.col(big_integer("entreprise"));
                t.col(big_integer_null("contact"));
                t.col(decimal_len("montant", 19, 4));
                t.col(big_integer_null("probabilite"));
                t.col(decimal_len_null("montant_pondere", 19, 4));
                t.col(decimal_len_null("montant_ttc", 19, 4));
                t.col(string_null("etape"));
                t.col(date_null("date_cloture"));
                t.col(text_null("notes_internes"));
                t.reference("entreprise", "entreprise", OnDelete::Restrict);
                t.reference("contact", "contact", OnDelete::SetNull);
            })
            .apply(manager)
            .await
    }
}
