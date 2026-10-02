//! NE PAS MODIFIER : généré par forge depuis `forge.json` (table `activite`).

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "activite")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub sujet: String,
    pub nature: String,
    pub debut: DateTimeUtc,
    pub duree: Option<i64>,
    pub opportunite: Option<i64>,
    pub contact: Option<i64>,
    pub terminee: Option<bool>,
    #[sea_orm(column_type = "Text")]
    pub compte_rendu: Option<String>,
    pub owner: Option<i64>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
