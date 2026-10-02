//! Rendu d'une migration : opérations `Plan` de montée et de descente.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::diff::{Change, Hints, Risk, diff};
use crate::error::Error;
use crate::layout::{ColumnLayout, Layout, Storage, TableLayout};
use crate::render::Renderer;
use crate::writer::{OutputFile, Policy};

/// Migration à écrire, avec son bilan.
#[derive(Debug)]
pub(crate) struct Migration {
    pub name: String,
    pub file: OutputFile,
    pub changes: Vec<Change>,
}

#[derive(Serialize)]
struct Context<'a> {
    name: &'a str,
    summary: Vec<String>,
    data_loss: Vec<String>,
    may_fail: Vec<String>,
    up: Vec<String>,
    down: Vec<String>,
    uses_on_delete: bool,
    uses_schema: bool,
    redefines: bool,
}

/// Migration faisant passer la base de `before` à `after`, ou `None` si rien ne change.
pub(crate) fn plan(
    renderer: &Renderer,
    number: usize,
    before: &Layout,
    after: &Layout,
    hints: &Hints,
) -> Result<Option<Migration>, Error> {
    let changes = diff(before, after, hints);
    if changes.is_empty() {
        return Ok(None);
    }
    let reversed = hints.reversed();
    let down_changes = diff(after, before, &reversed);
    let name = format!("m{number:04}_{}", slug(before, &changes));

    let up = operations(&changes, after, hints);
    let down = operations(&down_changes, before, &reversed);
    let code: String = up.iter().chain(&down).map(String::as_str).collect();
    let (mut data_loss, mut may_fail) = (Vec::new(), Vec::new());
    for change in &changes {
        match change.risk() {
            Risk::DataLoss(reason) => data_loss.push(reason),
            Risk::MayFail(reason) => may_fail.push(reason),
            Risk::Safe => {}
        }
    }
    let context = Context {
        name: &name,
        summary: changes.iter().map(ToString::to_string).collect(),
        data_loss,
        may_fail,
        uses_on_delete: code.contains("OnDelete::"),
        redefines: code.contains(".redefine("),
        uses_schema: ["t.col(", ".add_column(", ".alter_column("]
            .iter()
            .any(|f| code.contains(f)),
        up,
        down,
    };
    let file = OutputFile {
        path: PathBuf::from(format!("backend/src/migrations/{name}.rs")),
        content: renderer.render("backend/migration.rs", &context)?,
        policy: Policy::Once,
    };
    Ok(Some(Migration {
        name,
        file,
        changes,
    }))
}

/// Suffixe lisible : `init`, ou les tables concernées.
fn slug(before: &Layout, changes: &[Change]) -> String {
    if before.tables.is_empty() {
        return "init".into();
    }
    let mut tables: Vec<&str> = changes.iter().map(Change::table).collect();
    tables.dedup();
    if tables.len() > 3 {
        tables.truncate(3);
        tables.push("etc");
    }
    tables.join("_")
}

/// Appels `Plan` des changements, puis la définition complète (`redefine`) de
/// chaque table modifiée, telle que dans `target`.
fn operations(changes: &[Change], target: &Layout, hints: &Hints) -> Vec<String> {
    let mut operations: Vec<String> = changes.iter().map(|c| operation(c, hints)).collect();
    let mut altered: Vec<&str> = changes.iter().filter_map(Change::altered_table).collect();
    altered.dedup();
    for name in altered {
        let table = target
            .table(name)
            .expect("table modifiée présente dans la cible");
        operations.push(format!(
            ".redefine({name:?}, |t| {{\n{}}})",
            table_body(table, hints)
        ));
    }
    operations
}

fn operation(change: &Change, hints: &Hints) -> String {
    match change {
        Change::CreateTable(table) => {
            format!(
                ".table({:?}, |t| {{\n{}}})",
                table.name,
                table_body(table, hints)
            )
        }
        Change::DropTable(table) => format!(".drop_table({:?})", table.name),
        Change::CreateJoinTable(join) => {
            format!(
                ".join_table({:?}, {:?}, {:?})",
                join.name, join.source, join.target
            )
        }
        Change::DropJoinTable(join) => format!(".drop_table({:?})", join.name),
        Change::AddColumn { table, column } => format!(
            ".add_column({table:?}, {})",
            column_definition(column, hints.default_of(table, &column.name))
        ),
        Change::DropColumn { table, column } => {
            format!(".drop_column({table:?}, {:?})", column.name)
        }
        Change::RenameColumn { table, from, to } => {
            format!(".rename_column({table:?}, {from:?}, {to:?})")
        }
        Change::AlterColumn { table, after, .. } => format!(
            ".alter_column({table:?}, {})",
            column_definition(after, hints.default_of(table, &after.name))
        ),
        Change::AddUnique { table, column, .. } => format!(".unique({table:?}, {column:?})"),
        Change::DropUnique { table, column } => format!(".drop_unique({table:?}, {column:?})"),
        Change::AddReference { table, column, .. } => format!(
            ".reference({table:?}, {:?}, {:?}, OnDelete::{})",
            column.name,
            column.references.as_deref().unwrap_or_default(),
            on_delete(column)
        ),
        Change::DropReference { table, column } => {
            format!(".drop_reference({table:?}, {column:?})")
        }
    }
}

/// Corps d'une définition de table : colonnes, unicités, références.
fn table_body(table: &TableLayout, hints: &Hints) -> String {
    let mut lines = Vec::new();
    for column in &table.columns {
        let default = hints.default_of(&table.name, &column.name);
        lines.push(format!("t.col({});", column_definition(column, default)));
    }
    for column in table.columns.iter().filter(|c| c.unique) {
        lines.push(format!("t.unique({:?});", column.name));
    }
    for column in &table.columns {
        if let Some(target) = &column.references {
            lines.push(format!(
                "t.reference({:?}, {target:?}, OnDelete::{});",
                column.name,
                on_delete(column)
            ));
        }
    }
    lines.iter().map(|l| l.clone() + "\n").collect()
}

/// Référence obligatoire : suppression de la cible interdite ; facultative : mise à `NULL`.
fn on_delete(column: &ColumnLayout) -> &'static str {
    if column.nullable {
        "SetNull"
    } else {
        "Restrict"
    }
}

/// Définition sea-orm-migration d'une colonne, par exemple
/// `decimal_len_null("montant", 19, 4)` ou `big_integer("probabilite").default(50)`.
///
/// La valeur par défaut du schéma devient celle de la base : elle remplit les
/// lignes existantes quand la colonne est ajoutée à une table.
pub(crate) fn column_definition(column: &ColumnLayout, default: Option<&Value>) -> String {
    let (function, extra) = match column.storage {
        Storage::String => ("string", ""),
        Storage::Text => ("text", ""),
        Storage::BigInteger => ("big_integer", ""),
        Storage::Decimal => ("decimal_len", ", 19, 4"),
        Storage::Boolean => ("boolean", ""),
        Storage::Date => ("date", ""),
        Storage::Timestamp => ("timestamp_with_time_zone", ""),
    };
    let null = if column.nullable { "_null" } else { "" };
    let default = default
        .and_then(|v| default_literal(column.storage, v))
        .map(|literal| format!(".default({literal})"))
        .unwrap_or_default();
    format!("{function}{null}({:?}{extra}){default}", column.name)
}

/// Littéral Rust d'une valeur par défaut (validée par le schéma).
fn default_literal(storage: Storage, value: &Value) -> Option<String> {
    Some(match storage {
        Storage::String | Storage::Text | Storage::Date | Storage::Timestamp => {
            format!("{:?}", value.as_str()?)
        }
        Storage::BigInteger => {
            let n = value.as_i64()?;
            if i32::try_from(n).is_ok() {
                n.to_string()
            } else {
                format!("{n}_i64")
            }
        }
        // Texte : converti par la base, sans perte de précision.
        Storage::Decimal => format!("{:?}", value.as_number()?.to_string()),
        Storage::Boolean => value.as_bool()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn column(name: &str, storage: Storage, nullable: bool) -> ColumnLayout {
        ColumnLayout {
            name: name.into(),
            storage,
            nullable,
            unique: false,
            references: None,
        }
    }

    #[test]
    fn column_definitions() {
        let montant = column("montant", Storage::Decimal, true);
        assert_eq!(
            column_definition(&montant, None),
            r#"decimal_len_null("montant", 19, 4)"#
        );
        assert_eq!(
            column_definition(&montant, Some(&json!(20))),
            r#"decimal_len_null("montant", 19, 4).default("20")"#
        );
        let probabilite = column("probabilite", Storage::BigInteger, false);
        assert_eq!(
            column_definition(&probabilite, Some(&json!(50))),
            r#"big_integer("probabilite").default(50)"#
        );
        assert_eq!(
            column_definition(&probabilite, Some(&json!(5_000_000_000_i64))),
            r#"big_integer("probabilite").default(5000000000_i64)"#
        );
        let couleur = column("couleur", Storage::String, true);
        assert_eq!(
            column_definition(&couleur, Some(&json!("#607D8B"))),
            r##"string_null("couleur").default("#607D8B")"##
        );
    }

    #[test]
    fn altered_tables_are_redefined() {
        let before = Layout {
            tables: vec![TableLayout {
                name: "client".into(),
                columns: vec![column("nom", Storage::String, false)],
            }],
            join_tables: Vec::new(),
        };
        let mut after = before.clone();
        let mut entreprise = column("entreprise", Storage::BigInteger, true);
        entreprise.references = Some("entreprise".into());
        after.tables[0].columns.push(entreprise);

        let changes = diff(&before, &after, &Hints::default());
        let operations = operations(&changes, &after, &Hints::default());
        assert_eq!(
            operations,
            [
                r#".add_column("client", big_integer_null("entreprise"))"#,
                r#".reference("client", "entreprise", "entreprise", OnDelete::SetNull)"#,
                ".redefine(\"client\", |t| {\nt.col(string(\"nom\"));\nt.col(big_integer_null(\"entreprise\"));\n\
                 t.reference(\"entreprise\", \"entreprise\", OnDelete::SetNull);\n})",
            ]
        );
        assert_eq!(slug(&before, &changes), "client");
        assert_eq!(slug(&Layout::empty(), &changes), "init");
    }
}
