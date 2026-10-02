//! Types du fichier d'entrée `forge.json`, tels qu'écrits par l'utilisateur.
//!
//! Ces types décrivent la *forme* du fichier (et produisent `forge.schema.json`).
//! La cohérence (références, formules, options compatibles…) est vérifiée par
//! [`crate::Model::from_spec`].

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

/// Description complète d'une application forge.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// Chemin vers `forge.schema.json`, pour l'autocomplétion dans l'éditeur.
    #[serde(rename = "$schema", default)]
    pub json_schema: Option<String>,
    pub app: App,
    /// Rôles utilisateurs. Le rôle `admin` est obligatoire.
    pub roles: Vec<String>,
    /// Paramètres globaux, éditables par l'admin et lisibles dans les formules via `$param.nom`.
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    /// Fonctions de formule personnalisées, implémentées dans `src/custom/functions.rs`.
    #[serde(default)]
    pub functions: Vec<FunctionDecl>,
    pub tables: Vec<Table>,
}

/// Fonction de formule personnalisée : signature vérifiée par forge,
/// implémentation fournie par l'application.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FunctionDecl {
    /// Nom, en majuscules par convention (`TVA`).
    pub name: String,
    /// Type de chaque argument.
    pub args: Vec<FormulaType>,
    /// Type du résultat.
    pub returns: FormulaType,
    /// Le résultat dépend d'autre chose que des arguments (date du jour, service
    /// externe…) : la fonction est alors interdite dans les formules persistées.
    #[serde(default)]
    pub volatile: bool,
}

/// Type d'une valeur de formule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FormulaType {
    /// Entier, décimal ou durée.
    Number,
    Text,
    Boolean,
    Date,
    Datetime,
    /// N'importe quel type.
    Any,
}

impl From<FormulaType> for forge_formula::Type {
    fn from(ty: FormulaType) -> Self {
        match ty {
            FormulaType::Number => Self::Number,
            FormulaType::Text => Self::Text,
            FormulaType::Boolean => Self::Boolean,
            FormulaType::Date => Self::Date,
            FormulaType::Datetime => Self::DateTime,
            FormulaType::Any => Self::Any,
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// Identifiant de l'application (`snake_case`).
    pub name: String,
    /// Langue par défaut ; doit figurer dans `locales`.
    pub default_locale: String,
    /// Langues supportées, par exemple `["fr", "en"]`.
    pub locales: Vec<String>,
}

/// Libellé affiché : texte unique ou traduction par langue.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Label {
    Plain(String),
    Localized(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColumnType,
    #[serde(default)]
    pub label: Option<Label>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Nom de la table (`snake_case`, singulier conseillé).
    pub name: String,
    #[serde(default)]
    pub label: Option<Label>,
    /// Colonnes métier. `id`, `created_at`, `updated_at` et `owner` sont ajoutées automatiquement.
    pub columns: Vec<Column>,
    #[serde(default)]
    pub views: Views,
    /// Règles d'autorisation. Sans règle, seul `admin` a accès à la table.
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    String,
    Text,
    Integer,
    Decimal,
    Boolean,
    Date,
    Datetime,
    /// Durée en secondes.
    Duration,
    /// Valeur parmi `values`.
    Enum,
    /// Relation N→1 vers `target`.
    Reference,
    /// Relation N↔N vers `target` (table de jointure générée).
    ReferenceList,
    /// Valeur lue via un chemin de références (`path`), en lecture seule.
    Lookup,
}

impl ColumnType {
    pub fn name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::Duration => "duration",
            Self::Enum => "enum",
            Self::Reference => "reference",
            Self::ReferenceList => "reference_list",
            Self::Lookup => "lookup",
        }
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Integer | Self::Decimal | Self::Duration)
    }

    pub fn is_temporal(self) -> bool {
        matches!(self, Self::Date | Self::Datetime)
    }

    /// Type stockant une valeur simple (ni relation, ni lookup).
    /// Type des valeurs de ce type de colonne dans une formule (`Any` pour un lookup,
    /// dont le type est celui de la colonne visée).
    pub fn formula_type(self) -> forge_formula::Type {
        use forge_formula::Type;
        match self {
            Self::String | Self::Text | Self::Enum => Type::Text,
            Self::Integer | Self::Decimal | Self::Duration | Self::Reference => Type::Number,
            Self::Boolean => Type::Boolean,
            Self::Date => Type::Date,
            Self::Datetime => Type::DateTime,
            Self::ReferenceList | Self::Lookup => Type::Any,
        }
    }

    pub fn is_scalar(self) -> bool {
        !matches!(
            self,
            Self::Enum | Self::Reference | Self::ReferenceList | Self::Lookup
        )
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "reflète les options booléennes du format JSON"
)]
pub struct Column {
    /// Nom de la colonne (`snake_case`).
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColumnType,
    #[serde(default)]
    pub label: Option<Label>,
    /// Valeur obligatoire.
    #[serde(default)]
    pub required: bool,
    /// Valeur unique dans la table.
    #[serde(default)]
    pub unique: bool,
    /// Absente des vues ; reste accessible via l'API selon les règles.
    #[serde(default)]
    pub hidden: bool,
    /// Compose l'intitulé du record (plusieurs colonnes possibles, dans l'ordre).
    #[serde(default)]
    pub title_field: bool,
    /// Valeur par défaut, du type de la colonne.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Formule de calcul, par exemple `montant * probabilite / 100`.
    #[serde(default)]
    pub formula: Option<String>,
    /// Avec `formula` : valeur calculée à l'écriture et stockée en base.
    #[serde(default)]
    pub persist: bool,
    /// `reference` / `reference_list` : table cible.
    #[serde(default)]
    pub target: Option<String>,
    /// `reference` / `reference_list` : nom de la relation inverse sur la table cible
    /// (par défaut, le nom de la table courante). Utilisable dans les agrégats.
    #[serde(default)]
    pub inverse: Option<String>,
    /// `enum` : valeurs autorisées (`snake_case`).
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// `lookup` : chemin de références, par exemple `entreprise.secteur`.
    #[serde(default)]
    pub path: Option<String>,
    /// Ancien nom de la colonne, pour une migration par renommage sans perte.
    #[serde(default)]
    pub renamed_from: Option<String>,
}

impl Column {
    /// Colonne calculée : formule ou lookup.
    pub fn is_computed(&self) -> bool {
        self.formula.is_some() || self.ty == ColumnType::Lookup
    }

    /// Colonne dont la valeur existe en base (donc filtrable et triable en SQL).
    pub fn is_stored(&self) -> bool {
        match self.ty {
            ColumnType::Lookup | ColumnType::ReferenceList => false,
            _ => self.formula.is_none() || self.persist,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Views {
    #[serde(default)]
    pub calendar: Option<CalendarView>,
    #[serde(default)]
    pub stats: Option<StatsView>,
}

/// Vue calendrier : un début, et une fin ou une durée.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarView {
    /// Colonne `date` ou `datetime`.
    pub start: String,
    /// Colonne de fin, du même type que `start`.
    #[serde(default)]
    pub end: Option<String>,
    /// Colonne `duration`.
    #[serde(default)]
    pub duration: Option<String>,
}

/// Vue statistiques : somme, moyenne, min, max et graphiques.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatsView {
    /// Colonnes numériques (`integer`, `decimal`, `duration`).
    pub fields: Vec<String>,
    /// Regroupement par une colonne `enum`, `boolean`, `reference` ou `string`.
    #[serde(default)]
    pub group_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub roles: Vec<String>,
    pub actions: Vec<Action>,
    /// Condition sur le record, par exemple `owner == $user.id`.
    #[serde(default)]
    pub when: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, JsonSchema)]
pub enum Action {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "delete")]
    Delete,
    /// Toutes les actions.
    #[serde(rename = "*")]
    All,
}
