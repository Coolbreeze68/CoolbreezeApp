//! Noms de l'API GraphQL dérivés du schéma.
//!
//! Source unique, partagée par la validation (détection des conflits) et par le
//! runtime (construction du schéma GraphQL). Pour une table `opportunite` :
//!
//! | Élément | Nom |
//! |---|---|
//! | type d'objet, d'entrée, de filtre, de page | `Opportunite`, `OpportuniteInput`, `OpportuniteFilter`, `OpportunitePage` |
//! | énumération de la colonne `etape` | `OpportuniteEtape` |
//! | requêtes | `opportunite(id)`, `opportunite_list`, `opportunite_aggregate` |
//! | mutations | `create_opportunite`, `update_opportunite`, `delete_opportunite` |

use std::collections::BTreeMap;

use crate::names::pascal_case;
use crate::spec::{ColumnType, Spec};

/// Scalaires propres à forge (en plus de `Int`, `Float`, `String`, `Boolean`, `ID`).
pub const SCALARS: [&str; 4] = ["Decimal", "Date", "DateTime", "JSON"];

/// Types définis par le runtime, indépendamment des tables.
pub const BUILTIN_TYPES: [&str; 18] = [
    "Query",
    "Mutation",
    "Int",
    "Float",
    "String",
    "Boolean",
    "ID",
    "Decimal",
    "Date",
    "DateTime",
    "JSON",
    "IDFilter",
    "IntFilter",
    "DecimalFilter",
    "StringFilter",
    "BooleanFilter",
    "DateFilter",
    "DateTimeFilter",
];

/// Requête et mutation des paramètres.
pub const PARAMETERS_QUERY: &str = "parameters";
pub const PARAMETER_MUTATION: &str = "update_parameter";

pub fn object(table: &str) -> String {
    pascal_case(table)
}

pub fn input(table: &str) -> String {
    format!("{}Input", pascal_case(table))
}

pub fn filter(table: &str) -> String {
    format!("{}Filter", pascal_case(table))
}

pub fn page(table: &str) -> String {
    format!("{}Page", pascal_case(table))
}

pub fn enumeration(table: &str, column: &str) -> String {
    pascal_case(table) + &pascal_case(column)
}

pub fn list(table: &str) -> String {
    format!("{table}_list")
}

pub fn aggregate(table: &str) -> String {
    format!("{table}_aggregate")
}

pub fn create(table: &str) -> String {
    format!("create_{table}")
}

pub fn update(table: &str) -> String {
    format!("update_{table}")
}

pub fn delete(table: &str) -> String {
    format!("delete_{table}")
}

/// Noms GraphQL en conflit : (indice de la table, message). Deux tables peuvent
/// produire le même nom (`a_b` + colonne `c` et `a` + colonne `b_c`), ou un nom
/// prédéfini (`decimal` → `Decimal`).
pub fn conflicts(spec: &Spec) -> Vec<(usize, String)> {
    let builtin = || "un type prédéfini".to_owned();
    let mut types: BTreeMap<String, String> = BUILTIN_TYPES
        .iter()
        .map(|t| ((*t).to_owned(), builtin()))
        .collect();
    let mut queries = BTreeMap::from([(PARAMETERS_QUERY.to_owned(), "les paramètres".to_owned())]);
    let mut mutations =
        BTreeMap::from([(PARAMETER_MUTATION.to_owned(), "les paramètres".to_owned())]);

    let mut found = Vec::new();
    let mut claim =
        |names: &mut BTreeMap<String, String>, index: usize, name: String, owner: String| {
            if let Some(previous) = names.get(&name) {
                found.push((
                    index,
                    format!("nom GraphQL `{name}` ({owner}) déjà utilisé par {previous}"),
                ));
            } else {
                names.insert(name, owner);
            }
        };
    for (i, table) in spec.tables.iter().enumerate() {
        let t = &table.name;
        claim(&mut types, i, object(t), format!("type de la table `{t}`"));
        claim(&mut types, i, input(t), format!("type d'entrée de `{t}`"));
        claim(&mut types, i, filter(t), format!("filtre de `{t}`"));
        claim(&mut types, i, page(t), format!("page de `{t}`"));
        for column in table.columns.iter().filter(|c| c.ty == ColumnType::Enum) {
            let owner = format!("énumération de `{t}.{}`", column.name);
            claim(&mut types, i, enumeration(t, &column.name), owner);
        }
        for name in [t.clone(), list(t), aggregate(t)] {
            claim(&mut queries, i, name, format!("requête de `{t}`"));
        }
        for name in [create(t), update(t), delete(t)] {
            claim(&mut mutations, i, name, format!("mutation de `{t}`"));
        }
    }
    found
}
