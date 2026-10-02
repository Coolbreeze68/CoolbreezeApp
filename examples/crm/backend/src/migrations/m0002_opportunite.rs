//! Migration `m0002_opportunite`, générée par forge :
//!
//! - `opportunite` : nouvelle colonne `montant_ttc`
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
        "m0002_opportunite"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .add_column("opportunite", decimal_len_null("montant_ttc", 19, 4))
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
                t.col(text_null("notes_internes"));
                t.reference("entreprise", "entreprise", OnDelete::Restrict);
                t.reference("contact", "contact", OnDelete::SetNull);
            })
            .apply(manager)
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Plan::new()
            .drop_column("opportunite", "montant_ttc")
            .redefine("opportunite", |t| {
                t.col(string("titre"));
                t.col(big_integer("entreprise"));
                t.col(big_integer_null("contact"));
                t.col(decimal_len("montant", 19, 4));
                t.col(big_integer_null("probabilite"));
                t.col(decimal_len_null("montant_pondere", 19, 4));
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
