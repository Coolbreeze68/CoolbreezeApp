//! NE PAS MODIFIER : généré par forge depuis `forge.json` (table `opportunite`).

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "opportunite")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub titre: String,
    pub entreprise: i64,
    pub contact: Option<i64>,
    pub montant: Decimal,
    pub probabilite: Option<i64>,
    pub montant_pondere: Option<Decimal>,
    pub etape: Option<String>,
    pub date_cloture: Option<Date>,
    #[sea_orm(column_type = "Text")]
    pub notes_internes: Option<String>,
    pub owner: Option<i64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
