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
    /// Interface(s) générée(s) : `"flutter"` (web et mobile, par défaut),
    /// `"web"` (React), ou les deux : `["flutter", "web"]`.
    #[serde(default)]
    pub frontend: Frontends,
    /// Devise par défaut des colonnes `money` (code ISO 4217, `EUR` par défaut).
    #[serde(default)]
    pub currency: Option<String>,
}

impl App {
    pub const DEFAULT_CURRENCY: &str = "EUR";

    pub fn currency(&self) -> &str {
        self.currency.as_deref().unwrap_or(Self::DEFAULT_CURRENCY)
    }
}

/// Interface utilisateur générée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Frontend {
    /// Application Flutter (`app/`) : web, Android, iOS.
    Flutter,
    /// Application web React (`web/`).
    Web,
}

/// Une interface, ou plusieurs.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Frontends {
    One(Frontend),
    Many(Vec<Frontend>),
}

impl Default for Frontends {
    fn default() -> Self {
        Self::One(Frontend::Flutter)
    }
}

impl Frontends {
    pub fn list(&self) -> &[Frontend] {
        match self {
            Self::One(frontend) => std::slice::from_ref(frontend),
            Self::Many(list) => list,
        }
    }

    pub fn has(&self, frontend: Frontend) -> bool {
        self.list().contains(&frontend)
    }
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
    /// Couleur `#rrggbb` (sélecteur de couleur).
    Color,
    /// Adresse e-mail.
    Email,
    /// Adresse web `http(s)://`.
    Url,
    /// Numéro de téléphone.
    Phone,
    /// Texte long mis en forme en Markdown.
    Markdown,
    /// Note entière de 0 à `max` (5 par défaut), affichée en étoiles.
    Rating,
    /// Pourcentage, stocké en proportion (`0.25` pour 25 %).
    Percent,
    /// Montant dans la devise `currency`.
    Money,
    /// Fichier téléversé (taille limitée par `max_size`, types par `accept`).
    File,
    /// Image téléversée (PNG, JPEG, GIF ou WebP).
    Image,
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
            Self::Color => "color",
            Self::Email => "email",
            Self::Url => "url",
            Self::Phone => "phone",
            Self::Markdown => "markdown",
            Self::Rating => "rating",
            Self::Percent => "percent",
            Self::Money => "money",
            Self::File => "file",
            Self::Image => "image",
        }
    }

    /// Type de base, qui décide du stockage, des filtres et du type en formule :
    /// un modèle de champ (`color`, `money`…) est un type de base avec une
    /// validation et une présentation propres.
    #[must_use]
    pub fn base(self) -> Self {
        match self {
            Self::Color | Self::Email | Self::Url | Self::Phone | Self::File | Self::Image => {
                Self::String
            }
            Self::Markdown => Self::Text,
            Self::Rating => Self::Integer,
            Self::Percent | Self::Money => Self::Decimal,
            other => other,
        }
    }

    /// Fichier téléversé (`file` ou `image`) : la colonne stocke son identifiant.
    pub fn is_file(self) -> bool {
        matches!(self, Self::File | Self::Image)
    }

    pub fn is_numeric(self) -> bool {
        matches!(self.base(), Self::Integer | Self::Decimal | Self::Duration)
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
            model => model.base().formula_type(),
        }
    }

    pub fn is_scalar(self) -> bool {
        !matches!(
            self,
            Self::Enum | Self::Reference | Self::ReferenceList | Self::Lookup
        ) && !self.is_file()
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
    /// `rating` : note maximale (1 à 10, 5 par défaut).
    #[serde(default)]
    pub max: Option<u32>,
    /// `money` : devise (code ISO 4217), par défaut `app.currency`.
    #[serde(default)]
    pub currency: Option<String>,
    /// `file` / `image` : taille maximale en Mo (10 par défaut).
    #[serde(default)]
    pub max_size: Option<u32>,
    /// `file` : types acceptés, MIME (`application/pdf`, `image/*`) ou extensions (`.csv`).
    #[serde(default)]
    pub accept: Option<Vec<String>>,
    /// Ancien nom de la colonne, pour une migration par renommage sans perte.
    #[serde(default)]
    pub renamed_from: Option<String>,
}

impl Column {
    pub const DEFAULT_RATING_MAX: u32 = 5;
    pub const DEFAULT_MAX_SIZE_MB: u32 = 10;

    /// `rating` : note maximale.
    pub fn rating_max(&self) -> u32 {
        self.max.unwrap_or(Self::DEFAULT_RATING_MAX)
    }

    /// `file` / `image` : taille maximale en octets.
    pub fn max_size_bytes(&self) -> u64 {
        u64::from(self.max_size.unwrap_or(Self::DEFAULT_MAX_SIZE_MB)) * 1024 * 1024
    }

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
