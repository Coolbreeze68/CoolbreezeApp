//! NE PAS MODIFIER : généré par forge depuis `forge.json` (table `contact`).

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "contact")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub prenom: Option<String>,
    pub nom: String,
    pub email: Option<String>,
    pub telephone: Option<String>,
    pub photo: Option<String>,
    pub poste: Option<String>,
    pub entreprise: Option<i64>,
    #[sea_orm(column_type = "Text")]
    pub notes: Option<String>,
    pub owner: Option<i64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
