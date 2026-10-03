//! NE PAS MODIFIER : généré par forge depuis `forge.json` (table `entreprise`).

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "entreprise")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub nom: String,
    pub secteur: Option<String>,
    pub ville: Option<String>,
    pub site_web: Option<String>,
    pub logo: Option<String>,
    pub satisfaction: Option<i64>,
    pub chiffre_affaires: Option<Decimal>,
    #[sea_orm(column_type = "Text")]
    pub presentation: Option<String>,
    pub pipeline: Option<Decimal>,
    pub owner: Option<i64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
