use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use forge_formula::{Expr, FunctionRegistry};

use crate::error::{Issue, SchemaError};
use crate::spec::{Column, ColumnType, Spec, Table};
use crate::validate;

/// Désigne une colonne : `table.colonne`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnRef {
    pub table: String,
    pub column: String,
}

impl ColumnRef {
    pub fn new(table: impl Into<String>, column: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            column: column.into(),
        }
    }
}

impl fmt::Display for ColumnRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.table, self.column)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    /// Colonne `reference` : N→1.
    ManyToOne,
    /// Colonne `reference_list` : N↔N via une table de jointure.
    ManyToMany,
}

/// Relation déclarée par une colonne `reference` ou `reference_list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    /// Colonne qui déclare la relation.
    pub source: ColumnRef,
    pub target: String,
    pub kind: RelationKind,
    /// Nom de la relation inverse, vue depuis `target` (ex. `entreprise.opportunites`).
    pub inverse: String,
}

impl Relation {
    /// Table de jointure d'une relation N↔N : `<table>_<colonne>`.
    pub fn join_table(&self) -> Option<String> {
        (self.kind == RelationKind::ManyToMany)
            .then(|| format!("{}_{}", self.source.table, self.source.column))
    }
}

/// Schéma validé : toutes les références sont résolues et les formules analysées.
#[derive(Debug, Clone)]
pub struct Model {
    pub(crate) spec: Spec,
    pub(crate) relations: Vec<Relation>,
    pub(crate) formulas: BTreeMap<ColumnRef, Expr>,
    pub(crate) lookups: BTreeMap<ColumnRef, Vec<String>>,
    pub(crate) conditions: BTreeMap<(String, usize), Expr>,
    pub(crate) computed_order: Vec<ColumnRef>,
    pub(crate) dependencies: BTreeMap<ColumnRef, BTreeSet<ColumnRef>>,
    pub(crate) functions: FunctionRegistry,
}

impl Model {
    /// Analyse et valide un document JSON.
    pub fn from_json(src: &str) -> Result<Self, SchemaError> {
        let deserializer = &mut serde_json::Deserializer::from_str(src);
        let spec: Spec = serde_path_to_error::deserialize(deserializer).map_err(|err| {
            let path = err.path().to_string();
            let path = if path == "." { String::new() } else { path };
            SchemaError {
                issues: vec![Issue::new(path, format!("JSON invalide : {}", err.inner()))],
            }
        })?;
        Self::from_spec(spec)
    }

    /// Valide un document déjà désérialisé.
    pub fn from_spec(spec: Spec) -> Result<Self, SchemaError> {
        validate::validate(spec)
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    pub fn tables(&self) -> &[Table] {
        &self.spec.tables
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.spec.tables.iter().find(|t| t.name == name)
    }

    pub fn column(&self, column: &ColumnRef) -> Option<&Column> {
        self.table(&column.table)?
            .columns
            .iter()
            .find(|c| c.name == column.column)
    }

    /// Colonne dont la valeur est lue : pour un lookup, la colonne au bout du
    /// chemin (avec sa table) ; sinon la colonne elle-même.
    pub fn resolved<'a>(&'a self, table: &'a str, column: &'a Column) -> (&'a str, &'a Column) {
        let (mut table, mut column) = (table, column);
        while column.ty == ColumnType::Lookup {
            let Some((dep, target)) = self
                .dependencies(&ColumnRef::new(table, &column.name))
                .and_then(|deps| deps.first())
                .and_then(|dep| Some((dep, self.column(dep)?)))
            else {
                break;
            };
            table = &dep.table;
            column = target;
        }
        (table, column)
    }

    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    /// AST de la formule d'une colonne calculée.
    pub fn formula(&self, column: &ColumnRef) -> Option<&Expr> {
        self.formulas.get(column)
    }

    /// Chemin d'une colonne `lookup`, découpé en segments.
    pub fn lookup_path(&self, column: &ColumnRef) -> Option<&[String]> {
        self.lookups.get(column).map(Vec::as_slice)
    }

    /// AST de la condition `when` de la règle `rule_index` de `table`.
    pub fn condition(&self, table: &str, rule_index: usize) -> Option<&Expr> {
        self.conditions.get(&(table.to_owned(), rule_index))
    }

    /// Colonnes lues par une colonne calculée (au bout de chacun de ses chemins).
    pub fn dependencies(&self, column: &ColumnRef) -> Option<&BTreeSet<ColumnRef>> {
        self.dependencies.get(column)
    }

    /// Fonctions intégrées et déclarées (signatures ; les fonctions déclarées
    /// restent à implémenter par l'application).
    pub fn functions(&self) -> &FunctionRegistry {
        &self.functions
    }

    /// Colonnes calculées (formules et lookups) triées de sorte que chacune
    /// vienne après celles dont elle dépend.
    pub fn computed_order(&self) -> &[ColumnRef] {
        &self.computed_order
    }
}
